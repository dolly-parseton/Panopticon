use crate::prelude::*;

#[derive(Debug)]
pub struct Namespace {
    pub name: String,
    pub ty: NamespaceType,
}

pub mod sealed {
    #[doc(hidden)]
    pub struct Single(());
    #[doc(hidden)]
    pub struct Iterative(());
}
use sealed::{Iterative, Single};

pub struct NamespaceBuilder<T> {
    name: String,
    // options
    store_path: Option<StorePath>,
    source: Option<IteratorSource>,
    iter_var: Option<String>,
    index_var: Option<String>,
    // marker
    _marker: std::marker::PhantomData<T>,
}

impl NamespaceBuilder<Single> {
    pub fn new(name: &str) -> Self {
        NamespaceBuilder {
            name: name.to_string(),
            store_path: None,
            source: None,
            iter_var: None,
            index_var: None,
            _marker: std::marker::PhantomData,
        }
    }
    pub fn build(self) -> Result<Namespace> {
        Ok(Namespace {
            name: self.name,
            ty: NamespaceType::Single,
        })
    }
    pub fn iterative(self) -> NamespaceBuilder<Iterative> {
        NamespaceBuilder {
            name: self.name,
            store_path: None,
            source: None,
            iter_var: None,
            index_var: None,
            _marker: std::marker::PhantomData,
        }
    }
}

