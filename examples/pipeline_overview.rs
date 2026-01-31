//! Example: Pipeline overview using multiple commands
//!
//! Demonstrates the core workflow:
//!   1. Load CSV fixtures with FileCommand
//!   2. Query loaded data with SqlCommand
//!   3. Aggregate statistics with AggregateCommand
//!   4. Branch on results with ConditionCommand
//!
//! Run with: cargo run --example pipeline_overview

use panopticon_core::prelude::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // --- Static namespace: provide input values available to all commands via Tera substitution ---
    pipeline.add_namespace(
        NamespaceBuilder::new("config")
            .static_ns()
            .insert("min_age", ScalarValue::Number(28.into())),
    )?;

    // --- Namespace "data": load CSV fixtures ---
    let file_attrs = ObjectBuilder::new()
        .insert(
            "files",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "users")
                    .insert(
                        "file",
                        fixtures_dir().join("users.csv").to_string_lossy().to_string(),
                    )
                    .insert("format", "csv")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "products")
                    .insert(
                        "file",
                        fixtures_dir().join("products.csv").to_string_lossy().to_string(),
                    )
                    .insert("format", "csv")
                    .build_scalar(),
            ]),
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("data"))?
        .add_command::<FileCommand>("load", &file_attrs)?;

    // --- Namespace "query": run SQL against loaded data ---
    // Filter users whose age exceeds the config threshold (Tera substitution on the query string)
    let sql_attrs = ObjectBuilder::new()
        .insert(
            "tables",
            ScalarValue::Array(vec![ObjectBuilder::new()
                .insert("name", "users")
                .insert("source", "data.load.users.data")
                .build_scalar()]),
        )
        .insert("query", "SELECT * FROM users WHERE age > {{ config.min_age }}")
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("query"))?
        .add_command::<SqlCommand>("senior_users", &sql_attrs)?;

    // --- Namespace "stats": aggregate product data ---
    let agg_attrs = ObjectBuilder::new()
        .insert("source", "data.load.products.data")
        .insert(
            "aggregations",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "product_count")
                    .insert("op", "count")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "total_value")
                    .insert("column", "price")
                    .insert("op", "sum")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "avg_price")
                    .insert("column", "price")
                    .insert("op", "mean")
                    .build_scalar(),
            ]),
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("stats"))?
        .add_command::<AggregateCommand>("products", &agg_attrs)?;

    // --- Namespace "check": conditional logic based on query results ---
    let cond_attrs = ObjectBuilder::new()
        .insert(
            "branches",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("if", "query.senior_users.rows > 1")
                    .insert("then", "Multiple senior users found ({{ query.senior_users.rows }})")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("if", "query.senior_users.rows == 1")
                    .insert("then", "Exactly one senior user found")
                    .build_scalar(),
            ]),
        )
        .insert("default", "No senior users found")
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("check"))?
        .add_command::<ConditionCommand>("senior_status", &cond_attrs)?;

    // --- Compile and execute the pipeline ---
    let completed = pipeline.compile()?.execute().await?;

    // --- Read results via ResultStore ---
    let results = completed.results(ResultSettings::default()).await?;

    // File loading summary
    let data_source = StorePath::from_segments(["data", "load"]);
    let data_results = results.get_by_source(&data_source).expect("Expected data.load results");
    let file_count = data_results
        .meta_get(&data_source.with_segment("count"))
        .expect("Expected file count");
    println!("Files loaded: {}", file_count);

    // SQL query results
    let query_source = StorePath::from_segments(["query", "senior_users"]);
    let query_results = results
        .get_by_source(&query_source)
        .expect("Expected query.senior_users results");
    let senior_rows = query_results
        .meta_get(&query_source.with_segment("rows"))
        .expect("Expected rows");
    let senior_cols = query_results
        .meta_get(&query_source.with_segment("columns"))
        .expect("Expected columns");
    println!(
        "Senior users (age > 28): {} rows, columns: {}",
        senior_rows, senior_cols
    );

    // Aggregation results
    let stats_source = StorePath::from_segments(["stats", "products"]);
    let stats_results = results
        .get_by_source(&stats_source)
        .expect("Expected stats.products results");
    let product_count = stats_results
        .data_get(&stats_source.with_segment("product_count"))
        .and_then(|r| r.as_scalar())
        .expect("Expected product_count");
    let total_value = stats_results
        .data_get(&stats_source.with_segment("total_value"))
        .and_then(|r| r.as_scalar())
        .expect("Expected total_value");
    let avg_price = stats_results
        .data_get(&stats_source.with_segment("avg_price"))
        .and_then(|r| r.as_scalar())
        .expect("Expected avg_price");
    println!(
        "Products: count={}, total_value={}, avg_price={}",
        product_count.1, total_value.1, avg_price.1
    );

    // Condition result
    let check_source = StorePath::from_segments(["check", "senior_status"]);
    let check_results = results
        .get_by_source(&check_source)
        .expect("Expected check.senior_status results");
    let status = check_results
        .data_get(&check_source.with_segment("result"))
        .and_then(|r| r.as_scalar())
        .expect("Expected result");
    let matched = check_results
        .data_get(&check_source.with_segment("matched"))
        .and_then(|r| r.as_scalar())
        .expect("Expected matched");
    println!("Status: {} (matched: {})", status.1, matched.1);

    Ok(())
}
