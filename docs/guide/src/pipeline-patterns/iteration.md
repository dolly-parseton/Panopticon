# Iteration

**Problem**: You have a collection of items (object keys, array elements, or DataFrame column values) and need to run the same commands for each item.

**Solution**: Create an iterative namespace that loops over the collection, exposing each item via `iter_var` and its position via `index_var`.

## How Iterative Namespaces Work

When you create an iterative namespace, Panopticon:

1. Resolves the source collection from the data store
2. Executes all commands in the namespace once per item
3. Stores results with an index suffix (e.g., `classify.region[0]`, `classify.region[1]`)
4. Exposes the current item and index as template variables

## Iterator Types

Panopticon supports four iterator types:

| Type | Source | Iterates Over |
|------|--------|---------------|
| `scalar_object_keys` | JSON object | Keys of the object |
| `scalar_array` | JSON array | Elements of the array |
| `string_split` | String value | Segments split by delimiter |
| `tabular_column` | DataFrame column | Unique values in the column |

## Basic Pattern: Object Keys

The most common pattern is iterating over the keys of a configuration object.

```rust
use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // Create a static namespace with an object to iterate over
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

    // Create an iterative namespace that loops over region keys
    let mut handle = pipeline
        .add_namespace(
            NamespaceBuilder::new("classify")
                .iterative()
                .store_path(StorePath::from_segments(["config", "regions"]))
                .scalar_object_keys(None, false)  // All keys, no exclusions
                .iter_var("region")               // Current key available as {{ region }}
                .index_var("idx"),                // Current index available as {{ idx }}
        )
        .await?;

    // This command runs once per region key
    let attrs = ObjectBuilder::new()
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

    handle
        .add_command::<ConditionCommand>("check", &attrs)
        .await?;

    // Execute the pipeline
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // Access results by index
    for idx in 0.. {
        let source = StorePath::from_segments(["classify", "check"]).with_index(idx);
        let Some(cmd_results) = results.get_by_source(&source) else {
            break;  // No more iterations
        };

        let result = cmd_results
            .data_get(&source.with_segment("result"))
            .and_then(|r| r.as_scalar())
            .expect("Expected result");

        println!("[{}] {}", idx, result.1);
    }

    Ok(())
}
```

**Output**:
```
[0] Region us-east is in the US
[1] Region us-west is in the US
[2] Region eu-west is in the EU
```

## Key Concepts

### iter_var and index_var

These methods define the template variable names used during iteration:

```rust
.iter_var("region")   // {{ region }} contains the current item
.index_var("idx")     // {{ idx }} contains 0, 1, 2, ...
```

If not specified, the defaults are:
- `iter_var`: `"item"`
- `index_var`: `"index"`

These variables are available in:
- Tera template expressions in command attributes
- The `when` condition for conditional execution
- Any attribute that supports Tera substitution

### store_path

The `store_path` points to the collection in the scalar or tabular store:

```rust
.store_path(StorePath::from_segments(["config", "regions"]))
```

This path must exist when the pipeline executes. If it does not, execution fails with an error.

### Result Indexing

Iterative namespace results are indexed by iteration number. To access them:

```rust
// Build the base path
let base = StorePath::from_segments(["namespace", "command"]);

// Access specific iteration
let iteration_0 = base.with_index(0);  // namespace.command[0]
let iteration_1 = base.with_index(1);  // namespace.command[1]

// Get results
let results_0 = results.get_by_source(&iteration_0);
```

## Filtering Object Keys

You can filter which keys to iterate over:

```rust
// Only iterate over specific keys
.scalar_object_keys(Some(vec!["us-east".to_string(), "eu-west".to_string()]), false)

// Exclude specific keys (iterate over all except these)
.scalar_object_keys(Some(vec!["us-west".to_string()]), true)

// Iterate over all keys
.scalar_object_keys(None, false)
```

## Iterating Over Arrays

To iterate over array elements instead of object keys:

```rust
pipeline
    .add_namespace(
        NamespaceBuilder::new("config").static_ns().insert(
            "items",
            ScalarValue::Array(vec![
                ScalarValue::String("apple".to_string()),
                ScalarValue::String("banana".to_string()),
                ScalarValue::String("cherry".to_string()),
            ]),
        ),
    )
    .await?;

let mut handle = pipeline
    .add_namespace(
        NamespaceBuilder::new("process")
            .iterative()
            .store_path(StorePath::from_segments(["config", "items"]))
            .scalar_array(None)        // All elements
            .iter_var("fruit"),
    )
    .await?;
```

With a range to limit iterations:

```rust
.scalar_array(Some((0, 2)))  // Only first two elements (indices 0 and 1)
```

## Iterating Over String Segments

Split a string and iterate over the parts:

```rust
pipeline
    .add_namespace(
        NamespaceBuilder::new("config").static_ns()
            .insert("path", ScalarValue::String("/usr/local/bin".to_string())),
    )
    .await?;

let mut handle = pipeline
    .add_namespace(
        NamespaceBuilder::new("segments")
            .iterative()
            .store_path(StorePath::from_segments(["config", "path"]))
            .string_split("/")         // Split on "/"
            .iter_var("segment"),
    )
    .await?;
```

## Iterating Over DataFrame Columns

Extract unique values from a DataFrame column:

```rust
// Assuming data.load.users.data contains a DataFrame with a "department" column
let mut handle = pipeline
    .add_namespace(
        NamespaceBuilder::new("by_dept")
            .iterative()
            .store_path(StorePath::from_segments(["data", "load", "users", "data"]))
            .tabular_column("department", None)  // Unique values from "department"
            .iter_var("dept"),
    )
    .await?;
```

This iterates over the unique, non-null values in the specified column. Use a range to limit:

```rust
.tabular_column("department", Some((0, 5)))  // First 5 unique values
```

## Best Practices

### Use Descriptive Variable Names

Choose `iter_var` names that reflect what you are iterating over:

```rust
// Good: clear what we're iterating
.iter_var("region")
.iter_var("user_id")
.iter_var("filename")

// Avoid: generic names
.iter_var("item")
.iter_var("x")
```

### Keep Iteration Counts Reasonable

Each iteration creates separate command executions and results. For large collections, consider:

- Filtering with `scalar_object_keys(Some(keys), false)`
- Using ranges with `scalar_array(Some((start, end)))`
- Pre-filtering data with `SqlCommand` before iteration

### Access Results Systematically

When iterating over results, use a loop that terminates when `get_by_source` returns `None`:

```rust
let mut idx = 0;
loop {
    let source = StorePath::from_segments(["ns", "cmd"]).with_index(idx);
    let Some(results) = store.get_by_source(&source) else {
        break;
    };
    // Process results...
    idx += 1;
}
```

This pattern handles any number of iterations without hardcoding the count.
