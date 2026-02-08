# Data Stores

All data in a Panopticon pipeline flows through two stores: the **ScalarStore** for JSON-like values and the **TabularStore** for Polars DataFrames. Understanding these stores is essential for working with command inputs and outputs.

## Two Stores, Two Data Models

```
┌─────────────────────────────────────────────────────────────────┐
│                      ExecutionContext                           │
│                                                                 │
│  ┌─────────────────────────────┐  ┌─────────────────────────┐   │
│  │        ScalarStore          │  │      TabularStore       │   │
│  │                             │  │                         │   │
│  │  • Strings                  │  │  • Polars DataFrames    │   │
│  │  • Numbers                  │  │  • Columnar data        │   │
│  │  • Booleans                 │  │  • SQL-queryable        │   │
│  │  • Arrays                   │  │                         │   │
│  │  • Objects (nested)         │  │                         │   │
│  │  • Null                     │  │                         │   │
│  │                             │  │                         │   │
│  │  Backed by Tera Context     │  │  HashMap<String, DF>    │   │
│  │  (enables templating)       │  │                         │   │
│  └─────────────────────────────┘  └─────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### ScalarStore

The ScalarStore holds JSON-like values. Internally, it wraps a Tera context, which means all scalar values are automatically available for template substitution.

```rust
pub type ScalarValue = tera::Value;  // Re-export of serde_json::Value
```

Scalar values can be:
- **Null** - Absence of a value
- **Bool** - `true` or `false`
- **Number** - Integers or floating-point
- **String** - Text data
- **Array** - Ordered list of values
- **Object** - Key-value map (nested structure)

### TabularStore

The TabularStore holds Polars DataFrames - efficient columnar data structures ideal for analytical queries.

```rust
pub type TabularValue = polars::prelude::DataFrame;
```

DataFrames are stored by their full store path (as a dotted string key). Commands like `SqlCommand` can register these as tables and query across them.

## Store Paths

Values in both stores are addressed using `StorePath` - a structured path that typically follows the pattern `namespace.command.field`:

```rust
// Create a store path
let path = StorePath::from_segments(["data", "load", "users", "data"]);

// Access with dotted notation
let dotted = path.to_dotted();  // "data.load.users.data"

// Add segments
let status_path = path.with_segment("status");  // "data.load.users.status"

// Add iteration index
let indexed = path.with_index(0);  // "data.load.users[0]"
```

Store paths provide a consistent way to reference data throughout the pipeline.

## ScalarValue Operations

### Creating Values

Panopticon provides helper functions and the `ObjectBuilder` for creating scalar values:

```rust
use panopticon_core::prelude::*;

// Primitives convert automatically via Into<ScalarValue>
let string_val: ScalarValue = "hello".into();
let number_val: ScalarValue = 42.into();
let bool_val: ScalarValue = true.into();

// Arrays
let array_val = ScalarValue::Array(vec![
    "a".into(),
    "b".into(),
    "c".into(),
]);

// Objects using ObjectBuilder
let object_val = ObjectBuilder::new()
    .insert("name", "Alice")
    .insert("age", 30)
    .insert("active", true)
    .build_scalar();
```

### Type Checking and Extraction

ScalarValue provides methods for type checking and extraction:

```rust
// Standard serde_json methods
if let Some(s) = value.as_str() { /* use string */ }
if let Some(n) = value.as_i64() { /* use integer */ }
if let Some(b) = value.as_bool() { /* use boolean */ }
if let Some(arr) = value.as_array() { /* use array */ }
if let Some(obj) = value.as_object() { /* use object */ }

// Extension trait with error handling
use panopticon_core::prelude::ScalarAsExt;

let name = value.as_str_or_err("name")?;      // Returns Result
let count = value.as_i64_or_err("count")?;
let items = value.as_array_or_err("items")?;
```

### Map Extension Trait

For working with object maps, the `ScalarMapExt` trait provides convenient accessors:

```rust
use panopticon_core::prelude::ScalarMapExt;

let obj = value.as_object().unwrap();

// Required fields (returns Result)
let name = obj.get_required_string("name")?;
let count = obj.get_required_i64("count")?;
let enabled = obj.get_required_bool("enabled")?;

// Optional fields (returns Option)
let description = obj.get_optional_string("description");
let limit = obj.get_optional_i64("limit");
```

## How Commands Use Stores

Commands read inputs from and write outputs to the stores. Here is a typical pattern:

```
┌──────────────┐         ┌─────────────┐         ┌──────────────┐
│ ScalarStore  │────────▶│   Command   │────────▶│ ScalarStore  │
│ (inputs)     │         │             │         │ (outputs)    │
└──────────────┘         │  - Read     │         └──────────────┘
                         │  - Process  │
