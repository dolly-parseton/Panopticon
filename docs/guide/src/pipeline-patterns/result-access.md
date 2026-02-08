# Result Access

**Problem**: After pipeline execution, you need to retrieve results, export tabular data, and iterate over outputs programmatically.

**Solution**: Use `ResultSettings` to configure output behavior and `ResultStore` to access and iterate over all command results.

## How Result Access Works

After calling `.execute()`, you have a `Pipeline<Completed>`. To access results:

1. Create `ResultSettings` to configure output path and format
2. Call `.results(settings)` to get a `ResultStore`
3. Use `ResultStore` methods to access individual command results
4. Each `CommandResults` contains metadata and data, accessible via iterators or direct lookup

## Basic Pattern: Configure and Collect

```rust
use panopticon_core::prelude::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let output_dir = tempfile::tempdir()?;
    let mut pipeline = Pipeline::new();

    // Load product data
    let file_attrs = ObjectBuilder::new()
        .insert(
            "files",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "products")
                    .insert("file", fixtures_dir().join("products.csv").to_string_lossy().to_string())
                    .insert("format", "csv")
                    .build_scalar(),
            ]),
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("data"))
        .await?
        .add_command::<FileCommand>("load", &file_attrs)
        .await?;

    // Compute aggregations
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
            ]),
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("stats"))
        .await?
        .add_command::<AggregateCommand>("products", &agg_attrs)
        .await?;

    // Execute pipeline
    let completed = pipeline.compile().await?.execute().await?;

    // Configure result settings
    let settings = ResultSettings::new()
        .with_output_path(output_dir.path().to_path_buf())
        .with_format(TabularFormat::Json);

    // Collect results
    let results = completed.results(settings).await?;

    // Iterate over all results
    println!("=== Result Store ({} command(s)) ===\n", results.len());

    for cmd_results in results.iter() {
        println!("Source: {}", cmd_results.source().to_dotted());

        // Print metadata
        for (path, value) in cmd_results.meta_iter() {
            println!("  [meta] {} = {}", path.to_dotted(), value);
        }

        // Print data
        for (path, value) in cmd_results.data_iter() {
            match value.as_scalar() {
                Some((ty, val)) => {
                    println!("  [data] {} = {} ({:?})", path.to_dotted(), val, ty);
                }
                None => {
                    let (file_path, fmt, rows, cols) =
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

    Ok(())
}
```

**Output**:
```
=== Result Store (2 command(s)) ===

Source: data.load
  [meta] data.load.products.rows = 10
  [meta] data.load.products.columns = ["name", "price", "quantity"]
  [data] data.load.products.data => /tmp/xxx/data_load_products.json (10 rows x 3 cols)

Source: stats.products
  [meta] stats.products.status = "completed"
  [data] stats.products.row_count = 10 (Int)
  [data] stats.products.total_price = 1250.50 (Float)
  [data] stats.products.avg_price = 125.05 (Float)
```

## ResultSettings

`ResultSettings` configures how results are collected and exported.

### Creating Settings

```rust
// Default settings
let settings = ResultSettings::default();

// Or use the builder
let settings = ResultSettings::new();
```

### Output Path

Specify where tabular data files are written:

```rust
let settings = ResultSettings::new()
    .with_output_path(PathBuf::from("/path/to/output"));
```

Default: `./panopticon_results` in the current working directory.

### Output Format

Choose the format for tabular data exports:

```rust
// JSON (default)
let settings = ResultSettings::new()
    .with_format(TabularFormat::Json);

// CSV
let settings = ResultSettings::new()
    .with_format(TabularFormat::Csv);

// Parquet (efficient binary format)
let settings = ResultSettings::new()
    .with_format(TabularFormat::Parquet);
```

### Excluded Commands

Skip specific commands when collecting results:

```rust
let settings = ResultSettings::new()
    .with_excluded_commands(vec![
        StorePath::from_segments(["debug", "verbose_log"]),
        StorePath::from_segments(["temp", "intermediate"]),
    ]);
```

Excluded commands do not appear in the `ResultStore` and their tabular data is not exported.

## ResultStore

The `ResultStore` contains all command results from the pipeline execution.

### Basic Access

```rust
let results = completed.results(settings).await?;

// Number of commands with results
let count = results.len();

// Check if empty
if results.is_empty() {
    println!("No results");
}
```

### Lookup by Source

Access a specific command's results by its store path:

```rust
let source = StorePath::from_segments(["namespace", "command"]);

if let Some(cmd_results) = results.get_by_source(&source) {
    // Process this command's results
}
```

### Iteration

Iterate over all command results:

```rust
for cmd_results in results.iter() {
    println!("Command: {}", cmd_results.source().to_dotted());
}
```

## CommandResults

Each `CommandResults` contains the outputs from a single command.

### Source Path

The path identifying this command:

```rust
let source: &StorePath = cmd_results.source();
println!("Command at: {}", source.to_dotted());
```

### Metadata Access

Metadata includes execution information like row counts, column names, and status:

```rust
// Direct lookup
let source = StorePath::from_segments(["data", "load"]);
let rows = cmd_results
    .meta_get(&source.with_segment("products").with_segment("rows"))
    .expect("Expected rows");

// Iterate all metadata
for (path, value) in cmd_results.meta_iter() {
    println!("{} = {}", path.to_dotted(), value);
}

// Get all metadata keys
for key in cmd_results.meta_keys() {
    println!("Meta key: {}", key.to_dotted());
}
```

