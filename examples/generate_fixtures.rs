use polars::prelude::*;
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a simple DataFrame
    let df = df!(
        "id" => &[1i64, 2, 3],
        "category" => &["A", "B", "A"],
        "value" => &[100.5, 200.75, 150.25]
    )?;

    // Write to parquet
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/metrics.parquet");
    let file = File::create(path)?;
    ParquetWriter::new(file).finish(&mut df.clone())?;

    println!("Created {}", path);
    Ok(())
}
