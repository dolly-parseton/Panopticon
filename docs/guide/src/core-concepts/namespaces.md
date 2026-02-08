# Namespaces

Namespaces are the organizational unit in Panopticon. Every command belongs to exactly one namespace, and the namespace type determines how those commands execute.

## The Three Namespace Types

```
┌─────────────────────────────────────────────────────────────────┐
│                        Namespace Types                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Once        Execute commands once, in order                   │
│   ─────────────────────────────────────────────                 │
│   [cmd1] → [cmd2] → [cmd3]                                      │
│                                                                 │
│   Iterative   Execute commands once per item in a collection    │
│   ─────────────────────────────────────────────                 │
│   for item in collection:                                       │
│       [cmd1] → [cmd2] → [cmd3]                                  │
│                                                                 │
│   Static      No commands, just provides constant values        │
│   ─────────────────────────────────────────────                 │
│   { key1: value1, key2: value2 }                                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Once Namespaces

The `Once` namespace is the default. Commands in a Once namespace execute exactly once, in the order they were added.

```rust
// Create a Once namespace (the default)
pipeline
    .add_namespace(NamespaceBuilder::new("data"))
    .await?
    .add_command::<FileCommand>("load", &file_attrs)
    .await?;
```

This is the most common namespace type. Use it for:
- Loading data from files or APIs
- Running SQL queries
- Performing one-time transformations

### Iterative Namespaces

`Iterative` namespaces execute their commands once for each item in a collection. The collection can come from:
- An array in the scalar store
- Object keys from a JSON object
- A column in a DataFrame
- A string split by a delimiter

```rust
// Create an Iterative namespace that loops over object keys
let mut handle = pipeline
    .add_namespace(
        NamespaceBuilder::new("classify")
            .iterative()
            .store_path(StorePath::from_segments(["config", "regions"]))
            .scalar_object_keys(None, false)
            .iter_var("region")
            .index_var("idx"),
    )
    .await?;

handle
    .add_command::<ConditionCommand>("check", &condition_attrs)
    .await?;
```

During execution, Panopticon:
1. Resolves the collection from the store path
2. For each item, sets the iteration variables (`region` and `idx` in this example)
3. Executes all commands in the namespace
4. Cleans up the iteration variables

Results from iterative commands are indexed. Instead of storing at `classify.check.result`, we store at `classify.check[0].result`, `classify.check[1].result`, etc.

### Static Namespaces

`Static` namespaces contain no commands - they exist purely to provide constant values to the data stores. Think of them as configuration namespaces.

```rust
// Create a Static namespace with configuration values
pipeline
    .add_namespace(
        NamespaceBuilder::new("config")
            .static_ns()
            .insert("api_version", ScalarValue::String("v2".into()))
            .insert(
                "regions",
                ObjectBuilder::new()
                    .insert("us-east", "Virginia")
                    .insert("us-west", "Oregon")
                    .insert("eu-west", "Ireland")
                    .build_scalar(),
            ),
    )
    .await?;
```

Values from static namespaces are available to all subsequent commands via Tera templating:

```rust
// In a later command's attributes
.insert("endpoint", "https://api.example.com/{{ config.api_version }}/data")
```

## Iteration Sources

Iterative namespaces support several source types for determining what to iterate over:

### ScalarArray

Iterate over elements in a JSON array:

```rust
NamespaceBuilder::new("process")
    .iterative()
    .store_path(StorePath::from_segments(["data", "items"]))
    .scalar_array(None)  // None = all items, Some((start, end)) = range
    .iter_var("item")
```

### ScalarObjectKeys

Iterate over keys of a JSON object:

```rust
NamespaceBuilder::new("classify")
    .iterative()
    .store_path(StorePath::from_segments(["config", "regions"]))
    .scalar_object_keys(None, false)  // None = all keys, Some(vec) = filter
    .iter_var("region")
```

The second parameter controls exclusion - `true` means "iterate over all keys *except* those listed".

### ScalarStringSplit

Iterate over parts of a delimited string:

```rust
NamespaceBuilder::new("tags")
    .iterative()
    .store_path(StorePath::from_segments(["data", "tag_list"]))
    .string_split(",")
    .iter_var("tag")
```

### TabularColumn

Iterate over unique values in a DataFrame column:

```rust
NamespaceBuilder::new("by_category")
    .iterative()
    .store_path(StorePath::from_segments(["data", "products", "data"]))
    .tabular_column("category", None)
    .iter_var("category")
```

## Complete Example: Object Key Iteration

Here is a full example showing how to iterate over object keys:

```rust
use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // --- Static namespace: an object whose keys we will iterate ---
    pipeline
        .add_namespace(
            NamespaceBuilder::new("config").static_ns().insert(
                "regions",
                ObjectBuilder::new()
                    .insert("us-east", "Virginia")
                    .insert("us-west", "Oregon")
                    .insert("eu-west", "Ireland")
                    .build_scalar(),
            ),
        )
        .await?;

    // --- Iterative namespace: loop over each region key ---
    let condition_attrs = ObjectBuilder::new()
        .insert(
            "branches",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "is_us")
                    .insert("if", "region is starting_with(\"us-\")")
                    .insert("then", "Region {{ region }} is in the US")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "is_eu")
                    .insert("if", "region is starting_with(\"eu-\")")
                    .insert("then", "Region {{ region }} is in the EU")
                    .build_scalar(),
            ]),
        )
        .insert("default", "Region {{ region }} is in an unknown area")
        .build_hashmap();

    let mut handle = pipeline
        .add_namespace(
            NamespaceBuilder::new("classify")
                .iterative()
                .store_path(StorePath::from_segments(["config", "regions"]))
                .scalar_object_keys(None, false)
                .iter_var("region")
                .index_var("idx"),
        )
        .await?;
    handle
        .add_command::<ConditionCommand>("region", &condition_attrs)
        .await?;

    // --- Execute ---
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // --- Print results per iteration ---
    println!("=== Iterating over region keys ===\n");

    let mut idx = 0;
    loop {
        let source = StorePath::from_segments(["classify", "region"]).with_index(idx);
        let Some(cmd_results) = results.get_by_source(&source) else {
            break;
        };

        let result = cmd_results
            .data_get(&source.with_segment("result"))
            .and_then(|r| r.as_scalar())
            .expect("Expected result");
        println!("  [{}] {}", idx, result.1);

        idx += 1;
    }

    println!("\nProcessed {} region(s)", idx);

    Ok(())
}
```

Output:
```
=== Iterating over region keys ===

  [0] Region us-east is in the US
  [1] Region us-west is in the US
  [2] Region eu-west is in the EU

Processed 3 region(s)
```

## Reserved Names

Two namespace names are reserved and cannot be used:
- `item` - Default iteration variable name
- `index` - Default index variable name

If you try to create a namespace with a reserved name, the builder will return an error.

## Namespace Execution Order

Namespaces execute in the order they were added to the pipeline. This is important because later namespaces can reference data produced by earlier ones.

```rust
// 1. Load data (executes first)
pipeline.add_namespace(NamespaceBuilder::new("data")).await?;

// 2. Query the loaded data (executes second)
pipeline.add_namespace(NamespaceBuilder::new("query")).await?;

// 3. Aggregate the query results (executes third)
pipeline.add_namespace(NamespaceBuilder::new("stats")).await?;
```

Within a namespace, commands also execute in order. The combination of namespace ordering and command ordering gives us predictable, deterministic pipeline execution.
