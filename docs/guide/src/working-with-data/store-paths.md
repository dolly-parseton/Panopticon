# Store Paths

`StorePath` is the fundamental addressing mechanism in Panopticon. Every piece of data - whether scalar values or tabular DataFrames - is stored and retrieved using a `StorePath`.

## What is a StorePath?

A `StorePath` is a sequence of string segments that form a hierarchical path, similar to filesystem paths but using dots as separators:

```
namespace.command.field.subfield
    ^        ^      ^      ^
    |        |      |      +-- Nested field
    |        |      +--------- Result field
    |        +---------------- Command name
    +------------------------- Namespace name
```

The path `data.load.products.row_count` represents:
- Namespace: `data`
- Command: `load`
- Field: `products`
- Subfield: `row_count`

## Creating StorePaths

### from_segments()

Build a `StorePath` from an iterator of segments:

```rust
use panopticon_core::prelude::*;

// From a slice of string literals
let path = StorePath::from_segments(["namespace", "command", "field"]);
assert_eq!(path.to_dotted(), "namespace.command.field");

// From a Vec
let segments = vec!["data", "load", "products"];
let path = StorePath::from_segments(segments);
assert_eq!(path.to_dotted(), "data.load.products");

// From any iterator of Into<String>
let path = StorePath::from_segments(["a", "b", "c"].into_iter());
```

### from_dotted()

Parse a dotted string into a `StorePath`:

```rust
let path = StorePath::from_dotted("config.regions.us-east");
assert_eq!(path.segments(), &["config", "regions", "us-east"]);
```

## Extending StorePaths

StorePaths are immutable by default. Extension methods return new paths:

### with_segment()

Add a named segment to create a child path:

```rust
let base = StorePath::from_segments(["namespace", "command"]);

// Add a field
let field = base.with_segment("output");
assert_eq!(field.to_dotted(), "namespace.command.output");

// Chain multiple segments
let nested = base
    .with_segment("results")
    .with_segment("summary");
assert_eq!(nested.to_dotted(), "namespace.command.results.summary");
```

### with_index()

Add a numeric index segment (useful for iteration):

```rust
let base = StorePath::from_segments(["classify", "region"]);

// First iteration
let iter0 = base.with_index(0);
assert_eq!(iter0.to_dotted(), "classify.region.0");

// Access a field within an iteration
let result = iter0.with_segment("result");
assert_eq!(result.to_dotted(), "classify.region.0.result");
```

### add_segment() (Mutable)

Mutate a path in place:

```rust
let mut path = StorePath::from_segments(["namespace"]);
path.add_segment("command");
path.add_segment("field");
assert_eq!(path.to_dotted(), "namespace.command.field");
```

## Inspecting StorePaths

### segments()

Get the path segments as a slice:

```rust
let path = StorePath::from_segments(["a", "b", "c"]);
assert_eq!(path.segments(), &["a", "b", "c"]);
```

### namespace()

Get the first segment (typically the namespace name):

```rust
let path = StorePath::from_segments(["data", "load", "file"]);
assert_eq!(path.namespace(), Some(&"data".to_string()));

let empty = StorePath::default();
assert_eq!(empty.namespace(), None);
```

### to_dotted()

Convert to a dot-separated string:

```rust
let path = StorePath::from_segments(["config", "database", "host"]);
assert_eq!(path.to_dotted(), "config.database.host");
```

### starts_with()

Check if a path is a prefix of another:

```rust
let parent = StorePath::from_segments(["data", "load"]);
let child = StorePath::from_segments(["data", "load", "products", "data"]);
let other = StorePath::from_segments(["stats", "aggregate"]);

assert!(child.starts_with(&parent));
assert!(!other.starts_with(&parent));
```

### contains()

Check if any segment matches a value:

```rust
let path = StorePath::from_segments(["data", "load", "products"]);
assert!(path.contains("load"));
assert!(!path.contains("stats"));
```

## Practical Examples

### Example: Iterating Over Object Keys

This example from `iterate_object_keys.rs` shows how StorePaths work with iterative namespaces:

