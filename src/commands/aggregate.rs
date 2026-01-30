use crate::imports::*;
use polars::prelude::*;

static AGGREGATECOMMAND_SPEC: LazyLock<(Vec<AttributeSpec<&'static str>>, Vec<ResultSpec<&'static str>>)> =
    LazyLock::new(|| {
        let builder = CommandSpecBuilder::new().attribute(AttributeSpec {
            name: "source",
            ty: TypeDef::Scalar(ScalarType::String),
            required: true,
            hint: Some("Path to tabular data in store (e.g., 'query.results.data')"),
            default_value: None,
            reference_kind: ReferenceKind::StorePath,
        });

        let (pending, fields) = builder.array_of_objects(
            "aggregations",
            true,
            Some("Array of {name, column, op} aggregation specifications"),
        );

        let (fields, name_ref) = fields.add_literal(
            "name",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Output scalar name"),
        );
        let (fields, _) = fields.add_literal(
            "column",
            TypeDef::Scalar(ScalarType::String),
            false,
            Some("Column to aggregate (not required for 'count')"),
        );
        let (fields, _) = fields.add_literal(
            "op",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Operation: sum, mean, min, max, count, first, last, std, median, n_unique, null_count"),
        );

        pending
            .finalise_attribute(fields)
            .derived_result("aggregations", name_ref, None, ResultKind::Data)
            .build()
    });

#[derive(Debug, Clone, Copy)]
enum AggregateOp {
    Sum,
    Mean,
    Min,
    Max,
    Count,
    First,
    Last,
    Std,
    Median,
    NUnique,
    NullCount,
}

impl AggregateOp {
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "sum" => Ok(Self::Sum),
            "mean" | "avg" | "average" => Ok(Self::Mean),
            "min" => Ok(Self::Min),
            "max" => Ok(Self::Max),
            "count" | "len" => Ok(Self::Count),
            "first" => Ok(Self::First),
            "last" => Ok(Self::Last),
            "std" | "stddev" => Ok(Self::Std),
            "median" => Ok(Self::Median),
            "n_unique" | "nunique" | "distinct" => Ok(Self::NUnique),
            "null_count" | "nulls" => Ok(Self::NullCount),
            other => anyhow::bail!("Unknown aggregation operation: '{}'", other),
        }
    }

    fn requires_column(&self) -> bool {
        !matches!(self, Self::Count)
    }
}

struct AggregationSpec {
    name: String,
    column: Option<String>,
    op: AggregateOp,
}

pub struct AggregateCommand {
    source: String,
    aggregations: Vec<AggregationSpec>,
}

#[async_trait::async_trait]
impl Executable for AggregateCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        tracing::info!(
            source = %self.source,
            aggregation_count = self.aggregations.len(),
            "Executing AggregateCommand"
        );

        let source_path = StorePath::from_dotted(&self.source);

        let df = context.tabular().get(&source_path).await?.ok_or_else(|| {
            anyhow::anyhow!("Source '{}' not found in tabular store", self.source)
        })?;

        let out = InsertBatch::new(context, output_prefix);

        for agg in &self.aggregations {
            tracing::debug!(
                name = %agg.name,
                column = ?agg.column,
                op = ?agg.op,
                "Computing aggregation"
            );

            let value = compute_aggregation(&df, agg)?;
            out.scalar(&agg.name, value).await?;
        }

        tracing::info!(
            aggregations_computed = self.aggregations.len(),
            "AggregateCommand completed"
        );

        Ok(())
    }
}

