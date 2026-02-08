# Pipeline State Machine

The `Pipeline` type in Panopticon uses a compile-time state machine to enforce correct usage. This pattern prevents entire categories of bugs - we cannot accidentally execute an incomplete pipeline, modify one that is running, or access results that do not yet exist.

## The Three States

```
┌─────────┐    compile()    ┌─────────┐    execute()    ┌───────────┐
│  Draft  │ ──────────────▶ │  Ready  │ ──────────────▶ │ Completed │
└─────────┘                 └─────────┘                 └───────────┘
     ▲                           │                           │
     │         edit()            │          edit()           │
     └───────────────────────────┴───────────────────────────┘
```

### Draft

A `Pipeline<Draft>` is under construction. In this state we can:

- Add namespaces with `add_namespace()`
- Add commands to namespaces via the returned `NamespaceHandle`
- Configure services and options

We **cannot** execute a Draft pipeline. The `.execute()` method simply does not exist on this type.

### Ready

A `Pipeline<Ready>` has passed validation and is prepared for execution. The transition from Draft to Ready happens via `.compile()`, which performs several checks:

- Namespace names are unique and not reserved
- Command names are unique within their namespace
- Iterative namespaces have valid store paths
- Command attributes pass schema validation
- The execution plan is valid (no circular dependencies)

From Ready, we can either:
- Call `.execute()` to run the pipeline
- Call `.edit()` to return to Draft state for modifications

### Completed

A `Pipeline<Completed>` has finished executing all commands. The execution context containing all results is stored in the Completed state. From here we can:

- Call `.results()` to collect outputs into a `ResultStore`
- Call `.restart()` to return to Ready state and re-execute
- Call `.edit()` to return to Draft state and add more commands

## Why a State Machine?

This design is intentional. Consider what could go wrong without it:

**Without state machine:**
```rust
// Hypothetical bad API - don't do this
let pipeline = Pipeline::new();
pipeline.add_command(...);
let results = pipeline.execute();  // What if add_command() failed?
pipeline.add_command(...);         // Modifying during execution?
let more_results = pipeline.results();  // Which execution?
```

**With state machine:**
```rust
// The actual API - compile-time guarantees
let mut pipeline = Pipeline::new();           // Draft
pipeline.add_namespace(...).await?;           // Still Draft

let ready = pipeline.compile().await?;        // Ready - validated!
let completed = ready.execute().await?;       // Completed - all commands ran

let results = completed.results(...).await?;  // Results available
let pipeline = completed.edit();              // Back to Draft
```

The type system prevents us from calling methods that do not make sense in the current state. If we try to call `.execute()` on a Draft pipeline, we get a compile error - not a runtime panic.

## Practical Example: Pipeline Reuse

One powerful pattern enabled by the state machine is incremental pipeline building. We can execute a pipeline, inspect results, then add more processing steps:

```rust
use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ===== Pass 1: Load data and query =====
    println!("=== Pass 1: Load + Query ===\n");

    let mut pipeline = Pipeline::new();

    // Load users
    let file_attrs = ObjectBuilder::new()
        .insert(
            "files",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "users")
                    .insert("file", "fixtures/users.csv")
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

    // Query: all users sorted by age
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

    // Execute pass 1: Draft -> Ready -> Completed
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;
    println!("  Namespaces in pass 1: data, query");

    // ===== Pass 2: Edit pipeline, add aggregation, re-execute =====
    println!("\n=== Pass 2: Edit + Aggregate ===\n");

    // Return to Draft state - all previous namespaces and commands preserved
    let mut pipeline = completed.edit();

    // Add an aggregation namespace to the existing pipeline
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
    println!("  Namespaces in pass 2: data, query, stats");
    println!("\nPipeline successfully edited and re-executed.");

    Ok(())
}
```

The key insight is that calling `.edit()` on a Completed pipeline returns us to Draft while preserving all existing namespaces and commands. We can then add more processing steps and re-execute.

## State Transitions Summary

| From | To | Method | What Happens |
|------|-----|--------|--------------|
| Draft | Ready | `.compile()` | Validates pipeline configuration |
| Ready | Completed | `.execute()` | Runs all commands |
| Ready | Draft | `.edit()` | Returns to editing mode |
| Completed | Draft | `.edit()` | Returns to editing mode |
| Completed | Ready | `.restart()` | Clears results, ready to re-execute |

## Implementation Details

For those curious about the implementation, Panopticon uses Rust's type system to encode states:

```rust
// Marker types for states
pub struct Draft;
pub struct Ready;
pub struct Completed {
    context: ExecutionContext,
}

// Generic pipeline parameterized by state
pub struct Pipeline<T = Draft> {
    pub(crate) services: PipelineServices,
    pub(crate) namespaces: Vec<Namespace>,
    pub(crate) commands: Vec<CommandSpec>,
    state: T,
}
```

Each state has its own `impl Pipeline<State>` block defining only the methods valid for that state. The `Completed` state holds the `ExecutionContext` containing all results, which is why `.results()` is only available on `Pipeline<Completed>`.

This pattern is sometimes called the "typestate" pattern in Rust. It moves invariant checking from runtime to compile time, resulting in APIs that are impossible to misuse.
