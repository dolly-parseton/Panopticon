# Polars DataFrames

Panopticon uses [Polars](https://pola.rs/) for tabular data operations. Polars is a high-performance DataFrame library written in Rust, providing excellent performance for data manipulation tasks.

## TabularValue and TabularStore

### Type Definitions

```rust
// TabularValue is a type alias for Polars DataFrame
pub type TabularValue = polars::prelude::DataFrame;

// TabularStore manages DataFrames during pipeline execution
pub struct TabularStore {
    store: Arc<RwLock<HashMap<String, TabularValue>>>,
}
```

### Store Operations

The `TabularStore` provides async methods for managing DataFrames:

```rust
// Insert a DataFrame
ctx.tabular().insert(&path, dataframe).await?;

// Retrieve a DataFrame
let df: Option<TabularValue> = ctx.tabular().get(&path).await?;

// Remove a DataFrame
let removed: Option<TabularValue> = ctx.tabular().remove(&path).await?;

// List all stored paths
let keys: Vec<String> = ctx.tabular().keys().await;
```

## Loading Tabular Data

### FileCommand

Load data from CSV, JSON, or Parquet files:

```rust
let file_attrs = ObjectBuilder::new()
    .insert("files", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "products")
            .insert("file", "/path/to/products.csv")
            .insert("format", "csv")
            .build_scalar(),
        ObjectBuilder::new()
            .insert("name", "orders")
            .insert("file", "/path/to/orders.parquet")
            .insert("format", "parquet")
            .build_scalar(),
    ]))
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("data"))
    .await?
    .add_command::<FileCommand>("load", &file_attrs)
    .await?;
```

After execution, DataFrames are stored at paths like:
- `data.load.products.data` - The DataFrame
- `data.load.products.row_count` - Number of rows (scalar)
- `data.load.orders.data` - Another DataFrame

### SqlCommand

Query data using SQL:

```rust
let sql_attrs = ObjectBuilder::new()
    .insert("query", "SELECT * FROM products WHERE price > 100")
    .insert("sources", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "products")
            .insert("path", "data.load.products.data")
            .build_scalar(),
    ]))
    .build_hashmap();
```

## Aggregating Data

### AggregateCommand

Perform statistical aggregations on DataFrames:

```rust
let agg_attrs = ObjectBuilder::new()
    .insert("source", "data.load.products.data")
    .insert("aggregations", ScalarValue::Array(vec![
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
    ]))
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("stats"))
    .await?
    .add_command::<AggregateCommand>("products", &agg_attrs)
    .await?;
```

Supported aggregation operations:
- `count` - Row count (no column required)
- `sum` - Sum of column values
- `mean` - Average of column values
- `min` - Minimum value
- `max` - Maximum value
- `median` - Median value

## Accessing DataFrame Results

### During Execution

Commands can access DataFrames from the execution context:

```rust
async fn execute(&self, ctx: &ExecutionContext, source: &StorePath) -> Result<()> {
    // Get DataFrame from TabularStore
    let df_path = StorePath::from_segments(["data", "load", "products", "data"]);
    let df = ctx.tabular()
        .get(&df_path)
        .await?
        .context("DataFrame not found")?;

    // Work with the DataFrame
    println!("Columns: {:?}", df.get_column_names());
    println!("Shape: {:?}", df.shape());

    Ok(())
}
```

### After Execution

The `ResultStore` provides access to both scalar and tabular results:

```rust
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(ResultSettings::default()).await?;

for cmd_results in results.iter() {
    println!("Source: {}", cmd_results.source().to_dotted());

    // Iterate over metadata
    for (path, value) in cmd_results.meta_iter() {
        println!("  [meta] {} = {}", path.to_dotted(), value);
    }

    // Iterate over data results
    for (path, value) in cmd_results.data_iter() {
        match value.as_scalar() {
            Some((ty, val)) => {
                println!("  [data] {} = {} ({:?})", path.to_dotted(), val, ty);
            }
            None => {
                // Tabular results are exported to disk
                let (file_path, format, rows, cols) =
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
}
```

## Export Formats

DataFrames are automatically exported to disk when accessing results. Configure the format via `ResultSettings`:

```rust
// Export as CSV
let settings = ResultSettings::new()
    .with_format(TabularFormat::Csv)
    .with_output_path(PathBuf::from("/output/directory"));

// Export as JSON
let settings = ResultSettings::new()
    .with_format(TabularFormat::Json);

// Export as Parquet (default)
let settings = ResultSettings::new()
    .with_format(TabularFormat::Parquet);

let results = completed.results(settings).await?;
```

### TabularFormat Options

| Format | Extension | Use Case |
|--------|-----------|----------|
| `TabularFormat::Csv` | `.csv` | Human-readable, spreadsheet compatible |
| `TabularFormat::Json` | `.json` | Web applications, APIs |
| `TabularFormat::Parquet` | `.parquet` | Efficient storage, data pipelines |

## Data Flow Diagram

```
Tabular Data Flow:
==================

+------------------+
|  Input Files     |
|  - products.csv  |
|  - orders.json   |
+--------+---------+
         |
         v
+------------------+
|   FileCommand    |
+--------+---------+
         |
         v
+------------------+
|  TabularStore    |
|                  |
| data.load.       |
|   products.data  |  <-- DataFrame
|   orders.data    |  <-- DataFrame
+--------+---------+
         |
    +----+----+
    |         |
    v         v
+-------+  +------------+
| SQL   |  | Aggregate  |
+---+---+  +-----+------+
    |            |
    v            v
+------------------+
|  TabularStore    |
| (updated)        |
+--------+---------+
         |
         v
+------------------+
|   ResultStore    |
+--------+---------+
         |
         v
+------------------+
|  Output Files    |
|  - .csv          |
|  - .json         |
|  - .parquet      |
+------------------+
```

## Working with Polars Directly

When building custom commands, you can use Polars DataFrame operations:

```rust
use polars::prelude::*;

// Create a DataFrame
let df = df!(
    "name" => &["Alice", "Bob", "Charlie"],
    "age" => &[30, 25, 35],
    "city" => &["NYC", "LA", "Chicago"]
)?;

// Filter rows
let filtered = df.clone().lazy()
    .filter(col("age").gt(lit(28)))
    .collect()?;

// Select columns
let selected = df.clone().lazy()
    .select([col("name"), col("city")])
    .collect()?;

// Group and aggregate
let grouped = df.clone().lazy()
    .group_by([col("city")])
    .agg([
        col("age").mean().alias("avg_age"),
        col("name").count().alias("count"),
    ])
    .collect()?;

// Store in TabularStore
ctx.tabular().insert(&source.with_segment("result"), filtered).await?;
```

## Complete Example

```rust
use panopticon_core::prelude::*;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let output_dir = tempfile::tempdir()?;
    let mut pipeline = Pipeline::new();

    // --- Load product data ---
    let file_attrs = ObjectBuilder::new()
        .insert("files", ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "products")
                .insert("file", "/path/to/products.csv")
                .insert("format", "csv")
                .build_scalar(),
        ]))
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("data"))
        .await?
        .add_command::<FileCommand>("load", &file_attrs)
        .await?;

    // --- Aggregate: compute statistics ---
    let agg_attrs = ObjectBuilder::new()
        .insert("source", "data.load.products.data")
        .insert("aggregations", ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "row_count")
                .insert("op", "count")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "total_price")
                .insert("column", "price")
                .insert("op", "sum")
                .build_scalar(),
        ]))
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("stats"))
        .await?
        .add_command::<AggregateCommand>("products", &agg_attrs)
        .await?;

    // --- Execute with custom output path ---
    let completed = pipeline.compile().await?.execute().await?;
    let settings = ResultSettings::new()
        .with_output_path(output_dir.path().to_path_buf())
        .with_format(TabularFormat::Json);
    let results = completed.results(settings).await?;

    // --- Access results ---
    println!("=== Result Store ({} command(s)) ===\n", results.len());

    for cmd_results in results.iter() {
        println!("Source: {}", cmd_results.source().to_dotted());

        for (path, value) in cmd_results.data_iter() {
            match value.as_scalar() {
                Some((ty, val)) => {
                    println!("  {} = {} ({:?})", path.to_dotted(), val, ty);
                }
                None => {
                    let (file_path, fmt, rows, cols) =
                        value.as_tabular().expect("Expected tabular");
                    println!(
                        "  {} => {} ({} rows x {} cols)",
                        path.to_dotted(),
                        file_path.display(),
                        rows,
                        cols
                    );
                }
            }
        }
    }

    Ok(())
}
```

## Reference Links

- [Polars Documentation](https://pola.rs/)
- [Polars Rust API](https://docs.rs/polars/latest/polars/)
- [Polars User Guide](https://pola-rs.github.io/polars/user-guide/)

## Next Steps

Explore [Pipeline Patterns](../pipeline-patterns/index.md) to learn about iteration, conditional execution, and advanced pipeline techniques.
