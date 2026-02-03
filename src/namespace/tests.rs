use super::*;
use crate::namespace::sealed::Build;
use crate::test_utils::init_tracing;

#[tokio::test]
async fn test_extract_items_scalar_array() {
    init_tracing();

    let inputs_ns = NamespaceBuilder::new("data")
        .static_ns()
        .insert(
            "items",
            ScalarValue::Array(vec![
                ScalarValue::String("apple".to_string()),
                ScalarValue::String("banana".to_string()),
                ScalarValue::String("cherry".to_string()),
                ScalarValue::String("date".to_string()),
            ]),
        )
        .build()
        .unwrap();

    let context = ExecutionContext::default();

    // Insert the namespace values into the context scalar store
    if let ExecutionMode::Static { values } = inputs_ns.ty() {
        for (key, value) in values {
            let store_path = StorePath::from_segments([inputs_ns.name(), key]);
            context
                .scalar()
                .insert(&store_path, value.clone())
                .await
                .unwrap();
        }
    }

    let ns_type = ExecutionMode::Iterative {
        store_path: StorePath::from_segments(["data", "items"]),
        source: IteratorType::ScalarArray { range: None },
        iter_var: None,
        index_var: None,
    };

    let items = ns_type.resolve_iter_values(&context).await.unwrap();
    assert_eq!(items.len(), 4);
    assert_eq!(items[0], ScalarValue::String("apple".to_string()));
    assert_eq!(items[1], ScalarValue::String("banana".to_string()));
    assert_eq!(items[2], ScalarValue::String("cherry".to_string()));
    assert_eq!(items[3], ScalarValue::String("date".to_string()));
}

#[tokio::test]
async fn test_extract_items_scalar_array_with_range() {
    init_tracing();

    let inputs_ns = NamespaceBuilder::new("data")
        .static_ns()
        .insert(
            "items",
            ScalarValue::Array(vec![
                ScalarValue::String("a".to_string()),
                ScalarValue::String("b".to_string()),
                ScalarValue::String("c".to_string()),
                ScalarValue::String("d".to_string()),
                ScalarValue::String("e".to_string()),
            ]),
        )
        .build()
        .unwrap();

    let context = ExecutionContext::default();

    // Insert the namespace values into the context scalar store
    if let ExecutionMode::Static { values } = inputs_ns.ty() {
        for (key, value) in values {
            let store_path = StorePath::from_segments([inputs_ns.name(), key]);
            context
                .scalar()
                .insert(&store_path, value.clone())
                .await
                .unwrap();
        }
    }

    // Test range (1, 4) - should get items at index 1, 2, 3 ("b", "c", "d")
    let ns_type = ExecutionMode::Iterative {
        store_path: StorePath::from_segments(["data", "items"]),
        source: IteratorType::ScalarArray {
            range: Some((1, 4)),
        },
        iter_var: None,
        index_var: None,
    };

    let items = ns_type.resolve_iter_values(&context).await.unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0], ScalarValue::String("b".to_string()));
    assert_eq!(items[1], ScalarValue::String("c".to_string()));
    assert_eq!(items[2], ScalarValue::String("d".to_string()));
}

#[tokio::test]
async fn test_extract_items_string_split() {
    init_tracing();

    let inputs_ns = NamespaceBuilder::new("source")
        .static_ns()
        .insert("csv", ScalarValue::String("one,two,three".to_string()))
        .build()
        .unwrap();

    let context = ExecutionContext::default();

    // Insert the namespace values into the context scalar store
    if let ExecutionMode::Static { values } = inputs_ns.ty() {
        for (key, value) in values {
            let store_path = StorePath::from_segments([inputs_ns.name(), key]);
            context
                .scalar()
                .insert(&store_path, value.clone())
                .await
                .unwrap();
        }
    }

    println!("Context created");

    let ns_type = ExecutionMode::Iterative {
        store_path: StorePath::from_segments(["source", "csv"]),
        source: IteratorType::ScalarStringSplit {
            delimiter: ",".to_string(),
        },
        iter_var: None,
        index_var: None,
    };

    println!("NamespaceType created");

    let items = ns_type.resolve_iter_values(&context).await.unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0], ScalarValue::String("one".to_string()));
}