### Data Access

Data includes the actual outputs (scalar values or tabular data references):

```rust
// Direct lookup
let source = StorePath::from_segments(["stats", "products"]);
let avg = cmd_results
    .data_get(&source.with_segment("avg_price"))
    .and_then(|r| r.as_scalar());

// Iterate all data
for (path, value) in cmd_results.data_iter() {
    match value.as_scalar() {
        Some((ty, val)) => println!("Scalar: {} = {}", path.to_dotted(), val),
        None => {
            let (file, fmt, rows, cols) = value.as_tabular().unwrap();
            println!("Tabular: {} -> {}", path.to_dotted(), file.display());
        }
    }
}

// Get all data keys
for key in cmd_results.data_keys() {
    println!("Data key: {}", key.to_dotted());
}
```

## ResultValue

Each data value is either scalar or tabular.

### Scalar Values

```rust
if let Some((ty, value)) = result_value.as_scalar() {
    // ty: &ScalarType (Int, Float, String, Bool, etc.)
    // value: &ScalarValue
    println!("Type: {:?}, Value: {}", ty, value);
}

// Type check
if result_value.is_scalar() {
    // ...
}
```

### Tabular Values

```rust
if let Some((path, format, rows, cols)) = result_value.as_tabular() {
    // path: &PathBuf - file location on disk
    // format: &TabularFormat - Csv, Json, or Parquet
    // rows: usize - row count
    // cols: usize - column count
    println!("File: {}, Format: {}, Shape: {}x{}",
             path.display(), format, rows, cols);
}

// Type check
if result_value.is_tabular() {
    // ...
}
```

## Patterns for Common Tasks

### Print Summary Report

```rust
println!("Pipeline Results Summary");
println!("========================");

for cmd_results in results.iter() {
    let source = cmd_results.source();
    print!("{}: ", source.to_dotted());

    // Check status
    if let Some(status) = cmd_results.meta_get(&source.with_segment("status")) {
        if status.as_str() == Some("skipped") {
            println!("SKIPPED");
            continue;
        }
    }

    // Count outputs
    let scalar_count = cmd_results.data_iter()
        .filter(|(_, v)| v.is_scalar())
        .count();
    let tabular_count = cmd_results.data_iter()
        .filter(|(_, v)| v.is_tabular())
        .count();

    println!("{} scalars, {} tables", scalar_count, tabular_count);
}
```

### Export All Tables to Directory

```rust
let settings = ResultSettings::new()
    .with_output_path(export_dir.to_path_buf())
    .with_format(TabularFormat::Parquet);

let results = completed.results(settings).await?;

// List exported files
println!("Exported files:");
for entry in std::fs::read_dir(&export_dir)? {
    let entry = entry?;
    let meta = entry.metadata()?;
    println!("  {} ({} bytes)",
             entry.file_name().to_string_lossy(),
             meta.len());
}
```

### Collect Scalar Metrics

```rust
let mut metrics: HashMap<String, f64> = HashMap::new();

for cmd_results in results.iter() {
    for (path, value) in cmd_results.data_iter() {
        if let Some((ScalarType::Float, scalar)) = value.as_scalar() {
            if let Some(f) = scalar.as_f64() {
                metrics.insert(path.to_dotted(), f);
            }
        }
    }
}

for (name, value) in &metrics {
    println!("{}: {:.2}", name, value);
}
```

### Handle Iterative Results

For iterative namespaces, results are indexed:

```rust
let mut iteration = 0;
loop {
    let source = StorePath::from_segments(["iterative_ns", "command"])
        .with_index(iteration);

    let Some(cmd_results) = results.get_by_source(&source) else {
        break; // No more iterations
    };

    println!("Iteration {}: {:?}", iteration, cmd_results.source().to_dotted());

    // Process this iteration's results...

    iteration += 1;
}

println!("Total iterations: {}", iteration);
```

## Best Practices

### Choose the Right Format

- **JSON**: Human-readable, good for debugging and small datasets
- **CSV**: Widely compatible, good for sharing with other tools
- **Parquet**: Efficient storage, good for large datasets and further processing

### Clean Up Output Directories

```rust
// Use tempdir for automatic cleanup
let output_dir = tempfile::tempdir()?;
let settings = ResultSettings::new()
    .with_output_path(output_dir.path().to_path_buf());

// output_dir is deleted when it goes out of scope
```

### Handle Missing Results Gracefully

```rust
let source = StorePath::from_segments(["maybe", "exists"]);

match results.get_by_source(&source) {
    Some(cmd_results) => {
        // Process results
    }
    None => {
        println!("Command {} not in results (possibly excluded or skipped)",
                 source.to_dotted());
    }
}
```

### Use Type-Safe Path Construction

Build paths systematically to avoid typos:

```rust
// Define base paths once
let stats_base = StorePath::from_segments(["stats", "products"]);

// Build specific paths from the base
let row_count = stats_base.with_segment("row_count");
let avg_price = stats_base.with_segment("avg_price");
let total = stats_base.with_segment("total_price");
```