fn compute_aggregation(df: &TabularValue, agg: &AggregationSpec) -> Result<ScalarValue> {
    match agg.op {
        AggregateOp::Count => Ok(to_scalar::i64(df.height() as i64)),

        AggregateOp::NUnique => {
            let col_name = agg.column.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Aggregation '{}': n_unique requires a column", agg.name)
            })?;
            let column = df.column(col_name).map_err(|e| {
                anyhow::anyhow!(
                    "Aggregation '{}': column '{}' not found: {}",
                    agg.name,
                    col_name,
                    e
                )
            })?;
            let count = column.n_unique().map_err(|e| {
                anyhow::anyhow!("Aggregation '{}': n_unique failed: {}", agg.name, e)
            })?;
            Ok(to_scalar::i64(count as i64))
        }

        AggregateOp::NullCount => {
            let col_name = agg.column.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Aggregation '{}': null_count requires a column", agg.name)
            })?;
            let column = df.column(col_name).map_err(|e| {
                anyhow::anyhow!(
                    "Aggregation '{}': column '{}' not found: {}",
                    agg.name,
                    col_name,
                    e
                )
            })?;
            Ok(to_scalar::i64(column.null_count() as i64))
        }

        AggregateOp::Sum
        | AggregateOp::Mean
        | AggregateOp::Min
        | AggregateOp::Max
        | AggregateOp::First
        | AggregateOp::Last
        | AggregateOp::Std
        | AggregateOp::Median => {
            let col_name = agg.column.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Aggregation '{}': {:?} requires a column", agg.name, agg.op)
            })?;
            let column = df.column(col_name).map_err(|e| {
                anyhow::anyhow!(
                    "Aggregation '{}': column '{}' not found: {}",
                    agg.name,
                    col_name,
                    e
                )
            })?;

            extract_aggregation(column, agg.op)
        }
    }
}

fn extract_aggregation(column: &Column, op: AggregateOp) -> Result<ScalarValue> {
    // Convert Column to Series for aggregation methods
    let series = column.as_materialized_series();

    // Try numeric aggregation first
    let numeric_result: Option<f64> = match op {
        AggregateOp::Sum => series.sum().ok(),
        AggregateOp::Mean => series.mean(),
        AggregateOp::Min => series.min().ok().flatten(),
        AggregateOp::Max => series.max().ok().flatten(),
        AggregateOp::Std => series.std(1),
        AggregateOp::Median => series.median(),
        AggregateOp::First | AggregateOp::Last => {
            // Handle first/last for numeric columns
            if series.is_empty() {
                None
            } else {
                let idx = if matches!(op, AggregateOp::First) {
                    0
                } else {
                    series.len() - 1
                };
                series.get(idx).ok().and_then(|v| anyvalue_to_f64(&v))
            }
        }
        _ => unreachable!(),
    };

    // If we got a numeric result, convert it appropriately
    if let Some(v) = numeric_result {
        return f64_to_scalar(v);
    }

    // Handle string columns for first/last
    if matches!(op, AggregateOp::First | AggregateOp::Last) {
        if series.is_empty() {
            return Ok(to_scalar::null());
        }
        let idx = if matches!(op, AggregateOp::First) {
            0
        } else {
            series.len() - 1
        };
        if let Ok(av) = series.get(idx) {
            return anyvalue_to_scalar(&av);
        }
    }

    Ok(to_scalar::null())
}

fn anyvalue_to_f64(av: &AnyValue) -> Option<f64> {
    match av {
        AnyValue::Float64(f) => Some(*f),
        AnyValue::Float32(f) => Some(*f as f64),
        AnyValue::Int64(i) => Some(*i as f64),
        AnyValue::Int32(i) => Some(*i as f64),
        AnyValue::Int16(i) => Some(*i as f64),
        AnyValue::Int8(i) => Some(*i as f64),
        AnyValue::UInt64(u) => Some(*u as f64),
        AnyValue::UInt32(u) => Some(*u as f64),
        AnyValue::UInt16(u) => Some(*u as f64),
        AnyValue::UInt8(u) => Some(*u as f64),
        AnyValue::Null => None,
        _ => None,
    }
}