```rust
use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // Static namespace with region data
    pipeline.add_namespace(
        NamespaceBuilder::new("config")
            .static_ns()
            .insert("regions", ObjectBuilder::new()
                .insert("us-east", "Virginia")
                .insert("us-west", "Oregon")
                .insert("eu-west", "Ireland")
                .build_scalar()
            ),
    ).await?;

    // Iterative namespace that loops over region keys
    let mut handle = pipeline.add_namespace(
        NamespaceBuilder::new("classify")
            .iterative()
            .store_path(StorePath::from_segments(["config", "regions"]))
            .scalar_object_keys(None, false)
            .iter_var("region")
            .index_var("idx"),
    ).await?;

    // Add condition command for each iteration
    handle.add_command::<ConditionCommand>("region", &condition_attrs).await?;

    // Execute pipeline
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // Access results per iteration using indexed StorePaths
    let mut idx = 0;
    loop {
        // Build path with iteration index
        let source = StorePath::from_segments(["classify", "region"])
            .with_index(idx);

        // Try to get results for this iteration
        let Some(cmd_results) = results.get_by_source(&source) else {
            break; // No more iterations
        };

        // Access specific fields
        let result = cmd_results
            .data_get(&source.with_segment("result"))
            .and_then(|r| r.as_scalar())
            .expect("Expected result");

        let matched = cmd_results
            .data_get(&source.with_segment("matched"))
            .and_then(|r| r.as_scalar())
            .expect("Expected matched");

        println!("[{}] {} (matched: {})", idx, result.1, matched.1);
        idx += 1;
    }

    Ok(())
}
```

### Example: Accessing Aggregation Results

```rust
use panopticon_core::prelude::*;

// After pipeline execution...
let results = completed.results(ResultSettings::default()).await?;

// Build path to the stats command
let stats_source = StorePath::from_segments(["stats", "products"]);

if let Some(cmd_results) = results.get_by_source(&stats_source) {
    // Access individual aggregation results
    let fields = ["row_count", "total_price", "avg_price", "max_quantity"];

    for field in fields {
        let field_path = stats_source.with_segment(field);
        if let Some(value) = cmd_results.data_get(&field_path) {
            if let Some((ty, scalar)) = value.as_scalar() {
                println!("{}: {} ({:?})", field, scalar, ty);
            }
        }
    }
}
```

## StorePath in Templates

StorePaths directly correspond to Tera template variables. The path `config.database.host` is accessed in templates as:

```
{{ config.database.host }}
```

See [Tera Templating](./tera-templating.md) for more details on template syntax.

## StorePath Diagram

```
StorePath Structure:
====================

StorePath::from_segments(["data", "load", "products", "row_count"])
                            |      |        |           |
                            v      v        v           v
                         +------+------+---------+-----------+
    segments: Vec<String>|"data"|"load"|"products"|"row_count"|
                         +------+------+---------+-----------+
                            0      1        2           3

    to_dotted() -> "data.load.products.row_count"
    namespace() -> Some("data")
    segments()  -> &["data", "load", "products", "row_count"]


Path Operations:
================

    base = ["data", "load"]
              |
              +-- with_segment("file")    -> ["data", "load", "file"]
              |
              +-- with_index(0)           -> ["data", "load", "0"]
              |
              +-- with_segment("products")
                    |
                    +-- with_segment("data") -> ["data", "load", "products", "data"]
```

## Best Practices

1. **Use meaningful segment names**: Paths should be self-documenting
   ```rust
   // Good
   StorePath::from_segments(["users", "fetch", "active_count"])

   // Less clear
   StorePath::from_segments(["u", "f", "ac"])
   ```

2. **Build paths incrementally**: Use `with_segment()` to build on base paths
   ```rust
   let base = StorePath::from_segments(["namespace", "command"]);
   let output = base.with_segment("output");
   let data = base.with_segment("data");
   ```

3. **Use `with_index()` for iteration**: Makes iteration patterns clear
   ```rust
   for i in 0..count {
       let iter_path = base.with_index(i);
       // Process iteration...
   }
   ```

4. **Store base paths as constants**: Avoid typos in repeated path construction
   ```rust
   const DATA_NS: &[&str] = &["data", "load"];
   let base = StorePath::from_segments(DATA_NS.iter().copied());
   ```

## Next Steps

Continue to [Tera Templating](./tera-templating.md) to learn how StorePaths integrate with template syntax.
