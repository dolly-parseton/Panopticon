use crate::imports::*;
use polars::prelude::*;

static AGGREGATECOMMAND_SPEC: CommandSchema = LazyLock::new(|| {
    let builder = CommandSpecBuilder::new().attribute(
        AttributeSpecBuilder::new("source", TypeDef::Scalar(ScalarType::String))
            .required()
            .hint("Path to tabular data in store (e.g., 'query.results.data')")
            .reference(ReferenceKind::StorePath)
            .build(),
    );

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
        Some(
            "Operation: sum, mean, min, max, count, first, last, std, median, n_unique, null_count",
        ),
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
        let source_path = StorePath::from_dotted(&self.source);

        let df = context.tabular().get(&source_path).await?.ok_or_else(|| {
            anyhow::anyhow!("Source '{}' not found in tabular store", self.source)
        })?;

        let out = InsertBatch::new(context, output_prefix);

        for agg in &self.aggregations {
            let value = compute_aggregation(&df, agg)?;
            out.scalar(&agg.name, value).await?;
        }

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
    // Going to redo these.
}
