# Working with Data

This section covers how data flows through Panopticon pipelines and the tools available for manipulating and accessing that data.

## Data Flow Overview

In Panopticon, data flows between commands through two parallel storage systems:

```
+------------------+     +------------------+     +------------------+
|    Command A     |     |    Command B     |     |    Command C     |
+------------------+     +------------------+     +------------------+
         |                        |                        |
         v                        v                        v
+------------------------------------------------------------------------+
|                         ExecutionContext                               |
|  +----------------------------+  +----------------------------+        |
|  |       ScalarStore          |  |      TabularStore          |        |
|  |  (JSON-like values)        |  |  (Polars DataFrames)       |        |
|  +----------------------------+  +----------------------------+        |
+------------------------------------------------------------------------+
         ^                        ^                        ^
         |                        |                        |
   StorePath refs           StorePath refs           StorePath refs
```

### The Two Stores

Panopticon maintains two separate data stores during pipeline execution:

| Store | Type Alias | Contents | Use Case |
|-------|------------|----------|----------|
| **ScalarStore** | `ScalarValue` | JSON-compatible values (strings, numbers, booleans, arrays, objects) | Configuration, metadata, single values, template variables |
| **TabularStore** | `TabularValue` | Polars DataFrames | Structured data, CSV/JSON/Parquet files, SQL query results |

### StorePath: The Universal Reference

All data in both stores is addressed using `StorePath` - a dot-separated path that uniquely identifies values:

```rust
// Creating paths
let path = StorePath::from_segments(["namespace", "command", "field"]);
let child = path.with_segment("subfield");    // namespace.command.field.subfield
let indexed = path.with_index(0);             // namespace.command.field.0
```

StorePaths serve as:
- **Storage keys**: Where commands write their outputs
- **Dependency references**: How commands declare what data they need
- **Template variables**: Accessed via `{{ namespace.command.field }}` syntax
- **Result accessors**: How you retrieve data after pipeline execution

## Data Flow Example

Consider a pipeline that loads a CSV file and performs aggregations:

```
Pipeline Definition:
====================

Namespace: data                    Namespace: stats
+------------------+              +------------------+
| FileCommand      |              | AggregateCommand |
| name: "load"     | -----------> | source: "data.   |
|                  |              |   load.products. |
|                  |              |   data"          |
+------------------+              +------------------+

Data Flow:
==========

1. FileCommand executes:
   - Reads products.csv
   - Stores DataFrame at: data.load.products.data
   - Stores row count at: data.load.products.row_count

2. AggregateCommand executes:
   - Retrieves DataFrame from: data.load.products.data
   - Computes aggregations
   - Stores results at: stats.products.*
```

## Chapter Overview

This section contains three detailed chapters:

### [Store Paths](./store-paths.md)

Learn the `StorePath` API for creating, manipulating, and navigating data paths:
- `from_segments()` - Build paths from components
- `with_segment()` - Extend paths with new segments
- `with_index()` - Add numeric indices for iteration

### [Tera Templating](./tera-templating.md)

Master the Tera templating syntax used throughout Panopticon:
- Variable interpolation with `{{ path.to.value }}`
- Filters for transforming values
- Control structures for conditional content
- Template inheritance for complex outputs

### [Polars DataFrames](./polars-dataframes.md)

Work with tabular data using Polars:
- Understanding `TabularValue` (the DataFrame type alias)
- Accessing DataFrame results from the `TabularStore`
- Export formats: CSV, JSON, Parquet

## Quick Reference

### Accessing Data During Execution

Commands receive an `ExecutionContext` that provides access to both stores:

```rust
// In a command's execute method:
async fn execute(&self, ctx: &ExecutionContext, source: &StorePath) -> Result<()> {
    // Read scalar values
    let value = ctx.scalar().get(&some_path).await?;

    // Read tabular values
    let df = ctx.tabular().get(&some_path).await?;

    // Template substitution (uses ScalarStore)
    let rendered = ctx.substitute("Hello, {{ user.name }}!").await?;

    // Write results
    ctx.scalar().insert(&source.with_segment("output"), my_value).await?;
    ctx.tabular().insert(&source.with_segment("data"), my_df).await?;

    Ok(())
}
```

### Accessing Data After Execution

After a pipeline completes, use `ResultStore` to access outputs:

```rust
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(ResultSettings::default()).await?;

// Get results for a specific command
let source = StorePath::from_segments(["namespace", "command"]);
if let Some(cmd_results) = results.get_by_source(&source) {
    // Access scalar results
    if let Some(value) = cmd_results.data_get(&source.with_segment("field")) {
        if let Some((ty, scalar)) = value.as_scalar() {
            println!("Scalar: {:?} = {}", ty, scalar);
        }
    }

    // Access tabular results (exported to disk)
    if let Some(value) = cmd_results.data_get(&source.with_segment("data")) {
        if let Some((path, format, rows, cols)) = value.as_tabular() {
            println!("Table: {} ({} rows x {} cols)", path.display(), rows, cols);
        }
    }
}
```

## Next Steps

Continue to [Store Paths](./store-paths.md) to learn the details of the `StorePath` API.
