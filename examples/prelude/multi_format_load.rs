//! Example: Loading CSV, JSON, and Parquet files
//!
//! Demonstrates FileCommand with all three supported formats, then uses
//! SqlCommand to join data across the loaded tables.
//!
//! Run with: cargo run --example multi_format_load

use panopticon_core::prelude::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // --- Load files in three different formats ---
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
                ObjectBuilder::new()
                    .insert("name", "events")
                    .insert(
                        "file",
                        fixtures_dir()
                            .join("events.json")
                            .to_string_lossy()
                            .to_string(),
                    )
                    .insert("format", "json")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "metrics")
                    .insert(
                        "file",
                        fixtures_dir()
                            .join("metrics.parquet")
                            .to_string_lossy()
                            .to_string(),
                    )
                    .insert("format", "parquet")
                    .build_scalar(),
            ]),
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("data"))
        .await?
        .add_command::<FileCommand>("load", &file_attrs)
        .await?;

    // --- SQL join: users x events ---
    // Cross-join to pair each user with each event (small fixture data)
    let join_attrs = ObjectBuilder::new()
        .insert(
            "tables",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "users")
                    .insert("source", "data.load.users.data")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "events")
                    .insert("source", "data.load.events.data")
                    .build_scalar(),
            ]),
        )
        .insert(
            "query",
            "SELECT u.name, u.email, e.type AS event_type, e.timestamp \
             FROM users u CROSS JOIN events e \
             ORDER BY u.name, e.timestamp",
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("joined"))
        .await?
        .add_command::<SqlCommand>("user_events", &join_attrs)
        .await?;

    // --- Execute ---
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // --- Print file-load summary ---
    let data_source = StorePath::from_segments(["data", "load"]);
    let data_results = results
        .get_by_source(&data_source)
        .expect("Expected data.load results");

    let file_count = data_results
        .meta_get(&data_source.with_segment("count"))
        .expect("Expected count");
    let total_rows = data_results
        .meta_get(&data_source.with_segment("total_rows"))
        .expect("Expected total_rows");
    println!(
        "Loaded {} files ({} total rows across all formats)",
        file_count, total_rows
    );

    let total_size = data_results
        .meta_get(&data_source.with_segment("total_size"))
        .expect("Expected total_size");
    println!("  Total size: {} bytes", total_size);

    // Per-file tabular data is available in the execution context at paths like
    // "data.load.users.data", which SqlCommand references as table sources above.

    // --- Print join result summary ---
    let join_source = StorePath::from_segments(["joined", "user_events"]);
    let join_results = results
        .get_by_source(&join_source)
        .expect("Expected joined.user_events results");

    let join_rows = join_results
        .meta_get(&join_source.with_segment("rows"))
        .expect("Expected rows");
    let join_cols = join_results
        .meta_get(&join_source.with_segment("columns"))
        .expect("Expected columns");
    println!(
        "\nCross-join result: {} rows, columns: {}",
        join_rows, join_cols
    );

    Ok(())
}
