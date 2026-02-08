# Pipeline Editing

**Problem**: You have executed a pipeline and want to add more commands without re-running everything from scratch.

**Solution**: Call `.edit()` on a completed pipeline to return to Draft state, add new namespaces and commands, then re-compile and execute.

## How Pipeline Editing Works

The Pipeline type has a state machine with three states:

```
Draft  --compile-->  Ready  --execute-->  Completed
  ^                                           |
  |                                           |
  +------------------edit()-------------------+
```

When you call `.edit()` on a `Pipeline<Completed>`:

1. The pipeline returns to `Pipeline<Draft>` state
2. All existing namespaces and commands are preserved
3. All data in the stores is preserved
4. You can add new namespaces and commands
5. Re-compilation and execution runs only the new additions (existing results remain)

## Basic Pattern: Two-Pass Pipeline

A common workflow: load data, run initial analysis, review, then add more analysis.

```rust
use panopticon_core::prelude::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ===== Pass 1: Load data and run initial query =====
    println!("=== Pass 1: Load + Query ===\n");

    let mut pipeline = Pipeline::new();

    // Load users from CSV
    let file_attrs = ObjectBuilder::new()
        .insert(
            "files",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "users")
                    .insert("file", fixtures_dir().join("users.csv").to_string_lossy().to_string())
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

    // Run a SQL query
    let sql_attrs = ObjectBuilder::new()
        .insert(
            "tables",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "users")
                    .insert("source", "data.load.users.data")
                    .build_scalar(),
            ]),
        )
        .insert("query", "SELECT name, age FROM users ORDER BY age DESC")
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("query"))
        .await?
        .add_command::<SqlCommand>("sorted", &sql_attrs)
        .await?;

    // Execute pass 1
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // Inspect pass 1 results
    let query_source = StorePath::from_segments(["query", "sorted"]);
    let query_results = results.get_by_source(&query_source).expect("Expected query results");
    let rows = query_results.meta_get(&query_source.with_segment("rows")).expect("Expected rows");
    println!("  query.sorted: {} rows", rows);
    println!("  Namespaces: data, query");

    // ===== Pass 2: Add aggregation to existing pipeline =====
    println!("\n=== Pass 2: Edit + Aggregate ===\n");

    // Return to Draft state - this is the key step!
    let mut pipeline = completed.edit();

    // Add aggregation namespace (data from pass 1 is still available)
    let agg_attrs = ObjectBuilder::new()
        .insert("source", "data.load.users.data")
        .insert(
            "aggregations",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "user_count")
                    .insert("op", "count")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "avg_age")
                    .insert("column", "age")
                    .insert("op", "mean")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "oldest")
                    .insert("column", "age")
                    .insert("op", "max")
                    .build_scalar(),
            ]),
        )
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("stats"))
        .await?
        .add_command::<AggregateCommand>("users", &agg_attrs)
        .await?;

    // Re-compile and execute
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // Original results are still present
    let query_source = StorePath::from_segments(["query", "sorted"]);
    let query_results = results.get_by_source(&query_source).expect("query.sorted should still exist");
    let rows = query_results.meta_get(&query_source.with_segment("rows")).expect("Expected rows");
    println!("  query.sorted: {} rows (preserved from pass 1)", rows);

    // New aggregation results are available
    let stats_source = StorePath::from_segments(["stats", "users"]);
    let stats_results = results.get_by_source(&stats_source).expect("Expected stats results");

    for name in ["user_count", "avg_age", "oldest"] {
        let value = stats_results
            .data_get(&stats_source.with_segment(name))
            .and_then(|r| r.as_scalar())
            .expect(&format!("Expected {}", name));
        println!("  stats.users.{} = {}", name, value.1);
    }

    println!("  Namespaces: data, query, stats");
    println!("\nPipeline successfully edited and re-executed.");

    Ok(())
}
```

**Output**:
```
=== Pass 1: Load + Query ===

  query.sorted: 5 rows
  Namespaces: data, query

=== Pass 2: Edit + Aggregate ===

  query.sorted: 5 rows (preserved from pass 1)
  stats.users.user_count = 5
  stats.users.avg_age = 32.4
  stats.users.oldest = 45
  Namespaces: data, query, stats

Pipeline successfully edited and re-executed.
```

## Key Points

### State Preservation

When you call `.edit()`:

- **Preserved**: Namespaces, commands, store data, execution results
- **Reset**: Pipeline state returns to Draft

