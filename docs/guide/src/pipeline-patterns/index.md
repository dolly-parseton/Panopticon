# Pipeline Patterns

This section covers common patterns and recipes for building effective Panopticon pipelines. Each pattern addresses a specific problem with a concrete solution you can adapt to your use case.

## Pattern Overview

| Pattern | Problem | Solution |
|---------|---------|----------|
| [Iteration](./iteration.md) | Process each item in a collection | Use iterative namespaces with `iter_var` and `index_var` |
| [Conditional Execution](./conditional-execution.md) | Skip commands based on runtime conditions | Use the `when` attribute with Tera expressions |
| [Pipeline Editing](./pipeline-editing.md) | Add stages to an already-executed pipeline | Use `.edit()` to return to Draft state |
| [Result Access](./result-access.md) | Retrieve and export pipeline outputs | Configure `ResultSettings` and iterate `ResultStore` |

## When to Use These Patterns

### Iteration

Use iterative namespaces when you need to:

- Process each key in a configuration object
- Apply the same operation to every item in an array
- Loop over unique values in a DataFrame column
- Split a string and handle each segment

**Example scenario**: You have a configuration object with region keys (`us-east`, `us-west`, `eu-west`) and need to run a classification command for each region.

### Conditional Execution

Use the `when` attribute when you need to:

- Feature-flag parts of your pipeline
- Skip expensive operations when their inputs are empty
- Create debug-only or production-only commands
- Short-circuit processing based on earlier results

**Example scenario**: You have a feature flag in your configuration, and certain commands should only execute when that flag is enabled.

### Pipeline Editing

Use `.edit()` when you need to:

- Add follow-up analysis after initial exploration
- Build pipelines incrementally based on intermediate results
- Implement REPL-style workflows
- Reuse expensive data loading across multiple analyses

**Example scenario**: You loaded a large dataset and ran an initial query. Now you want to add aggregation commands without re-loading the data.

### Result Access

Configure `ResultSettings` when you need to:

- Export tabular results to a specific directory
- Choose output format (CSV, JSON, Parquet)
- Exclude certain commands from result collection
- Iterate over all results programmatically

**Example scenario**: After pipeline execution, you want to write all DataFrames to a temporary directory as Parquet files and print a summary of all computed metrics.

## Combining Patterns

These patterns compose naturally. A real-world pipeline might:

1. Load data into a static namespace for configuration
2. Use an iterative namespace to process each configuration key
3. Apply `when` conditions to skip processing for disabled regions
4. Call `.edit()` to add aggregation after reviewing initial results
5. Export all results with custom `ResultSettings`

```rust
// Pseudocode showing pattern composition
let mut pipeline = Pipeline::new();

// Static config (Pattern: static namespaces)
pipeline.add_namespace(NamespaceBuilder::new("config").static_ns()
    .insert("regions", regions_object)
    .insert("debug_mode", ScalarValue::Bool(false))
).await?;

// Iterative processing (Pattern: iteration)
let mut handle = pipeline.add_namespace(
    NamespaceBuilder::new("process")
        .iterative()
        .store_path(StorePath::from_segments(["config", "regions"]))
        .scalar_object_keys(None, false)
        .iter_var("region")
).await?;

// Conditional command (Pattern: when)
handle.add_command::<ExpensiveCommand>("analyze", &ObjectBuilder::new()
    .insert("when", "config.debug_mode == false")  // Skip in debug mode
    .build_hashmap()
).await?;

// Execute first pass
let completed = pipeline.compile().await?.execute().await?;

// Add more analysis (Pattern: pipeline editing)
let mut pipeline = completed.edit();
pipeline.add_namespace(NamespaceBuilder::new("summary")).await?
    .add_command::<AggregateCommand>("totals", &agg_attrs).await?;

// Re-execute and collect results (Pattern: result access)
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(
    ResultSettings::new()
        .with_output_path(output_dir)
        .with_format(TabularFormat::Parquet)
).await?;
```

The following pages dive into each pattern with complete, runnable examples.