fn anyvalue_to_scalar(av: &AnyValue) -> Result<ScalarValue> {
    match av {
        AnyValue::Null => Ok(to_scalar::null()),
        AnyValue::Boolean(b) => Ok(to_scalar::bool(*b)),
        AnyValue::String(s) => Ok(to_scalar::string(*s)),
        AnyValue::Int64(i) => Ok(to_scalar::i64(*i)),
        AnyValue::Int32(i) => Ok(to_scalar::i64(*i as i64)),
        AnyValue::Int16(i) => Ok(to_scalar::i64(*i as i64)),
        AnyValue::Int8(i) => Ok(to_scalar::i64(*i as i64)),
        AnyValue::UInt64(u) => Ok(to_scalar::i64(*u as i64)),
        AnyValue::UInt32(u) => Ok(to_scalar::i64(*u as i64)),
        AnyValue::UInt16(u) => Ok(to_scalar::i64(*u as i64)),
        AnyValue::UInt8(u) => Ok(to_scalar::i64(*u as i64)),
        AnyValue::Float64(f) => f64_to_scalar(*f),
        AnyValue::Float32(f) => f64_to_scalar(*f as f64),
        _ => Ok(to_scalar::null()), // Unsupported types become null
    }
}

fn f64_to_scalar(v: f64) -> Result<ScalarValue> {
    if !v.is_finite() {
        return Ok(to_scalar::null());
    }

    // Preserve integer representation if possible
    if v.fract() == 0.0 && v.abs() < (i64::MAX as f64) {
        Ok(to_scalar::i64(v as i64))
    } else {
        Ok(to_scalar::f64(v))
    }
}

impl Descriptor for AggregateCommand {
    fn command_type() -> &'static str {
        "AggregateCommand"
    }
    fn command_attributes() -> &'static [AttributeSpec<&'static str>] {
        &AGGREGATECOMMAND_SPEC.0
    }
    fn command_results() -> &'static [ResultSpec<&'static str>] {
        &AGGREGATECOMMAND_SPEC.1
    }
}