This means pass 2 can reference data created in pass 1 without re-executing the original commands.

### Ownership Transfer

The `.edit()` method consumes the `Pipeline<Completed>`:

```rust
// completed is consumed here
let mut pipeline = completed.edit();

// This would fail - completed no longer exists:
// let _ = completed.results(...);  // ERROR: use of moved value
```

### Re-compilation Required

After editing, you must call `.compile()` before `.execute()`:

```rust
let mut pipeline = completed.edit();
pipeline.add_namespace(...).await?;

// Must compile before executing
let completed = pipeline.compile().await?.execute().await?;
```

## Use Cases

### Exploratory Data Analysis

Build your analysis incrementally:

```rust
// Step 1: Load and explore
let completed = load_and_preview().await?;
// ... review results ...

// Step 2: Add filtering based on what you saw
let mut pipeline = completed.edit();
add_filters(&mut pipeline).await?;
let completed = pipeline.compile().await?.execute().await?;
// ... review filtered results ...

// Step 3: Add aggregation
let mut pipeline = completed.edit();
add_aggregations(&mut pipeline).await?;
let completed = pipeline.compile().await?.execute().await?;
```

### Conditional Follow-Up

Add analysis based on intermediate results:

```rust
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(ResultSettings::default()).await?;

// Check if we need additional analysis
let row_count = get_row_count(&results);
if row_count > 1000 {
    let mut pipeline = completed.edit();
    add_sampling_namespace(&mut pipeline).await?;
    let completed = pipeline.compile().await?.execute().await?;
}
```

### REPL-Style Workflows

In an interactive environment, let users add commands incrementally:

```rust
let mut completed = initial_pipeline.compile().await?.execute().await?;

loop {
    // Show current results
    display_results(&completed).await?;

    // Get user input
    let command = get_user_command()?;
    if command == "quit" {
        break;
    }

    // Add the new command
    let mut pipeline = completed.edit();
    add_user_command(&mut pipeline, &command).await?;
    completed = pipeline.compile().await?.execute().await?;
}
```

### Expensive Data Reuse

Load expensive data once, analyze multiple ways:

```rust
// Load large dataset (expensive)
let completed = load_big_dataset().await?;

// Analysis 1
let mut p1 = completed.edit();
add_analysis_1(&mut p1).await?;
let c1 = p1.compile().await?.execute().await?;
save_results(&c1, "analysis_1").await?;

// Analysis 2 (starts fresh from after load, not from c1)
let mut p2 = completed.edit();
add_analysis_2(&mut p2).await?;
let c2 = p2.compile().await?.execute().await?;
save_results(&c2, "analysis_2").await?;
```

Note: In this pattern, each `.edit()` call forks from the same point. Changes in `p1` do not affect `p2`.

## Best Practices

### Name Namespaces for Phases

Use namespace names that indicate which pass they belong to:

```rust
// Pass 1
.add_namespace(NamespaceBuilder::new("load"))
.add_namespace(NamespaceBuilder::new("initial_query"))

// Pass 2
.add_namespace(NamespaceBuilder::new("refined_query"))
.add_namespace(NamespaceBuilder::new("aggregations"))
```

### Document the Multi-Pass Structure

When your pipeline has multiple passes, document the intended flow:

```rust
// Pipeline structure:
// Pass 1: Load data, run initial validation
// Pass 2: Apply transformations based on validation results
// Pass 3: Generate final reports
```

### Avoid Namespace Name Conflicts

Each namespace must have a unique name. Adding a namespace with a duplicate name will fail:

```rust
// Pass 1
pipeline.add_namespace(NamespaceBuilder::new("data")).await?;

// Pass 2 - this fails!
let mut pipeline = completed.edit();
pipeline.add_namespace(NamespaceBuilder::new("data")).await?;  // ERROR: duplicate name
```

### Consider Memory Usage

All store data is preserved across edits. For very large datasets, this can increase memory usage. If you need to release memory:

```rust
// Create a fresh pipeline instead of editing
let mut new_pipeline = Pipeline::new();
// Copy only the data you need
```

## Ready State Editing

You can also call `.edit()` on a `Pipeline<Ready>` (after compile, before execute):

```rust
let ready = pipeline.compile().await?;
// Oops, forgot something
let mut pipeline = ready.edit();
pipeline.add_namespace(...).await?;
let ready = pipeline.compile().await?;
let completed = ready.execute().await?;
```

This is useful when you realize you need to add something after compilation but before execution.
