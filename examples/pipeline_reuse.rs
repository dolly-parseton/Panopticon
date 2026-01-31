//! Example: Editing a completed pipeline for incremental building
//!
//! Demonstrates the Pipeline state machine lifecycle:
//!   Draft → Ready → Completed → (edit) → Draft → Ready → Completed
//!
//! First pass loads data and runs a query. Second pass adds an aggregation
//! namespace to the same pipeline and re-executes.
//!
//! Run with: cargo run --example pipeline_reuse

use panopticon_core::prelude::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ===== Pass 1: Load data and query =====
    println!("=== Pass 1: Load + Query ===\n");

    let mut pipeline = Pipeline::new();

    // Load users
    let file_attrs = ObjectBuilder::new()
        .insert(
            "files",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "users")
                    .insert(
                        "file",
                        fixtures_dir()
                            .join("users.csv")
                            .to_string_lossy()
                            .to_string(),
                    )
                    .insert("format", "csv")
                    .build_scalar(),
            ]),
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("data"))?
        .add_command::<FileCommand>("load", &file_attrs)?;

    // Query: all users sorted by age
    let sql_attrs = ObjectBuilder::new()
        .insert(
            "tables",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "users")
                    .insert("source", "data.load.users.data")
                    .build_scalar(),
            ]),
        )
        .insert("query", "SELECT name, age FROM users ORDER BY age DESC")
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("query"))?
        .add_command::<SqlCommand>("sorted", &sql_attrs)?;

    // Execute pass 1
    let completed = pipeline.compile()?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    let query_source = StorePath::from_segments(["query", "sorted"]);
    let query_results = results
        .get_by_source(&query_source)
        .expect("Expected query.sorted results");
    let rows = query_results
        .meta_get(&query_source.with_segment("rows"))
        .expect("Expected rows");
    let cols = query_results
        .meta_get(&query_source.with_segment("columns"))
        .expect("Expected columns");
    println!("  query.sorted: {} rows, columns: {}", rows, cols);
    println!("  Namespaces in pass 1: data, query");

    // ===== Pass 2: Edit pipeline, add aggregation, re-execute =====
    println!("\n=== Pass 2: Edit + Aggregate ===\n");

    // Return to Draft state
    let mut pipeline = completed.edit();

    // Add an aggregation namespace to the existing pipeline
    let agg_attrs = ObjectBuilder::new()
        .insert("source", "data.load.users.data")
        .insert(
            "aggregations",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "user_count")
                    .insert("op", "count")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "avg_age")
                    .insert("column", "age")
                    .insert("op", "mean")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "oldest")
                    .insert("column", "age")
                    .insert("op", "max")
                    .build_scalar(),
            ]),
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("stats"))?
        .add_command::<AggregateCommand>("users", &agg_attrs)?;

    // Re-compile and execute
    let completed = pipeline.compile()?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // Original query results are still present
    let query_source = StorePath::from_segments(["query", "sorted"]);
    let query_results = results
        .get_by_source(&query_source)
        .expect("query.sorted should still be present");
    let rows = query_results
        .meta_get(&query_source.with_segment("rows"))
        .expect("Expected rows");
    println!("  query.sorted: {} rows (still present)", rows);

    // New aggregation results
    let stats_source = StorePath::from_segments(["stats", "users"]);
    let stats_results = results
        .get_by_source(&stats_source)
        .expect("Expected stats.users results");

    for name in ["user_count", "avg_age", "oldest"] {
        let value = stats_results
            .data_get(&stats_source.with_segment(name))
            .and_then(|r| r.as_scalar())
            .unwrap_or_else(|| panic!("Expected {}", name));
        println!("  stats.users.{} = {}", name, value.1);
    }

    println!("  Namespaces in pass 2: data, query, stats");
    println!("\nPipeline successfully edited and re-executed.");

    Ok(())
}
