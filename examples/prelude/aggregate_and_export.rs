//! Example: Aggregation operations and result export
//!
//! Loads products.csv, runs a variety of aggregation operations, and exports
//! results to a temporary directory. Walks the ResultStore to print a summary.
//!
//! Run with: cargo run --example aggregate_and_export

use panopticon_core::prelude::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let output_dir = tempfile::tempdir()?;
    let mut pipeline = Pipeline::new();

    // --- Load product data ---
    let file_attrs = ObjectBuilder::new()
        .insert(
            "files",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "products")
                    .insert(
                        "file",
                        fixtures_dir()
                            .join("products.csv")
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

    // --- Aggregate: multiple operations on product columns ---
    let agg_attrs = ObjectBuilder::new()
        .insert("source", "data.load.products.data")
        .insert(
            "aggregations",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "row_count")
                    .insert("op", "count")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "total_price")
                    .insert("column", "price")
                    .insert("op", "sum")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "avg_price")
                    .insert("column", "price")
                    .insert("op", "mean")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "max_quantity")
                    .insert("column", "quantity")
                    .insert("op", "max")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "min_quantity")
                    .insert("column", "quantity")
                    .insert("op", "min")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "median_price")
                    .insert("column", "price")
                    .insert("op", "median")
                    .build_scalar(),
            ]),
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("stats"))?
        .add_command::<AggregateCommand>("products", &agg_attrs)?;

    // --- Execute with custom output path ---
    let completed = pipeline.compile()?.execute().await?;
    let settings = ResultSettings::new().with_output_path(output_dir.path().to_path_buf());
    let results = completed.results(settings).await?;

    // --- Walk all results ---
    println!("=== Result Store ({} command(s)) ===\n", results.len());

    for cmd_results in results.iter() {
        println!("Source: {}", cmd_results.source().to_dotted());

        // Metadata
        for (path, value) in cmd_results.meta_iter() {
            println!("  [meta] {} = {}", path.to_dotted(), value);
        }

        // Data
        for (path, value) in cmd_results.data_iter() {
            match value.as_scalar() {
                Some((ty, val)) => {
                    println!("  [data] {} = {} ({:?})", path.to_dotted(), val, ty);
                }
                None => {
                    let (file_path, _fmt, rows, cols) =
                        value.as_tabular().expect("Expected tabular result");
                    println!(
                        "  [data] {} => {} ({} rows x {} cols)",
                        path.to_dotted(),
                        file_path.display(),
                        rows,
                        cols
                    );
                }
            }
        }
        println!();
    }

    // --- Verify output files exist on disk ---
    println!("Output directory: {}", output_dir.path().display());
    for entry in std::fs::read_dir(output_dir.path())? {
        let entry = entry?;
        let meta = entry.metadata()?;
        println!(
            "  {} ({} bytes)",
            entry.file_name().to_string_lossy(),
            meta.len()
        );
    }

    Ok(())
}
