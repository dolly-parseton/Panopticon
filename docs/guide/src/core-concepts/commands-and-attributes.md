# Commands and Attributes

Commands are the workhorses of Panopticon. Each command performs a specific operation - loading files, running SQL queries, evaluating conditions, and so on. We configure commands through attributes, which are key-value pairs that control the command's behavior.

## The Command Trait

Under the hood, a command is any type that implements three traits:

```rust
pub trait Command: FromAttributes + Descriptor + Executable {}
```

- **FromAttributes** - Constructs the command from an attribute map
- **Descriptor** - Provides metadata (type name, attribute schema, result schema)
- **Executable** - Performs the actual work during pipeline execution

Panopticon provides a blanket implementation, so any type implementing the three base traits automatically implements `Command`.

## Adding Commands to a Pipeline

Commands are always added to a namespace. The pattern looks like this:

```rust
let mut handle = pipeline
    .add_namespace(NamespaceBuilder::new("data"))
    .await?;

handle
    .add_command::<FileCommand>("load", &file_attrs)
    .await?;
```

The turbofish syntax `<FileCommand>` tells Panopticon which command type to use. The string `"load"` is the command's name within this namespace, which becomes part of the store path for its results (`data.load.*`).

## Building Attributes with ObjectBuilder

Attributes are a `HashMap<String, ScalarValue>`. While we could construct this manually, `ObjectBuilder` provides a more ergonomic interface:

```rust
let file_attrs = ObjectBuilder::new()
    .insert(
        "files",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "users")
                .insert("file", "data/users.csv")
                .insert("format", "csv")
                .build_scalar(),
        ]),
    )
    .build_hashmap();
```

### ObjectBuilder Methods

| Method | Description |
|--------|-------------|
| `new()` | Create a new empty builder |
| `insert(key, value)` | Add a key-value pair |
| `object(key, nested)` | Add a nested ObjectBuilder |
| `build_scalar()` | Convert to a `ScalarValue::Object` |
| `build_hashmap()` | Convert to `HashMap<String, ScalarValue>` |

### Nested Objects

For complex attribute structures, we can nest ObjectBuilders:

```rust
let attrs = ObjectBuilder::new()
    .insert("name", "report")
    .object("options",
        ObjectBuilder::new()
            .insert("format", "json")
            .insert("pretty", true)
    )
    .build_hashmap();
```

This produces the equivalent of:
```json
{
    "name": "report",
    "options": {
        "format": "json",
        "pretty": true
    }
}
```

### Arrays of Objects

Many commands accept arrays of configuration objects:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "aggregations",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "total")
                .insert("op", "sum")
                .insert("column", "amount")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "average")
                .insert("op", "mean")
                .insert("column", "amount")
                .build_scalar(),
        ]),
    )
    .build_hashmap();
```

## Common Attributes

All commands support a `when` attribute for conditional execution:

```rust
let attrs = ObjectBuilder::new()
    .insert("when", "data.load.status == \"success\"")
    // ... other attributes
    .build_hashmap();
```

The `when` attribute is evaluated as a Tera expression. If it evaluates to a falsy value, the command is skipped and its status is set to `"skipped"`.

## Attribute Validation

When a pipeline compiles, Panopticon validates all command attributes against their schemas. Each command type declares:

- **Required attributes** - Must be present
- **Optional attributes** - May be omitted
- **Type constraints** - Values must match expected types

If validation fails, `.compile()` returns an error describing what is wrong.

## Command Results

Every command produces results, which are stored at paths derived from the namespace and command name. All commands automatically produce:

| Result | Type | Description |
|--------|------|-------------|
| `status` | String | `"success"`, `"skipped"`, `"error"`, or `"cancelled"` |
| `duration_ms` | Number | Execution time in milliseconds |

Commands also produce their own specific results. For example, `ConditionCommand` produces:

| Result | Type | Description |
|--------|------|-------------|
| `result` | String | The value from the matched branch or default |
| `matched` | Bool | Whether a branch condition matched |
| `branch_index` | Number | Index of matched branch, or -1 for default |

## Result Kinds: Meta vs Data

Results are categorized as either **Meta** or **Data**:

- **Meta** results describe the execution (status, duration, row counts)
- **Data** results contain the actual output (query results, computed values)

This distinction matters when collecting results - we might want to include all data but only summary metadata.

## Example: ConditionCommand

Let us walk through a complete example using `ConditionCommand`:

```rust
use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // Static config providing a value to check
    pipeline
        .add_namespace(
            NamespaceBuilder::new("input")
                .static_ns()
                .insert("score", ScalarValue::Number(85.into())),
        )
        .await?;

    // Condition command to classify the score
    let condition_attrs = ObjectBuilder::new()
        .insert(
            "branches",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "excellent")
                    .insert("if", "input.score >= 90")
                    .insert("then", "Excellent work!")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "good")
                    .insert("if", "input.score >= 70")
                    .insert("then", "Good job!")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "needs_work")
                    .insert("if", "input.score >= 50")
                    .insert("then", "Keep practicing!")
                    .build_scalar(),
            ]),
        )
        .insert("default", "Please try again.")
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("classify"))
        .await?
        .add_command::<ConditionCommand>("score", &condition_attrs)
        .await?;

    // Execute
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // Get the result
    let source = StorePath::from_segments(["classify", "score"]);
    let cmd_results = results.get_by_source(&source).expect("Expected results");

    let result = cmd_results
        .data_get(&source.with_segment("result"))
        .and_then(|r| r.as_scalar())
        .expect("Expected result");

    println!("Classification: {}", result.1);  // "Good job!"

    Ok(())
}
```

## Built-in Commands

Panopticon provides several built-in commands:

| Command | Purpose |
|---------|---------|
| `FileCommand` | Load data from CSV, JSON, or Parquet files |
| `SqlCommand` | Run SQL queries against loaded DataFrames |
| `AggregateCommand` | Compute aggregations (sum, mean, count, etc.) |
| `ConditionCommand` | Evaluate conditional logic with branches |
| `TemplateCommand` | Generate text using Tera templates |

Each command has its own attribute schema documented in the [Commands](../commands/index.md) section.

## Tera Substitution in Attributes

String attributes support Tera template syntax. Before a command executes, Panopticon substitutes any `{{ ... }}` expressions with values from the scalar store:

```rust
let attrs = ObjectBuilder::new()
    .insert("query", "SELECT * FROM users WHERE region = '{{ config.region }}'")
    .build_hashmap();
```

This enables dynamic configuration based on earlier command results or static namespace values.

## Summary

The command system in Panopticon follows a consistent pattern:

1. Commands implement `FromAttributes`, `Descriptor`, and `Executable`
2. We add commands to namespaces using `add_command::<T>(name, &attrs)`
3. Attributes are built using `ObjectBuilder` for type safety
4. All commands support the `when` attribute for conditional execution
5. Results are stored at `namespace.command.field` paths
6. String attributes support Tera templating for dynamic values

This design keeps command configuration declarative while enabling powerful dynamic behavior through templating and conditional execution.
