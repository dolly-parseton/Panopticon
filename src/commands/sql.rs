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
    use super::*;
    use crate::test_utils::init_tracing;
    use polars::prelude::*;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
    }

    /// Helper to build sql command attributes
    fn sql_attrs(tables: Vec<(&str, &str)>, query: &str) -> Attributes {
        let table_values: Vec<ScalarValue> = tables
            .into_iter()
            .map(|(name, source)| {
                ObjectBuilder::new()
                    .insert("name", name)
                    .insert("source", source)
                    .build_scalar()
            })
            .collect();

        ObjectBuilder::new()
            .insert("tables", ScalarValue::Array(table_values))
            .insert("query", query)
            .build_hashmap()
    }

    async fn get_results(
        context: &ExecutionContext,
        namespace: &str,
        command: &str,
    ) -> (u64, Vec<String>) {
        let prefix = StorePath::from_segments([namespace, command]);
        let rows = context
            .scalar()
            .get(&prefix.with_segment("rows"))
            .await
            .unwrap()
            .unwrap()
            .as_u64()
            .unwrap();
        let columns = context
            .scalar()
            .get(&prefix.with_segment("columns"))
            .await
            .unwrap()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        (rows, columns)
    }

    async fn setup_test_data(pipeline: &mut Pipeline) {
        // Load test CSV files using FileCommand
        let file1 = ObjectBuilder::new()
            .insert("name", "users")
            .insert(
                "file",
                fixtures_dir()
                    .join("users.csv")
                    .to_string_lossy()
                    .to_string(),
            )
            .insert("format", "csv")
            .build_scalar();

        let file2 = ObjectBuilder::new()
            .insert("name", "products")
            .insert(
                "file",
                fixtures_dir()
                    .join("products.csv")
                    .to_string_lossy()
                    .to_string(),
            )
            .insert("format", "csv")
            .build_scalar();

        let file_attrs = ObjectBuilder::new()
            .insert("files", ScalarValue::Array(vec![file1, file2]))
            .build_hashmap();

        pipeline
            .add_namespace(NamespaceBuilder::new("files"))
            .unwrap()
            .add_command::<FileCommand>("load", &file_attrs)
            .unwrap();
    }

    #[tokio::test]
    async fn test_simple_select() {
        init_tracing();
        let mut pipeline = Pipeline::new();
        setup_test_data(&mut pipeline).await;

        let attrs = sql_attrs(
            vec![("users", "files.load.users.data")],
            "SELECT * FROM users",
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("query"))
            .unwrap()
            .add_command::<SqlCommand>("all_users", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();

        let (rows, columns) = get_results(&context, "query", "all_users").await;
        assert_eq!(rows, 3);
        assert_eq!(columns, vec!["id", "name", "email", "age"]);

        // Verify tabular data exists
        let prefix = StorePath::from_segments(["query", "all_users", "data"]);
        let df = context.tabular().get(&prefix).await.unwrap().unwrap();
        assert_eq!(df.height(), 3);
        assert_eq!(df.width(), 4);
    }

    #[tokio::test]
    async fn test_select_with_where() {
        init_tracing();
        let context = ExecutionContext::new();

        // Create test DataFrame directly
        let users_df = df! {
            "name" => ["Alice", "Bob", "Charlie"],
            "age" => [20, 30, 25]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "users"]), users_df)
            .await
            .unwrap();

        let cmd = SqlCommand {
            tables: vec![TableMapping {
                name: "users".to_string(),
                source: "data.users".to_string(),
            }],
            query: "SELECT name, age FROM users WHERE age > 25".to_string(),
        };

        let prefix = StorePath::from_segments(["query", "older_users"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let rows = context
            .scalar()
            .get(&prefix.with_segment("rows"))
            .await
            .unwrap()
            .unwrap()
            .as_u64()
            .unwrap();

        // Only Bob (age 30) should match WHERE age > 25
        assert_eq!(rows, 1);

        let df = context
            .tabular()
            .get(&prefix.with_segment("data"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(df.width(), 2); // name, age columns
    }

    #[tokio::test]
    async fn test_select_with_aggregation() {
        init_tracing();
        let mut pipeline = Pipeline::new();
        setup_test_data(&mut pipeline).await;

        let attrs = sql_attrs(
            vec![("users", "files.load.users.data")],
            "SELECT COUNT(*) as user_count, AVG(age) as avg_age FROM users",
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("query"))
            .unwrap()
            .add_command::<SqlCommand>("stats", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();

        let (rows, columns) = get_results(&context, "query", "stats").await;
        assert_eq!(rows, 1); // Single aggregation row
        assert!(columns.contains(&"user_count".to_string()));
        assert!(columns.contains(&"avg_age".to_string()));
    }

    #[tokio::test]
    async fn test_join_tables() {
        init_tracing();

        // Create two DataFrames for join test
        let context = ExecutionContext::new();

        let users_df = df! {
            "user_id" => [1, 2, 3],
            "name" => ["Alice", "Bob", "Charlie"]
        }
        .unwrap();

        let orders_df = df! {
            "order_id" => [101, 102, 103, 104],
            "user_id" => [1, 1, 2, 3],
            "amount" => [100.0, 200.0, 150.0, 300.0]
        }
        .unwrap();

        // Store DataFrames directly
        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "users"]), users_df)
            .await
            .unwrap();
        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "orders"]), orders_df)
            .await
            .unwrap();

        // Build and execute SqlCommand directly
        let cmd = SqlCommand {
            tables: vec![
                TableMapping {
                    name: "users".to_string(),
                    source: "data.users".to_string(),
                },
                TableMapping {
                    name: "orders".to_string(),
                    source: "data.orders".to_string(),
                },
            ],
            query: r#"
                SELECT u.name, SUM(o.amount) as total_spent
                FROM users u
                JOIN orders o ON u.user_id = o.user_id
                GROUP BY u.name
                ORDER BY total_spent DESC
            "#
            .to_string(),
        };

        let prefix = StorePath::from_segments(["query", "joined"]);
        cmd.execute(&context, &prefix).await.unwrap();

        // Verify results
        let rows = context
            .scalar()
            .get(&prefix.with_segment("rows"))
            .await
            .unwrap()
            .unwrap()
            .as_u64()
            .unwrap();

        assert_eq!(rows, 3); // 3 users with orders

        let df = context
            .tabular()
            .get(&prefix.with_segment("data"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(df.width(), 2); // name, total_spent
    }

    #[tokio::test]
    async fn test_tera_substitution_in_query() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        // Add inputs namespace with filter value
        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("min_age", ScalarValue::Number(25.into())),
            )
            .unwrap();

        setup_test_data(&mut pipeline).await;

        let attrs = sql_attrs(
            vec![("users", "files.load.users.data")],
            "SELECT * FROM users WHERE age >= {{ inputs.min_age }}",
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("query"))
            .unwrap()
            .add_command::<SqlCommand>("filtered", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();

        // Query should have filtered based on inputs.min_age
        let prefix = StorePath::from_segments(["query", "filtered", "data"]);
        let df = context.tabular().get(&prefix).await.unwrap().unwrap();
        assert!(df.height() <= 3);
    }

    #[tokio::test]
    async fn test_missing_table_source_error() {
        init_tracing();
        let context = ExecutionContext::new();
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = SqlCommand {
            tables: vec![TableMapping {
                name: "nonexistent".to_string(),
                source: "does.not.exist".to_string(),
            }],
            query: "SELECT * FROM nonexistent".to_string(),
        };

        let result = cmd.execute(&context, &prefix).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found in tabular store"));
    }

    #[tokio::test]
    async fn test_invalid_sql_error() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "id" => [1, 2, 3]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "test"]), df)
            .await
            .unwrap();

        let cmd = SqlCommand {
            tables: vec![TableMapping {
                name: "test".to_string(),
                source: "data.test".to_string(),
            }],
            query: "INVALID SQL SYNTAX HERE".to_string(),
        };

        let prefix = StorePath::from_segments(["ns", "cmd"]);
        let result = cmd.execute(&context, &prefix).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("SQL execution failed"));
    }

    #[tokio::test]
    async fn test_from_attributes() {
        init_tracing();

        let table1 = ObjectBuilder::new()
            .insert("name", "users")
            .insert("source", "files.load.users.data")
            .build_scalar();

        let table2 = ObjectBuilder::new()
            .insert("name", "orders")
            .insert("source", "files.load.orders.data")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("tables", ScalarValue::Array(vec![table1, table2]))
            .insert(
                "query",
                "SELECT * FROM users JOIN orders ON users.id = orders.user_id",
            )
            .build_hashmap();

        let cmd = SqlCommand::from_attributes(&attrs).unwrap();

        assert_eq!(cmd.tables.len(), 2);
        assert_eq!(cmd.tables[0].name, "users");
        assert_eq!(cmd.tables[0].source, "files.load.users.data");
        assert_eq!(cmd.tables[1].name, "orders");
        assert_eq!(cmd.tables[1].source, "files.load.orders.data");
        assert!(cmd.query.contains("SELECT * FROM users"));
    }

    #[tokio::test]
    async fn test_factory_rejects_missing_tables() {
        init_tracing();

        let attrs = ObjectBuilder::new()
            .insert("query", "SELECT * FROM test")
            .build_hashmap();

        let factory = SqlCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing attribute"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required attribute 'tables'"),
                    "Expected missing attribute error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_missing_query() {
        init_tracing();

        let table = ObjectBuilder::new()
            .insert("name", "test")
            .insert("source", "data.test")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("tables", ScalarValue::Array(vec![table]))
            .build_hashmap();

        let factory = SqlCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing attribute"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required attribute 'query'"),
                    "Expected missing attribute error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_invalid_table_object() {
        init_tracing();

        // Missing 'source' field
        let table = ObjectBuilder::new().insert("name", "test").build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("tables", ScalarValue::Array(vec![table]))
            .insert("query", "SELECT * FROM test")
            .build_hashmap();

        let factory = SqlCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing field"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required field 'tables[0].source'"),
                    "Expected missing field error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_empty_result() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "id" => [1, 2, 3],
            "value" => [10, 20, 30]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "test"]), df)
            .await
            .unwrap();

        let cmd = SqlCommand {
            tables: vec![TableMapping {
                name: "test".to_string(),
                source: "data.test".to_string(),
            }],
            query: "SELECT * FROM test WHERE value > 100".to_string(),
        };

        let prefix = StorePath::from_segments(["query", "empty"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let rows = context
            .scalar()
            .get(&prefix.with_segment("rows"))
            .await
            .unwrap()
            .unwrap()
            .as_u64()
            .unwrap();

        assert_eq!(rows, 0);
    }
}