┌──────────────┐         │  - Write    │         ┌──────────────┐
│ TabularStore │────────▶│             │────────▶│ TabularStore │
│ (inputs)     │         └─────────────┘         │ (outputs)    │
└──────────────┘                                 └──────────────┘
```

### FileCommand Example

When `FileCommand` loads a CSV file:

1. Reads the file from disk (not from stores)
2. Writes DataFrame to TabularStore at `namespace.command.name.data`
3. Writes metadata to ScalarStore:
   - `namespace.command.name.rows` - Row count
   - `namespace.command.name.columns` - Column count
   - `namespace.command.status` - "success"
   - `namespace.command.duration_ms` - Execution time

### SqlCommand Example

When `SqlCommand` runs a query:

1. Reads DataFrames from TabularStore based on `tables` attribute
2. Registers them as SQL tables
3. Executes the query
4. Writes result DataFrame to TabularStore
5. Writes metadata to ScalarStore

## Template Substitution

Because ScalarStore wraps a Tera context, all values are available for template substitution in command attributes:

```rust
// Static namespace provides config
pipeline.add_namespace(
    NamespaceBuilder::new("config")
        .static_ns()
        .insert("region", "us-east")
        .insert("limit", ScalarValue::Number(100.into()))
).await?;

// SQL command uses templated query
let sql_attrs = ObjectBuilder::new()
    .insert("query", "SELECT * FROM users WHERE region = '{{ config.region }}' LIMIT {{ config.limit }}")
    .build_hashmap();
```

During execution, Panopticon substitutes `{{ config.region }}` with `"us-east"` and `{{ config.limit }}` with `100` before the command runs.

## Accessing Results

After pipeline execution, we retrieve results through the `ResultStore`:

```rust
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(ResultSettings::default()).await?;

// Get results for a specific command
let source = StorePath::from_segments(["data", "load"]);
let cmd_results = results.get_by_source(&source).expect("Expected results");

// Access metadata
let rows = cmd_results.meta_get(&source.with_segment("rows"));

// Access data
let status = cmd_results.data_get(&source.with_segment("result"));
```

### ResultValue Types

Results come in two forms:

```rust
pub enum ResultValue {
    Scalar {
        ty: ScalarType,
        value: ScalarValue,
    },
    Tabular {
        path: PathBuf,      // File path where DataFrame was written
        format: OutputFormat,
        rows_count: usize,
        columns_count: usize,
    },
}
```

Tabular results are written to disk (CSV, JSON, or Parquet) and the `ResultValue` contains the path and summary statistics.

## ScalarType Enum

For type introspection, Panopticon provides a `ScalarType` enum:

```rust
pub enum ScalarType {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}
```

This is useful for schema validation and result type checking.

## Practical Example

Here is a complete example showing data flow through the stores:

```rust
use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // 1. Static namespace adds values to ScalarStore
    pipeline
        .add_namespace(
            NamespaceBuilder::new("config")
                .static_ns()
                .insert("threshold", ScalarValue::Number(50.into()))
        )
        .await?;

    // 2. FileCommand writes DataFrame to TabularStore
    let file_attrs = ObjectBuilder::new()
        .insert("files", ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "scores")
                .insert("file", "data/scores.csv")
                .insert("format", "csv")
                .build_scalar(),
        ]))
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("data"))
        .await?
        .add_command::<FileCommand>("load", &file_attrs)
        .await?;

    // 3. SqlCommand reads from TabularStore, uses ScalarStore for templating
    let sql_attrs = ObjectBuilder::new()
        .insert("tables", ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "scores")
                .insert("source", "data.load.scores.data")
                .build_scalar(),
        ]))
        .insert("query", "SELECT * FROM scores WHERE value > {{ config.threshold }}")
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("filtered"))
        .await?
        .add_command::<SqlCommand>("high_scores", &sql_attrs)
        .await?;

    // Execute and collect results
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // 4. Results contain both scalar metadata and tabular file paths
    let source = StorePath::from_segments(["filtered", "high_scores"]);
    if let Some(cmd_results) = results.get_by_source(&source) {
        // Metadata from ScalarStore
        if let Some(rows) = cmd_results.meta_get(&source.with_segment("rows")) {
            println!("Filtered rows: {}", rows);
        }

        // Tabular data was written to file
        if let Some(result_value) = cmd_results.data_get(&source.with_segment("data")) {
            if let Some((path, _format)) = result_value.as_tabular() {
                println!("Data written to: {}", path.display());
            }
        }
    }

    Ok(())
}
```

## Summary

The dual-store architecture in Panopticon separates concerns:

- **ScalarStore** handles configuration, metadata, and template substitution
- **TabularStore** handles large datasets efficiently with Polars

This separation allows us to:
- Use JSON-like values for flexible configuration
- Leverage Tera templating for dynamic attribute values
- Process large datasets with columnar efficiency
- Query across DataFrames using SQL

Understanding how data flows through these stores is key to building effective Panopticon pipelines.
