use crate::imports::*;
use polars::prelude::IntoLazy;

static SQLCOMMAND_SPEC: CommandSchema = LazyLock::new(|| {
    let (pending, fields) = CommandSpecBuilder::new().array_of_objects(
        "tables",
        true,
        Some("Array of {name, source} objects mapping table names to stored data"),
    );

    let (fields, _) = fields.add_literal(
        "name",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Table name to use in SQL query"),
    );
    let fields = fields.add_template(
        "source",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Path to tabular data in store (e.g., 'load.users.data')"),
        ReferenceKind::StorePath,
    );

    pending
        .finalise_attribute(fields)
        .attribute(AttributeSpec {
            name: "query",
            ty: TypeDef::Scalar(ScalarType::String),
            required: true,
            hint: Some("SQL query to execute (supports Tera substitution)"),
            default_value: None,
            reference_kind: ReferenceKind::StaticTeraTemplate,
        })
        .fixed_result(
            "data",
            TypeDef::Tabular,
            Some("The query result as a DataFrame"),
            ResultKind::Data,
        )
        .fixed_result(
            "rows",
            TypeDef::Scalar(ScalarType::Number),
            Some("Number of rows in the result"),
            ResultKind::Meta,
        )
        .fixed_result(
            "columns",
            TypeDef::Scalar(ScalarType::Array),
            Some("Column names in the result"),
            ResultKind::Meta,
        )
        .build()
});

struct TableMapping {
    name: String,
    source: String,
}

pub struct SqlCommand {
    tables: Vec<TableMapping>,
    query: String,
}

#[async_trait::async_trait]
impl Executable for SqlCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        // Collect DataFrames from the tabular store
        let mut table_data: Vec<(String, TabularValue)> = Vec::with_capacity(self.tables.len());
        for table in &self.tables {
            let source_path = StorePath::from_dotted(&table.source);
            let df = context.tabular().get(&source_path).await?.ok_or_else(|| {
                anyhow::anyhow!("Table source '{}' not found in tabular store", table.source)
            })?;
            table_data.push((table.name.clone(), df));
        }

        // Substitute any Tera expressions in the query
        let query = context.substitute(&self.query).await?;

        // Execute the query in a blocking task (SQLContext is not Send-safe across await)
        let df = tokio::task::spawn_blocking(move || -> Result<TabularValue> {
            let mut sql_ctx = polars::sql::SQLContext::new();

            // Register all tables
            for (name, df) in table_data {
                sql_ctx.register(&name, df.lazy());
            }

            // Execute the query
            let lazy_result = match sql_ctx.execute(&query) {
                Ok(lazy_df) => lazy_df,
                Err(e) => {
                    tracing::warn!(
                        query = %query,
                        "SQL execution error"
                    );
                    return Err(anyhow::anyhow!("SQL execution failed: {}", e));
                }
            };

            // Collect the result
            lazy_result
                .collect()
                .map_err(|e| anyhow::anyhow!("Failed to collect query result: {}", e))
        })
        .await
        .map_err(|e| anyhow::anyhow!("Task join error: {}", e))??;

        // Store outputs
        let row_count = df.height() as u64;
        let column_names: Vec<ScalarValue> = df
            .get_column_names()
            .iter()
            .map(|n| ScalarValue::String(n.to_string()))
            .collect();

        let out = InsertBatch::new(context, output_prefix);
        out.tabular("data", df).await?;
        out.u64("rows", row_count).await?;
        out.scalar("columns", ScalarValue::Array(column_names))
            .await?;

        Ok(())
    }
}

impl Descriptor for SqlCommand {
    fn command_type() -> &'static str {
        "SqlCommand"
    }
    fn command_attributes() -> &'static [AttributeSpec<&'static str>] {
        &SQLCOMMAND_SPEC.0
    }
    fn command_results() -> &'static [ResultSpec<&'static str>] {
        &SQLCOMMAND_SPEC.1
    }
}

impl FromAttributes for SqlCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        let tables_array = attrs.get_required("tables")?.as_array_or_err("tables")?;

        let mut tables = Vec::with_capacity(tables_array.len());
        for (i, table_value) in tables_array.iter().enumerate() {
            let table_obj = table_value.as_object_or_err(&format!("tables[{}]", i))?;

            let name = table_obj
                .get_required_string("name")
                .context(format!("tables[{}]", i))?;
            let source = table_obj
                .get_required_string("source")
                .context(format!("tables[{}]", i))?;

            tables.push(TableMapping { name, source });
        }

        let query = attrs.get_required_string("query")?;

        Ok(SqlCommand { tables, query })
    }
}

#[cfg(test)]
mod tests {
    // Going to redo these.
}
