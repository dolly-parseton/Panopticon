//! Example: Iterative namespace with indexed output paths
//!
//! Demonstrates that iterative namespaces correctly:
//!   1. Insert iteration variables so Tera templates resolve them (not `[object]`)
//!   2. Store each iteration's command outputs at index-segmented paths
//!      (e.g., `process.render.0.content`, `process.render.1.content`)
//!
//! Run with: RUST_LOG=info cargo run --example iterative_tracing

use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        )
        .try_init()
        .ok();

    let temp_dir = tempfile::tempdir()?;

    let mut pipeline = Pipeline::new();

    // ─── Static namespace: source data to iterate over ───
    pipeline.add_namespace(
        NamespaceBuilder::new("source")
            .static_ns()
            .insert(
                "fruits",
                ScalarValue::Array(vec![
                    ScalarValue::String("apple".to_string()),
                    ScalarValue::String("banana".to_string()),
                    ScalarValue::String("cherry".to_string()),
                ]),
            ),
    )?;

    // ─── Iterative namespace: process each fruit ───
    let template_attrs = ObjectBuilder::new()
        .insert(
            "templates",
            ScalarValue::Array(vec![ObjectBuilder::new()
                .insert("name", "fruit_report")
                .insert("content", "Fruit #{{ idx }}: {{ item }}")
                .build_scalar()]),
        )
        .insert("render", "fruit_report")
        .insert(
            "output",
            format!(
                "{}/fruit_{{{{ idx }}}}.txt",
                temp_dir.path().display()
            ),
        )
        .insert("capture", true)
        .build_hashmap();

    let mut handle = pipeline.add_namespace(
        NamespaceBuilder::new("process")
            .iterative()
            .store_path(StorePath::from_segments(["source", "fruits"]))
            .scalar_array(None)
            .iter_var("item")
            .index_var("idx"),
    )?;
    handle.add_command::<TemplateCommand>("render", &template_attrs)?;

    // ─── Compile & Execute ───
    println!("=== Executing pipeline ===\n");
    let completed = pipeline.compile()?.execute().await?;

    // ─── Verify indexed outputs via ResultStore ───
    println!("=== Indexed output paths (via ResultStore) ===\n");

    let results = completed.results(ResultSettings::default()).await?;
    let fruits = ["apple", "banana", "cherry"];

    for i in 0..3 {
        let source = StorePath::from_segments(["process", "render"]).with_index(i);
        let cmd_results = results
            .get_by_source(&source)
            .unwrap_or_else(|| panic!("Expected results at index {}", i));

        // TemplateCommand stores "content" as Meta kind
        let content_path = source.with_segment("content");
        let content = cmd_results
            .meta_get(&content_path)
            .unwrap_or_else(|| panic!("Expected content meta at index {}", i));

        let expected = format!("Fruit #{}: {}", i, fruits[i]);
        assert_eq!(content.as_str().unwrap(), expected);
        println!("  process.render.{}.content = {:?}", i, content);
    }

    // ─── Verify the non-indexed path has no result entry ───
    let non_indexed = StorePath::from_segments(["process", "render"]);
    assert!(
        results.get_by_source(&non_indexed).is_none(),
        "Non-indexed source path should not have results"
    );
    println!("\n  process.render (non-indexed) = None  [correct]");

    // ─── Verify files on disk ───
    println!("\n=== Files on disk ===\n");
    for i in 0..3 {
        let path = temp_dir.path().join(format!("fruit_{}.txt", i));
        let content = tokio::fs::read_to_string(&path).await?;
        println!("  {} => {:?}", path.display(), content);
    }

    println!("\nAll assertions passed.");
    Ok(())
}
