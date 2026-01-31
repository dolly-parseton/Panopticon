use super::sealed;
use crate::imports::*;

impl sealed::Build for NamespaceBuilder<sealed::Iterative> {
    fn build(self) -> Result<Namespace> {
        NamespaceBuilder::<sealed::Iterative>::build(self)
    }
}
impl NamespaceBuilder<sealed::Iterative> {
    fn build(self) -> Result<Namespace> {
        // Ensure required fields are set
        let store_path = self
            .store_path
            .ok_or_else(|| anyhow::anyhow!("store_path is required for iterative namespace"))?;
        let source = self
            .source
            .ok_or_else(|| anyhow::anyhow!("source is required for iterative namespace"))?;
        Ok(Namespace::new(
            self.name,
            ExecutionMode::Iterative {
                store_path,
                source,
                iter_var: self.iter_var,
                index_var: self.index_var,
            },
            sealed::BuilderToken(()),
        ))
    }

    pub fn store_path(mut self, store_path: StorePath) -> Self {
        self.store_path = Some(store_path);
        self
    }

    pub fn iter_var(mut self, iter_var: &str) -> Self {
        self.iter_var = Some(iter_var.to_string());
        self
    }

    pub fn index_var(mut self, index_var: &str) -> Self {
        self.index_var = Some(index_var.to_string());
        self
    }

    // Iterator methods
    pub fn source(mut self, source: IteratorType) -> Self {
        self.source = Some(source);
        self
    }

    pub fn string_split(mut self, delimiter: &str) -> Self {
        self.source = Some(IteratorType::ScalarStringSplit {
            delimiter: delimiter.to_string(),
        });
        self
    }

    pub fn scalar_array(mut self, range: Option<(usize, usize)>) -> Self {
        self.source = Some(IteratorType::ScalarArray { range });
        self
    }

    pub fn scalar_object_keys(mut self, keys: Option<Vec<String>>, exclude: bool) -> Self {
        self.source = Some(IteratorType::ScalarObjectKeys { keys, exclude });
        self
    }

    pub fn tabular_column(mut self, column: &str, range: Option<(usize, usize)>) -> Self {
        self.source = Some(IteratorType::TabularColumn {
            column: column.to_string(),
            range,
        });
        self
    }
}

pub(in crate::namespace) async fn resolve_iterator_values(
    context: &ExecutionContext,
    namespace_type: &ExecutionMode,
) -> Result<Vec<ScalarValue>> {
    let ExecutionMode::Iterative {
        store_path,
        source,
        iter_var: _,
        index_var: _,
    } = namespace_type
    else {
        anyhow::bail!("extract_items called on non-iterative NamespaceType");
    };
    match source {
        IteratorType::ScalarStringSplit { delimiter } => {
            let value =
                context.scalar().get(store_path).await?.ok_or_else(|| {
                    anyhow::anyhow!("Key '{}' not found in scalar store", store_path)
                })?;

            let s = value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Expected string value for key '{}'", store_path))?;

            let items: Vec<ScalarValue> = s
                .split(delimiter)
                .map(|part| ScalarValue::String(part.to_string()))
                .collect();

            Ok(items)
        }

        IteratorType::ScalarArray { range } => {
            let value = context
                .scalar()
                .get(store_path)
                .await?
                .ok_or(anyhow::anyhow!(
                    "Key '{}' not found in scalar store",
                    store_path
                ))?;

            let arr = value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected array value for key '{}'", store_path))?;

            let items: Vec<ScalarValue> = match range {
                Some((start, end)) => arr
                    .iter()
                    .skip(*start)
                    .take(end.saturating_sub(*start))
                    .cloned()
                    .collect(),
                None => arr.to_vec(),
            };

            Ok(items)
        }

        IteratorType::ScalarObjectKeys { keys, exclude } => {
            let value = match context.scalar().get(store_path).await? {
                Some(v) => v,
                None => {
                    return Err(anyhow::anyhow!(
                        "Key '{}' not found in scalar store",
                        store_path
                    ));
                }
            };

            let obj = value
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("Expected object value for key '{}'", store_path))?;
            let items: Vec<ScalarValue> = obj
                .keys()
                .filter(|k| match keys {
                    Some(key_list) => {
                        let is_in_list = key_list.contains(&k.to_string());
                        if *exclude { !is_in_list } else { is_in_list }
                    }
                    None => true,
                })
                .map(|k| ScalarValue::String(k.clone()))
                .collect();

            Ok(items)
        }
        IteratorType::TabularColumn { column, range } => {
            let df = match context.tabular().get(store_path).await? {
                Some(df) => df,
                _ => {
                    return Err(anyhow::anyhow!(
                        "Expected DataFrame value for key '{}'",
                        store_path
                    ));
                }
            };

            let col = df
                .column(column)
                .map_err(|e| anyhow::anyhow!("Column '{}' not found: {}", column, e))?;

            let unique_col = col
                .unique()
                .map_err(|e| anyhow::anyhow!("Failed to get unique values: {}", e))?;

            // In polars 0.52+, Column wraps Series. Use as_materialized_series() to get &Series for iteration.
            let unique_series = unique_col.as_materialized_series();

            let iter = unique_series.iter().filter_map(|v| {
                use polars::prelude::AnyValue;
                match v {
                    AnyValue::Null => None,
                    AnyValue::String(s) => Some(ScalarValue::String(s.to_string())),
                    AnyValue::Int8(i) => Some(ScalarValue::Number(i.into())),
                    AnyValue::Int16(i) => Some(ScalarValue::Number(i.into())),
                    AnyValue::Int32(i) => Some(ScalarValue::Number(i.into())),
                    AnyValue::Int64(i) => Some(ScalarValue::Number(i.into())),
                    AnyValue::UInt8(i) => Some(ScalarValue::Number(i.into())),
                    AnyValue::UInt16(i) => Some(ScalarValue::Number(i.into())),
                    AnyValue::UInt32(i) => Some(ScalarValue::Number(i.into())),
                    AnyValue::UInt64(i) => Some(ScalarValue::Number(i.into())),
                    AnyValue::Float32(f) => {
                        tera::Number::from_f64(f as f64).map(ScalarValue::Number)
                    }
                    AnyValue::Float64(f) => tera::Number::from_f64(f).map(ScalarValue::Number),
                    AnyValue::Boolean(b) => Some(ScalarValue::Bool(b)),
                    other => Some(ScalarValue::String(format!("{}", other))),
                }
            });

            let items: Vec<ScalarValue> = match range {
                Some((start, end)) => iter.skip(*start).take(end.saturating_sub(*start)).collect(),
                None => iter.collect(),
            };

            Ok(items)
        }
    }
}