impl NamespaceBuilder<Iterative> {
    pub fn build(self) -> Result<Namespace> {
        // Ensure required fields are set
        let store_path = self
            .store_path
            .ok_or_else(|| anyhow::anyhow!("store_path is required for iterative namespace"))?;
        let source = self
            .source
            .ok_or_else(|| anyhow::anyhow!("source is required for iterative namespace"))?;
        Ok(Namespace {
            name: self.name,
            ty: NamespaceType::Iterative {
                store_path,
                source,
                iter_var: self.iter_var,
                index_var: self.index_var,
            },
        })
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
    pub fn source(mut self, source: IteratorSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn string_split(mut self, delimiter: &str) -> Self {
        self.source = Some(IteratorSource::ScalarStringSplit {
            delimiter: delimiter.to_string(),
        });
        self
    }

    pub fn scalar_array(mut self, range: Option<(usize, usize)>) -> Self {
        self.source = Some(IteratorSource::ScalarArray { range });
        self
    }

    pub fn scalar_object_keys(mut self, keys: Option<Vec<String>>, exclude: bool) -> Self {
        self.source = Some(IteratorSource::ScalarObjectKeys { keys, exclude });
        self
    }

    pub fn tabular_column(mut self, column: &str, range: Option<(usize, usize)>) -> Self {
        self.source = Some(IteratorSource::TabularColumn {
            column: column.to_string(),
            range,
        });
        self
    }
}

#[derive(Debug, Default)]
pub enum NamespaceType {
    #[default]
    Single,
    Iterative {
        store_path: StorePath,
        source: IteratorSource,
        iter_var: Option<String>,  // If None, defaults to "item"
        index_var: Option<String>, // If None, defaults to "index"
    },
}

#[derive(Debug)]
pub enum IteratorSource {
    ScalarStringSplit {
        delimiter: String,
    },
    ScalarArray {
        range: Option<(usize, usize)>,
    },
    ScalarObjectKeys {
        keys: Option<Vec<String>>,
        exclude: bool,
    },
    TabularColumn {
        column: String,
        range: Option<(usize, usize)>,
    },
}

#[tracing::instrument(level = "debug", skip(context), fields(namespace_type = ?namespace_type))]
pub async fn extract_items(
    context: &ExecutionContext,
    namespace_type: &NamespaceType,
) -> Result<Vec<ScalarValue>> {
    tracing::debug!("Extracting items for iterative namespace");
    let NamespaceType::Iterative {
        store_path,
        source,
        iter_var: _,
        index_var: _,
    } = namespace_type
    else {
        anyhow::bail!("extract_items called on non-iterative NamespaceType");
    };
    match source {
        IteratorSource::ScalarStringSplit { delimiter } => {
            tracing::debug!(
                store_path = store_path.to_dotted().as_str(),
                delimiter = delimiter.as_str(),
                "Extracting items via string split"
            );
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

        IteratorSource::ScalarArray { range } => {
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

        IteratorSource::ScalarObjectKeys { keys, exclude } => {
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
        IteratorSource::TabularColumn { column, range } => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn init_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer() // integrates with cargo test output
            .try_init();
    }

    #[tokio::test]
    async fn test_extract_items_scalar_array() {
        init_tracing();

        let mut source: HashMap<String, ScalarValue> = HashMap::new();
        source.insert("data".to_string(), {
            let mut inner = HashMap::new();
            inner.insert(
                "items".to_string(),
                ScalarValue::Array(vec![
                    ScalarValue::String("apple".to_string()),
                    ScalarValue::String("banana".to_string()),
                    ScalarValue::String("cherry".to_string()),
                    ScalarValue::String("date".to_string()),
                ]),
            );
            scalar_value_from(inner).unwrap()
        });

        let context = ExecutionContext::new(Some(&source));

        let ns_type = NamespaceType::Iterative {
            store_path: StorePath::from_segments(["data", "items"]),
            source: IteratorSource::ScalarArray { range: None },
            iter_var: None,
            index_var: None,
        };

        let items = extract_items(&context, &ns_type).await.unwrap();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0], ScalarValue::String("apple".to_string()));
        assert_eq!(items[1], ScalarValue::String("banana".to_string()));
        assert_eq!(items[2], ScalarValue::String("cherry".to_string()));
        assert_eq!(items[3], ScalarValue::String("date".to_string()));
    }

    #[tokio::test]
    async fn test_extract_items_scalar_array_with_range() {
        init_tracing();

        let mut source: HashMap<String, ScalarValue> = HashMap::new();
        source.insert("data".to_string(), {
            let mut inner = HashMap::new();
            inner.insert(
                "items".to_string(),
                ScalarValue::Array(vec![
                    ScalarValue::String("a".to_string()),
                    ScalarValue::String("b".to_string()),
                    ScalarValue::String("c".to_string()),
                    ScalarValue::String("d".to_string()),
                    ScalarValue::String("e".to_string()),
                ]),
            );
            scalar_value_from(inner).unwrap()
        });

        let context = ExecutionContext::new(Some(&source));

        // Test range (1, 4) - should get items at index 1, 2, 3 ("b", "c", "d")
        let ns_type = NamespaceType::Iterative {
            store_path: StorePath::from_segments(["data", "items"]),
            source: IteratorSource::ScalarArray {
                range: Some((1, 4)),
            },
            iter_var: None,
            index_var: None,
        };

        let items = extract_items(&context, &ns_type).await.unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], ScalarValue::String("b".to_string()));
        assert_eq!(items[1], ScalarValue::String("c".to_string()));
        assert_eq!(items[2], ScalarValue::String("d".to_string()));
    }

    #[tokio::test]
    async fn test_extract_items_string_split() {
        init_tracing();

        let mut source: HashMap<String, ScalarValue> = HashMap::new();
        source.insert("source".to_string(), {
            let mut inner = HashMap::new();
            inner.insert(
                "csv".to_string(),
                ScalarValue::String("one,two,three".to_string()),
            );
            scalar_value_from(inner).unwrap()
        });
        println!("Source: {:?}", source);

        let context = ExecutionContext::new(Some(&source));

        println!("Context created");

        let ns_type = NamespaceType::Iterative {
            store_path: StorePath::from_segments(["source", "csv"]),
            source: IteratorSource::ScalarStringSplit {
                delimiter: ",".to_string(),
            },
            iter_var: None,
            index_var: None,
        };

        println!("NamespaceType created");

        let items = extract_items(&context, &ns_type).await.unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], ScalarValue::String("one".to_string()));
    }

    #[tokio::test]
    async fn test_extract_items_scalar_object_keys_all() {
        init_tracing();

        let mut source: HashMap<String, ScalarValue> = HashMap::new();
        source.insert("data".to_string(), {
            let mut inner = HashMap::new();
            let mut obj = HashMap::new();
            obj.insert(
                "key1".to_string(),
                ScalarValue::String("value1".to_string()),
            );
            obj.insert(
                "key2".to_string(),
                ScalarValue::String("value2".to_string()),
            );
            obj.insert(
                "key3".to_string(),
                ScalarValue::String("value3".to_string()),
            );
            inner.insert("object".to_string(), scalar_value_from(obj).unwrap());
            scalar_value_from(inner).unwrap()
        });

        let context = ExecutionContext::new(Some(&source));

        // Get all keys (no filter)
        let ns_type = NamespaceType::Iterative {
            store_path: StorePath::from_segments(["data", "object"]),
            source: IteratorSource::ScalarObjectKeys {
                keys: None,
                exclude: false,
            },
            iter_var: None,
            index_var: None,
        };

        let items = extract_items(&context, &ns_type).await.unwrap();
        assert_eq!(items.len(), 3);
        // Keys may be in any order, so check that all expected keys are present
        let keys: Vec<String> = items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[tokio::test]
    async fn test_extract_items_scalar_object_keys_filtered() {
        init_tracing();

        let mut source: HashMap<String, ScalarValue> = HashMap::new();
        source.insert("data".to_string(), {
            let mut inner = HashMap::new();
            let mut obj = HashMap::new();
            obj.insert("alpha".to_string(), ScalarValue::Number(1.into()));
            obj.insert("beta".to_string(), ScalarValue::Number(2.into()));
            obj.insert("gamma".to_string(), ScalarValue::Number(3.into()));
            obj.insert("delta".to_string(), ScalarValue::Number(4.into()));
            inner.insert("object".to_string(), scalar_value_from(obj).unwrap());
            scalar_value_from(inner).unwrap()
        });

        let context = ExecutionContext::new(Some(&source));

        // Include only specific keys
        let ns_type = NamespaceType::Iterative {
            store_path: StorePath::from_segments(["data", "object"]),
            source: IteratorSource::ScalarObjectKeys {
                keys: Some(vec!["alpha".to_string(), "gamma".to_string()]),
                exclude: false,
            },
            iter_var: None,
            index_var: None,
        };

        let items = extract_items(&context, &ns_type).await.unwrap();
        assert_eq!(items.len(), 2);
        let keys: Vec<String> = items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        assert!(keys.contains(&"alpha".to_string()));
        assert!(keys.contains(&"gamma".to_string()));
        assert!(!keys.contains(&"beta".to_string()));
        assert!(!keys.contains(&"delta".to_string()));
    }

    #[tokio::test]
    async fn test_extract_items_scalar_object_keys_excluded() {
        init_tracing();

        let mut source: HashMap<String, ScalarValue> = HashMap::new();
        source.insert("data".to_string(), {
            let mut inner = HashMap::new();
            let mut obj = HashMap::new();
            obj.insert("keep1".to_string(), ScalarValue::Bool(true));
            obj.insert("keep2".to_string(), ScalarValue::Bool(true));
            obj.insert("exclude1".to_string(), ScalarValue::Bool(false));
            obj.insert("exclude2".to_string(), ScalarValue::Bool(false));
            inner.insert("object".to_string(), scalar_value_from(obj).unwrap());
            scalar_value_from(inner).unwrap()
        });

        let context = ExecutionContext::new(Some(&source));

        // Exclude specific keys
        let ns_type = NamespaceType::Iterative {
            store_path: StorePath::from_segments(["data", "object"]),
            source: IteratorSource::ScalarObjectKeys {
                keys: Some(vec!["exclude1".to_string(), "exclude2".to_string()]),
                exclude: true,
            },
            iter_var: None,
            index_var: None,
        };

        let items = extract_items(&context, &ns_type).await.unwrap();
        assert_eq!(items.len(), 2);
        let keys: Vec<String> = items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        assert!(keys.contains(&"keep1".to_string()));
        assert!(keys.contains(&"keep2".to_string()));
        assert!(!keys.contains(&"exclude1".to_string()));
        assert!(!keys.contains(&"exclude2".to_string()));
    }

    #[tokio::test]
    async fn test_extract_items_tabular_column() {
        init_tracing();

        use polars::prelude::*;

        // Create a DataFrame with some duplicate values in a column
        let df = DataFrame::new(vec![
            Column::new(
                "category".into(),
                &["fruit", "vegetable", "fruit", "dairy", "vegetable"],
            ),
            Column::new(
                "item".into(),
                &["apple", "carrot", "banana", "milk", "broccoli"],
            ),
        ])
        .unwrap();

        let context = ExecutionContext::new(None);
        let store_path = StorePath::from_segments(["test", "df"]);
        context.tabular().insert(&store_path, df).await.unwrap();

        let ns_type = NamespaceType::Iterative {
            store_path: store_path.clone(),
            source: IteratorSource::TabularColumn {
                column: "category".to_string(),
                range: None,
            },
            iter_var: None,
            index_var: None,
        };

        let items = extract_items(&context, &ns_type).await.unwrap();
        // Should have 3 unique categories: fruit, vegetable, dairy
        assert_eq!(items.len(), 3);
        let categories: Vec<String> = items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        assert!(categories.contains(&"fruit".to_string()));
        assert!(categories.contains(&"vegetable".to_string()));
        assert!(categories.contains(&"dairy".to_string()));
    }

    #[tokio::test]
    async fn test_extract_items_tabular_column_with_range() {
        init_tracing();

        use polars::prelude::*;

        // Create a DataFrame with string values that will have unique entries
        let df = DataFrame::new(vec![
            Column::new("category".into(), &["a", "b", "c", "d", "e"]),
            Column::new("value".into(), &[1i64, 2, 3, 4, 5]),
        ])
        .unwrap();

        let context = ExecutionContext::new(None);
        let store_path = StorePath::from_segments(["test", "categories"]);
        context.tabular().insert(&store_path, df).await.unwrap();

        // Get only items 1-3 (indices 1, 2) - should get 2 items
        let ns_type = NamespaceType::Iterative {
            store_path: store_path.clone(),
            source: IteratorSource::TabularColumn {
                column: "category".to_string(),
                range: Some((1, 3)),
            },
            iter_var: None,
            index_var: None,
        };

        let items = extract_items(&context, &ns_type).await.unwrap();
        // Should get 2 items (indices 1 and 2 from the unique values)
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_extract_items_error_on_single_namespace() {
        init_tracing();

        let context = ExecutionContext::new(None);
        let ns_type = NamespaceType::Single;

        let result = extract_items(&context, &ns_type).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("non-iterative NamespaceType")
        );
    }

    #[tokio::test]
    async fn test_extract_items_error_key_not_found() {
        init_tracing();

        let context = ExecutionContext::new(None);

        let ns_type = NamespaceType::Iterative {
            store_path: StorePath::from_segments(["nonexistent", "key"]),
            source: IteratorSource::ScalarArray { range: None },
            iter_var: None,
            index_var: None,
        };

        let result = extract_items(&context, &ns_type).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
