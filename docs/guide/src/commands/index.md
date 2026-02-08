# Built-in Commands

Panopticon includes a set of built-in commands that cover common data pipeline operations. These commands work together to load, transform, analyze, and export data.

## Command Overview

| Command | Purpose | Key Use Cases |
|---------|---------|---------------|
| [FileCommand](./file-command.md) | Load data files | Read CSV, JSON, and Parquet files into the tabular store |
| [SqlCommand](./sql-command.md) | Query tabular data | Filter, join, transform data using SQL syntax |
| [AggregateCommand](./aggregate-command.md) | Compute statistics | Calculate count, sum, mean, max, min, median, and more |
| [ConditionCommand](./condition-command.md) | Branch logic | Evaluate Tera expressions to produce conditional outputs |
| [TemplateCommand](./template-command.md) | Render templates | Generate files using Tera templates with inheritance |

## Common Patterns

### Data Loading and Analysis

A typical pipeline starts by loading data with `FileCommand`, optionally transforms it with `SqlCommand`, and computes metrics with `AggregateCommand`:

```rust
// Load CSV data
pipeline
    .add_namespace(NamespaceBuilder::new("data"))
    .await?
    .add_command::<FileCommand>("load", &file_attrs)
    .await?;

// Query the loaded data
pipeline
    .add_namespace(NamespaceBuilder::new("query"))
    .await?
    .add_command::<SqlCommand>("filtered", &query_attrs)
    .await?;

// Aggregate results
pipeline
    .add_namespace(NamespaceBuilder::new("stats"))
    .await?
    .add_command::<AggregateCommand>("summary", &agg_attrs)
    .await?;
```

### Conditional Execution

All commands support the optional `when` attribute, which is a Tera expression evaluated at runtime. If it resolves to a falsy value, the command is skipped:

```rust
let attrs = ObjectBuilder::new()
    .insert("when", "inputs.feature_enabled")  // Skip if false
    .insert("source", "data.load.users.data")
    // ... other attributes
    .build_hashmap();
```

When a command is skipped:
- Its `status` meta result is set to `"skipped"`
- Data results are absent from the ResultStore
- Dependent commands that reference its outputs will fail unless handled

### Store Path References

Commands that consume data from earlier pipeline stages use **store paths** to reference results. Store paths are dot-separated strings that identify locations in the data stores:

```
namespace.command.result_key
```

For example:
- `data.load.users.data` - The tabular data loaded from users file
- `stats.summary.row_count` - A scalar aggregation result

See [Store Paths](../working-with-data/store-paths.md) for more details.

### Tera Template Substitution

Many command attributes support Tera template syntax for dynamic values. This is indicated by "supports Tera substitution" in the attribute documentation:

```rust
.insert("file", "{{ config.data_dir }}/users.csv")
.insert("query", "SELECT * FROM users WHERE status = '{{ inputs.status }}'")
```

The execution context automatically substitutes these expressions using values from the scalar store.

## Result Types

Commands produce two types of results:

### Meta Results

Metadata about the command execution (row counts, sizes, column lists). These are accessed via `meta_get()` on the command results:

```rust
let row_count = cmd_results
    .meta_get(&source.with_segment("rows"))
    .expect("Expected rows");
```

### Data Results

The primary outputs of the command (DataFrames, computed values). These are accessed via `data_get()`:

```rust
let df = cmd_results
    .data_get(&source.with_segment("data"))
    .and_then(|r| r.as_tabular());
```

Each command's documentation lists which results are meta vs. data.