impl FromAttributes for AggregateCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        let source = attrs.get_required_string("source")?;

        let aggregations_array = attrs
            .get_required("aggregations")?
            .as_array_or_err("aggregations")?;

        let mut aggregations = Vec::with_capacity(aggregations_array.len());
        for (i, agg_value) in aggregations_array.iter().enumerate() {
            let agg_obj = agg_value.as_object_or_err(&format!("aggregations[{}]", i))?;

            let name = agg_obj
                .get_required_string("name")
                .context(format!("aggregations[{}]", i))?;
            let column = agg_obj.get_optional_string("column");
            let op_str = agg_obj
                .get_required_string("op")
                .context(format!("aggregations[{}]", i))?;

            let op = AggregateOp::from_str(&op_str).context(format!("aggregations[{}]", i))?;

            if op.requires_column() && column.is_none() {
                return Err(anyhow::anyhow!(
                    "aggregations[{}]: operation '{}' requires a 'column' field",
                    i,
                    op_str
                ));
            }

            aggregations.push(AggregationSpec { name, column, op });
        }

        Ok(AggregateCommand {
            source,
            aggregations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_tracing;

    /// Helper to build aggregate command attributes
    fn agg_attrs(source: &str, aggregations: Vec<(&str, Option<&str>, &str)>) -> Attributes {
        let agg_values: Vec<ScalarValue> = aggregations
            .into_iter()
            .map(|(name, column, op)| {
                let mut builder = ObjectBuilder::new().insert("name", name).insert("op", op);
                if let Some(col) = column {
                    builder = builder.insert("column", col);
                }
                builder.build_scalar()
            })
            .collect();

        ObjectBuilder::new()
            .insert("source", source)
            .insert("aggregations", ScalarValue::Array(agg_values))
            .build_hashmap()
    }

    async fn get_scalar(
        context: &ExecutionContext,
        namespace: &str,
        command: &str,
        name: &str,
    ) -> Option<ScalarValue> {
        let prefix = StorePath::from_segments([namespace, command, name]);
        context.scalar().get(&prefix).await.unwrap()
    }

    #[tokio::test]
    async fn test_count_aggregation() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "id" => [1, 2, 3, 4, 5],
            "value" => [10, 20, 30, 40, 50]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "test"]), df)
            .await
            .unwrap();

        let cmd = AggregateCommand {
            source: "data.test".to_string(),
            aggregations: vec![AggregationSpec {
                name: "row_count".to_string(),
                column: None,
                op: AggregateOp::Count,
            }],
        };

        let prefix = StorePath::from_segments(["agg", "test"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let count = get_scalar(&context, "agg", "test", "row_count")
            .await
            .unwrap();
        assert_eq!(count.as_i64().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_sum_aggregation() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "amount" => [100, 200, 300]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "sales"]), df)
            .await
            .unwrap();

        let cmd = AggregateCommand {
            source: "data.sales".to_string(),
            aggregations: vec![AggregationSpec {
                name: "total".to_string(),
                column: Some("amount".to_string()),
                op: AggregateOp::Sum,
            }],
        };

        let prefix = StorePath::from_segments(["agg", "sales"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let total = get_scalar(&context, "agg", "sales", "total").await.unwrap();
        assert_eq!(total.as_i64().unwrap(), 600);
    }

    #[tokio::test]
    async fn test_mean_aggregation() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "score" => [10.0, 20.0, 30.0]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "scores"]), df)
            .await
            .unwrap();

        let cmd = AggregateCommand {
            source: "data.scores".to_string(),
            aggregations: vec![AggregationSpec {
                name: "average".to_string(),
                column: Some("score".to_string()),
                op: AggregateOp::Mean,
            }],
        };

        let prefix = StorePath::from_segments(["agg", "scores"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let avg = get_scalar(&context, "agg", "scores", "average")
            .await
            .unwrap();
        assert_eq!(avg.as_f64().unwrap(), 20.0);
    }

    #[tokio::test]
    async fn test_min_max_aggregation() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "value" => [5, 10, 15, 3, 8]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "values"]), df)
            .await
            .unwrap();

        let cmd = AggregateCommand {
            source: "data.values".to_string(),
            aggregations: vec![
                AggregationSpec {
                    name: "minimum".to_string(),
                    column: Some("value".to_string()),
                    op: AggregateOp::Min,
                },
                AggregationSpec {
                    name: "maximum".to_string(),
                    column: Some("value".to_string()),
                    op: AggregateOp::Max,
                },
            ],
        };

        let prefix = StorePath::from_segments(["agg", "minmax"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let min_val = get_scalar(&context, "agg", "minmax", "minimum")
            .await
            .unwrap();
        let max_val = get_scalar(&context, "agg", "minmax", "maximum")
            .await
            .unwrap();

        assert_eq!(min_val.as_i64().unwrap(), 3);
        assert_eq!(max_val.as_i64().unwrap(), 15);
    }

    #[tokio::test]
    async fn test_first_last_numeric() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "id" => [100, 200, 300]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "ids"]), df)
            .await
            .unwrap();

        let cmd = AggregateCommand {
            source: "data.ids".to_string(),
            aggregations: vec![
                AggregationSpec {
                    name: "first_id".to_string(),
                    column: Some("id".to_string()),
                    op: AggregateOp::First,
                },
                AggregationSpec {
                    name: "last_id".to_string(),
                    column: Some("id".to_string()),
                    op: AggregateOp::Last,
                },
            ],
        };

        let prefix = StorePath::from_segments(["agg", "firstlast"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let first = get_scalar(&context, "agg", "firstlast", "first_id")
            .await
            .unwrap();
        let last = get_scalar(&context, "agg", "firstlast", "last_id")
            .await
            .unwrap();

        assert_eq!(first.as_i64().unwrap(), 100);
        assert_eq!(last.as_i64().unwrap(), 300);
    }

    #[tokio::test]
    async fn test_first_last_string() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "name" => ["Alice", "Bob", "Charlie"]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "names"]), df)
            .await
            .unwrap();

        let cmd = AggregateCommand {
            source: "data.names".to_string(),
            aggregations: vec![
                AggregationSpec {
                    name: "first_name".to_string(),
                    column: Some("name".to_string()),
                    op: AggregateOp::First,
                },
                AggregationSpec {
                    name: "last_name".to_string(),
                    column: Some("name".to_string()),
                    op: AggregateOp::Last,
                },
            ],
        };

        let prefix = StorePath::from_segments(["agg", "names"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let first = get_scalar(&context, "agg", "names", "first_name")
            .await
            .unwrap();
        let last = get_scalar(&context, "agg", "names", "last_name")
            .await
            .unwrap();

        assert_eq!(first.as_str().unwrap(), "Alice");
        assert_eq!(last.as_str().unwrap(), "Charlie");
    }

    #[tokio::test]
    async fn test_n_unique_aggregation() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "category" => ["A", "B", "A", "C", "B", "A"]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "categories"]), df)
            .await
            .unwrap();

        let cmd = AggregateCommand {
            source: "data.categories".to_string(),
            aggregations: vec![AggregationSpec {
                name: "unique_count".to_string(),
                column: Some("category".to_string()),
                op: AggregateOp::NUnique,
            }],
        };

        let prefix = StorePath::from_segments(["agg", "unique"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let unique = get_scalar(&context, "agg", "unique", "unique_count")
            .await
            .unwrap();
        assert_eq!(unique.as_i64().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_multiple_aggregations() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "price" => [10.0, 20.0, 30.0, 40.0, 50.0],
            "category" => ["A", "B", "A", "B", "C"]
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "products"]), df)
            .await
            .unwrap();

        let cmd = AggregateCommand {
            source: "data.products".to_string(),
            aggregations: vec![
                AggregationSpec {
                    name: "total_price".to_string(),
                    column: Some("price".to_string()),
                    op: AggregateOp::Sum,
                },
                AggregationSpec {
                    name: "avg_price".to_string(),
                    column: Some("price".to_string()),
                    op: AggregateOp::Mean,
                },
                AggregationSpec {
                    name: "product_count".to_string(),
                    column: None,
                    op: AggregateOp::Count,
                },
                AggregationSpec {
                    name: "category_count".to_string(),
                    column: Some("category".to_string()),
                    op: AggregateOp::NUnique,
                },
            ],
        };

        let prefix = StorePath::from_segments(["agg", "products"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let total = get_scalar(&context, "agg", "products", "total_price")
            .await
            .unwrap();
        let avg = get_scalar(&context, "agg", "products", "avg_price")
            .await
            .unwrap();
        let count = get_scalar(&context, "agg", "products", "product_count")
            .await
            .unwrap();
        let categories = get_scalar(&context, "agg", "products", "category_count")
            .await
            .unwrap();

        assert_eq!(total.as_f64().unwrap(), 150.0);
        assert_eq!(avg.as_f64().unwrap(), 30.0);
        assert_eq!(count.as_i64().unwrap(), 5);
        assert_eq!(categories.as_i64().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_empty_dataframe() {
        init_tracing();
        let context = ExecutionContext::new();

        let df = df! {
            "value" => Vec::<i64>::new()
        }
        .unwrap();

        context
            .tabular()
            .insert(&StorePath::from_segments(["data", "empty"]), df)
            .await
            .unwrap();

        let cmd = AggregateCommand {
            source: "data.empty".to_string(),
            aggregations: vec![
                AggregationSpec {
                    name: "row_count".to_string(),
                    column: None,
                    op: AggregateOp::Count,
                },
                AggregationSpec {
                    name: "total".to_string(),
                    column: Some("value".to_string()),
                    op: AggregateOp::Sum,
                },
            ],
        };

        let prefix = StorePath::from_segments(["agg", "empty"]);
        cmd.execute(&context, &prefix).await.unwrap();

        let count = get_scalar(&context, "agg", "empty", "row_count")
            .await
            .unwrap();
        assert_eq!(count.as_i64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_missing_source_error() {
        init_tracing();
        let context = ExecutionContext::new();

        let cmd = AggregateCommand {
            source: "does.not.exist".to_string(),
            aggregations: vec![AggregationSpec {
                name: "count".to_string(),
                column: None,
                op: AggregateOp::Count,
            }],
        };

        let prefix = StorePath::from_segments(["agg", "error"]);
        let result = cmd.execute(&context, &prefix).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found in tabular store"));
    }

    #[tokio::test]
    async fn test_missing_column_error() {
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

        let cmd = AggregateCommand {
            source: "data.test".to_string(),
            aggregations: vec![AggregationSpec {
                name: "total".to_string(),
                column: Some("nonexistent".to_string()),
                op: AggregateOp::Sum,
            }],
        };

        let prefix = StorePath::from_segments(["agg", "error"]);
        let result = cmd.execute(&context, &prefix).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("column 'nonexistent' not found"));
    }

    #[tokio::test]
    async fn test_from_attributes() {
        init_tracing();

        let attrs = agg_attrs(
            "files.load.data",
            vec![
                ("total", Some("amount"), "sum"),
                ("average", Some("price"), "mean"),
                ("rows", None, "count"),
            ],
        );

        let cmd = AggregateCommand::from_attributes(&attrs).unwrap();

        assert_eq!(cmd.source, "files.load.data");
        assert_eq!(cmd.aggregations.len(), 3);
        assert_eq!(cmd.aggregations[0].name, "total");
        assert_eq!(cmd.aggregations[0].column, Some("amount".to_string()));
        assert_eq!(cmd.aggregations[1].name, "average");
        assert_eq!(cmd.aggregations[2].name, "rows");
        assert!(cmd.aggregations[2].column.is_none());
    }

    #[tokio::test]
    async fn test_factory_rejects_missing_source() {
        init_tracing();

        let agg = ObjectBuilder::new()
            .insert("name", "test")
            .insert("op", "count")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("aggregations", ScalarValue::Array(vec![agg]))
            .build_hashmap();

        let factory = AggregateCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing attribute"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required attribute 'source'"),
                    "Expected missing attribute error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_missing_aggregations() {
        init_tracing();

        let attrs = ObjectBuilder::new()
            .insert("source", "data.test")
            .build_hashmap();

        let factory = AggregateCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing attribute"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required attribute 'aggregations'"),
                    "Expected missing attribute error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_invalid_operation() {
        init_tracing();

        let agg = ObjectBuilder::new()
            .insert("name", "test")
            .insert("column", "value")
            .insert("op", "invalid_op")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("source", "data.test")
            .insert("aggregations", ScalarValue::Array(vec![agg]))
            .build_hashmap();

        let factory = AggregateCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with invalid operation"),
            Err(err) => {
                // Use alternate display format to get full error chain
                let msg = format!("{:#}", err);
                assert!(
                    msg.contains("Unknown aggregation operation"),
                    "Expected invalid operation error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_missing_column_for_sum() {
        init_tracing();

        let agg = ObjectBuilder::new()
            .insert("name", "test")
            .insert("op", "sum")
            // Missing column
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("source", "data.test")
            .insert("aggregations", ScalarValue::Array(vec![agg]))
            .build_hashmap();

        let factory = AggregateCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing column"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("requires a 'column' field"),
                    "Expected missing column error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_with_commands_execution() {
        init_tracing();

        let df = df! {
            "amount" => [100, 200, 300, 400],
            "category" => ["A", "B", "A", "B"]
        }
        .unwrap();

        // Set up the data in context
        let context = ExecutionContext::new();
        context
            .tabular()
            .insert(&StorePath::from_segments(["source", "data"]), df)
            .await
            .unwrap();

        // Build the command through attributes
        let attrs = agg_attrs(
            "source.data",
            vec![
                ("total_amount", Some("amount"), "sum"),
                ("row_count", None, "count"),
                ("unique_categories", Some("category"), "n_unique"),
            ],
        );

        // Execute directly on the context
        let cmd = AggregateCommand::from_attributes(&attrs).unwrap();
        let prefix = StorePath::from_segments(["stats", "summary"]);
        cmd.execute(&context, &prefix).await.unwrap();

        // Verify results
        let total = get_scalar(&context, "stats", "summary", "total_amount")
            .await
            .unwrap();
        let count = get_scalar(&context, "stats", "summary", "row_count")
            .await
            .unwrap();
        let categories = get_scalar(&context, "stats", "summary", "unique_categories")
            .await
            .unwrap();

        assert_eq!(total.as_i64().unwrap(), 1000);
        assert_eq!(count.as_i64().unwrap(), 4);
        assert_eq!(categories.as_i64().unwrap(), 2);
    }
}
