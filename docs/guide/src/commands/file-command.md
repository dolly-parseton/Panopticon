# FileCommand

`FileCommand` loads data files from disk into the tabular store. It supports CSV, JSON, and Parquet formats.

## When to Use

Use `FileCommand` when you need to:

- Load one or more data files into a pipeline
- Ingest data in different formats (CSV, JSON, Parquet)
- Make tabular data available for SQL queries or aggregations

## Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `files` | Array of objects | Yes | Array of file specifications to load |

### File Object Fields

Each object in the `files` array has the following fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | Yes | Identifier for this file in the tabular store |
| `file` | String | Yes | Path to the file (supports Tera substitution) |
| `format` | String | Yes | File format: `csv`, `json`, or `parquet` (supports Tera substitution) |

## Results

### Meta Results

| Result | Type | Description |
|--------|------|-------------|
| `count` | Number | Total number of files loaded |
| `total_rows` | Number | Sum of rows across all loaded files |
| `total_size` | Number | Sum of file sizes in bytes |

### Data Results (Per File)

For each file in the `files` array, the following results are produced under `{output_prefix}.{name}`:

| Result | Type | Description |
|--------|------|-------------|
| `data` | Tabular (DataFrame) | The loaded data |
| `rows` | Number | Row count for this file |
| `size` | Number | File size in bytes |
| `columns` | Array | Column names in the loaded data |

## Examples

### Loading a Single CSV File

```rust
use panopticon_core::prelude::*;

let attrs = ObjectBuilder::new()
    .insert(
        "files",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "users")
                .insert("file", "/path/to/users.csv")
                .insert("format", "csv")
                .build_scalar(),
        ]),
    )
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("data"))
    .await?
    .add_command::<FileCommand>("load", &attrs)
    .await?;

// After execution, the data is available at:
// - data.load.users.data      (the DataFrame)
// - data.load.users.rows      (row count)
// - data.load.users.columns   (column names)
```

### Loading Multiple Formats

Load CSV, JSON, and Parquet files in a single command:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "files",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "users")
                .insert("file", "fixtures/users.csv")
                .insert("format", "csv")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "events")
                .insert("file", "fixtures/events.json")
                .insert("format", "json")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "metrics")
                .insert("file", "fixtures/metrics.parquet")
                .insert("format", "parquet")
                .build_scalar(),
        ]),
    )
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("data"))
    .await?
    .add_command::<FileCommand>("load", &attrs)
    .await?;
```

After execution:
- `data.load.users.data` - DataFrame from users.csv
- `data.load.events.data` - DataFrame from events.json
- `data.load.metrics.data` - DataFrame from metrics.parquet
- `data.load.count` - 3 (number of files loaded)
- `data.load.total_rows` - Combined row count

### Using Tera Substitution for Dynamic Paths

```rust
// First, set up a static namespace with configuration
pipeline
    .add_namespace(
        NamespaceBuilder::new("config")
            .static_ns()
            .insert("data_dir", ScalarValue::String("/var/data".to_string()))
            .insert("file_format", ScalarValue::String("csv".to_string())),
    )
    .await?;

// Then reference those values in FileCommand
let attrs = ObjectBuilder::new()
    .insert(
        "files",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "daily_report")
                .insert("file", "{{ config.data_dir }}/report.{{ config.file_format }}")
                .insert("format", "{{ config.file_format }}")
                .build_scalar(),
        ]),
    )
    .build_hashmap();
```

## Accessing Results

```rust
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(ResultSettings::default()).await?;

let source = StorePath::from_segments(["data", "load"]);
let cmd_results = results.get_by_source(&source).expect("Expected results");

// Access meta results
let file_count = cmd_results
    .meta_get(&source.with_segment("count"))
    .expect("Expected count");
let total_rows = cmd_results
    .meta_get(&source.with_segment("total_rows"))
    .expect("Expected total_rows");

println!("Loaded {} files with {} total rows", file_count, total_rows);

// Access per-file meta
let users_rows = cmd_results
    .meta_get(&StorePath::from_dotted("data.load.users.rows"))
    .expect("Expected users rows");
```

## Common Patterns

### FileCommand + SqlCommand

Load data with `FileCommand`, then query it with [SqlCommand](./sql-command.md):

```rust
// Load
let file_attrs = ObjectBuilder::new()
    .insert("files", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "orders")
            .insert("file", "orders.csv")
            .insert("format", "csv")
            .build_scalar(),
    ]))
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("data"))
    .await?
    .add_command::<FileCommand>("load", &file_attrs)
    .await?;

// Query - reference the loaded data by store path
let query_attrs = ObjectBuilder::new()
    .insert("tables", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "orders")
            .insert("source", "data.load.orders.data")  // Store path reference
            .build_scalar(),
    ]))
    .insert("query", "SELECT * FROM orders WHERE status = 'completed'")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("query"))
    .await?
    .add_command::<SqlCommand>("completed", &query_attrs)
    .await?;
```

## Error Handling

`FileCommand` will return an error if:

- The file does not exist
- The path points to a directory instead of a file
- The file format is not one of `csv`, `json`, or `parquet`
- The file content cannot be parsed as the specified format

## Format Notes

### CSV

- Assumes the first row contains headers
- Uses default CSV parsing options from Polars

### JSON

- Expects newline-delimited JSON (NDJSON) or JSON array format
- Uses Polars' `JsonReader`

### Parquet

- Reads standard Apache Parquet files
- Efficient for large datasets with columnar storage