#[tokio::test]
async fn test_extract_items_scalar_object_keys_all() {
    init_tracing();

    let inputs_ns = NamespaceBuilder::new("data")
        .static_ns()
        .object("object", |o| {
            o.insert("key1", ScalarValue::String("value1".to_string()))
                .insert("key2", ScalarValue::String("value2".to_string()))
                .insert("key3", ScalarValue::String("value3".to_string()))
        })
        .build()
        .unwrap();

    let context = ExecutionContext::default();

    // Insert the namespace values into the context scalar store
    if let ExecutionMode::Static { values } = inputs_ns.ty() {
        for (key, value) in values {
            let store_path = StorePath::from_segments([inputs_ns.name(), key]);
            context
                .scalar()
                .insert(&store_path, value.clone())
                .await
                .unwrap();
        }
    }

    // Get all keys (no filter)
    let ns_type = ExecutionMode::Iterative {
        store_path: StorePath::from_segments(["data", "object"]),
        source: IteratorType::ScalarObjectKeys {
            keys: None,
            exclude: false,
        },
        iter_var: None,
        index_var: None,
    };

    let items = ns_type.resolve_iter_values(&context).await.unwrap();
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

    let inputs_ns = NamespaceBuilder::new("data")
        .static_ns()
        .object("object", |o| {
            o.insert("alpha", ScalarValue::Number(1.into()))
                .insert("beta", ScalarValue::Number(2.into()))
                .insert("gamma", ScalarValue::Number(3.into()))
                .insert("delta", ScalarValue::Number(4.into()))
        })
        .build()
        .unwrap();

    let context = ExecutionContext::default();

    // Insert the namespace values into the context scalar store
    if let ExecutionMode::Static { values } = inputs_ns.ty() {
        for (key, value) in values {
            let store_path = StorePath::from_segments([inputs_ns.name(), key]);
            context
                .scalar()
                .insert(&store_path, value.clone())
                .await
                .unwrap();
        }
    }

    // Include only specific keys
    let ns_type = ExecutionMode::Iterative {
        store_path: StorePath::from_segments(["data", "object"]),
        source: IteratorType::ScalarObjectKeys {
            keys: Some(vec!["alpha".to_string(), "gamma".to_string()]),
            exclude: false,
        },
        iter_var: None,
        index_var: None,
    };

    let items = ns_type.resolve_iter_values(&context).await.unwrap();
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

    let inputs_ns = NamespaceBuilder::new("data")
        .static_ns()
        .object("object", |o| {
            o.insert("keep1", ScalarValue::Bool(true))
                .insert("keep2", ScalarValue::Bool(true))
                .insert("exclude1", ScalarValue::Bool(false))
                .insert("exclude2", ScalarValue::Bool(false))
        })
        .build()
        .unwrap();

    let context = ExecutionContext::default();

    // Insert the namespace values into the context scalar store
    if let ExecutionMode::Static { values } = inputs_ns.ty() {
        for (key, value) in values {
            let store_path = StorePath::from_segments([inputs_ns.name(), key]);
            context
                .scalar()
                .insert(&store_path, value.clone())
                .await
                .unwrap();
        }
    }

    // Exclude specific keys
    let ns_type = ExecutionMode::Iterative {
        store_path: StorePath::from_segments(["data", "object"]),
        source: IteratorType::ScalarObjectKeys {
            keys: Some(vec!["exclude1".to_string(), "exclude2".to_string()]),
            exclude: true,
        },
        iter_var: None,
        index_var: None,
    };

    let items = ns_type.resolve_iter_values(&context).await.unwrap();
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

    let context = ExecutionContext::default();
    let store_path = StorePath::from_segments(["test", "df"]);
    context.tabular().insert(&store_path, df).await.unwrap();

    let ns_type = ExecutionMode::Iterative {
        store_path: store_path.clone(),
        source: IteratorType::TabularColumn {
            column: "category".to_string(),
            range: None,
        },
        iter_var: None,
        index_var: None,
    };

    let items = ns_type.resolve_iter_values(&context).await.unwrap();
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

    let context = ExecutionContext::default();
    let store_path = StorePath::from_segments(["test", "categories"]);
    context.tabular().insert(&store_path, df).await.unwrap();

    // Get only items 1-3 (indices 1, 2) - should get 2 items
    let ns_type = ExecutionMode::Iterative {
        store_path: store_path.clone(),
        source: IteratorType::TabularColumn {
            column: "category".to_string(),
            range: Some((1, 3)),
        },
        iter_var: None,
        index_var: None,
    };

    let items = ns_type.resolve_iter_values(&context).await.unwrap();
    // Should get 2 items (indices 1 and 2 from the unique values)
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn test_extract_items_error_on_single_namespace() {
    init_tracing();

    let context = ExecutionContext::default();
    let ns_type = ExecutionMode::Once;

    let result = ns_type.resolve_iter_values(&context).await;
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

    let context = ExecutionContext::default();

    let ns_type = ExecutionMode::Iterative {
        store_path: StorePath::from_segments(["nonexistent", "key"]),
        source: IteratorType::ScalarArray { range: None },
        iter_var: None,
        index_var: None,
    };

    let result = ns_type.resolve_iter_values(&context).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn test_insert_raw_renders_in_tera_template() {
    init_tracing();
    use crate::values::scalar::ScalarStore;

    let store = ScalarStore::new();

    // insert_raw should place a value directly into the Tera context
    store
        .insert_raw("item", ScalarValue::String("hello".to_string()))
        .await
        .unwrap();
    store.insert_raw("idx", to_scalar::i64(42)).await.unwrap();

    // Verify both render correctly in templates
    let result = store.render_template("{{ item }}").await.unwrap();
    assert_eq!(result, "hello");

    let result = store.render_template("{{ idx }}").await.unwrap();
    assert_eq!(result, "42");

    // Verify removal works (remove uses namespace() which is first segment)
    store
        .remove(&StorePath::from_segments(["item"]))
        .await
        .unwrap();

    // After removal, rendering should fail because the variable is gone
    let result = store.render_template("{{ item }}").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_iterative_pipeline_indexed_outputs() {
    init_tracing();
    use crate::commands::template::TemplateCommand;

    let temp_dir = tempfile::tempdir().unwrap();

    let mut pipeline = Pipeline::new();

    // Static source data
    pipeline
        .add_namespace(NamespaceBuilder::new("source").static_ns().insert(
            "items",
            ScalarValue::Array(vec![
                ScalarValue::String("alpha".to_string()),
                ScalarValue::String("beta".to_string()),
            ]),
        ))
        .await
        .unwrap();

    // Iterative namespace with a TemplateCommand
    let attrs = ObjectBuilder::new()
        .insert(
            "templates",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "tpl")
                    .insert("content", "Value: {{ item }}")
                    .build_scalar(),
            ]),
        )
        .insert("render", "tpl")
        .insert(
            "output",
            format!("{}/out_{{{{ idx }}}}.txt", temp_dir.path().display()),
        )
        .insert("capture", true)
        .build_hashmap();

    let mut handle = pipeline
        .add_namespace(
            NamespaceBuilder::new("process")
                .iterative()
                .store_path(StorePath::from_segments(["source", "items"]))
                .scalar_array(None)
                .iter_var("item")
                .index_var("idx"),
        )
        .await
        .unwrap();
    handle
        .add_command::<TemplateCommand>("render", &attrs)
        .await
        .unwrap();

    let completed = pipeline.compile().await.unwrap().execute().await.unwrap();

    let results = completed
        .results(ResultSettings {
            output_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Each iteration should have its own indexed result entry
    let prefix_0 = StorePath::from_segments(["process", "render", "0"]);
    let prefix_1 = StorePath::from_segments(["process", "render", "1"]);

    let result_0 = results
        .get_by_source(&prefix_0)
        .expect("missing result for iteration 0");
    let result_1 = results
        .get_by_source(&prefix_1)
        .expect("missing result for iteration 1");

    let content_0 = result_0
        .data_get(&prefix_0.with_segment("content"))
        .expect("missing content for iteration 0");
    let content_1 = result_1
        .data_get(&prefix_1.with_segment("content"))
        .expect("missing content for iteration 1");

    let (_, value_0) = content_0.as_scalar().unwrap();
    let (_, value_1) = content_1.as_scalar().unwrap();
    assert_eq!(value_0.as_str().unwrap(), "Value: alpha");
    assert_eq!(value_1.as_str().unwrap(), "Value: beta");

    // Verify the old non-indexed path does NOT exist (no overwrite at root)
    let non_indexed = StorePath::from_segments(["process", "render"]);
    assert!(results.get_by_source(&non_indexed).is_none());
}
