# Testing Your Command

With our `ReverseCommand` fully implemented, let's integrate it into a pipeline and verify it works correctly.

## Creating the Pipeline

### Basic Setup

Start by creating a pipeline and adding your command:

```rust
use panopticon_core::extend::*;
use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a new pipeline
    let mut pipeline = Pipeline::new();

    // Add a namespace for our command
    pipeline
        .add_namespace(NamespaceBuilder::new("demo"))
        .await?;

    // Add the command with attributes
    let attrs = ObjectBuilder::new()
        .insert("input", "Hello, world!")
        .build_hashmap();

    pipeline
        .add_command::<ReverseCommand>("reverse", &attrs)
        .await?;

    // Compile and execute
    let completed = pipeline.compile().await?.execute().await?;

    Ok(())
}
```

### Using ObjectBuilder for Attributes

`ObjectBuilder` provides a fluent API for constructing attribute maps:

```rust
let attrs = ObjectBuilder::new()
    .insert("input", "Hello, world!")        // String value
    .insert("count", 42)                      // Number value
    .insert("enabled", true)                  // Boolean value
    .build_hashmap();                         // -> HashMap<String, ScalarValue>
```

The `build_hashmap()` method converts the builder into the `Attributes` type expected by `add_command()`.

## Using Tera Templates

One of Panopticon's powerful features is referencing values from other namespaces using Tera templates.

### Setting Up Static Input Data

Create a static namespace with seed values:

```rust
pipeline
    .add_namespace(
        NamespaceBuilder::new("inputs")
            .static_ns()  // Mark as static (no commands, just data)
            .insert("greeting", ScalarValue::String("Hello, world!".to_string())),
    )
    .await?;
```

### Referencing Values in Templates

Now reference that value in your command:

```rust
pipeline
    .add_namespace(NamespaceBuilder::new("demo"))
    .await?;

// Use Tera template syntax to reference the static value
let attrs = ObjectBuilder::new()
    .insert("input", "{{ inputs.greeting }}")
    .build_hashmap();

pipeline
    .add_command::<ReverseCommand>("reverse", &attrs)
    .await?;
```

When `execute()` runs, `context.substitute(&self.input)` resolves `{{ inputs.greeting }}` to `"Hello, world!"`.

## Executing the Pipeline

### The Pipeline Lifecycle

```rust
// 1. Build phase - add namespaces and commands
let mut pipeline = Pipeline::new();
// ... add namespaces and commands ...

// 2. Compile phase - validate and build execution plan
let ready = pipeline.compile().await?;

// 3. Execute phase - run all commands in dependency order
let completed = ready.execute().await?;
```

### Accessing Results

After execution, retrieve results using the `ResultStore`:

```rust
let results = completed.results(ResultSettings::default()).await?;
```

### Querying by Source Path

Each command's results are stored under its source path (`namespace.command_name`):

```rust
// Build the source path
let source = StorePath::from_segments(["demo", "reverse"]);

// Get all results for this command
let cmd_results = results
    .get_by_source(&source)
    .expect("Expected demo.reverse results");
```

### Retrieving Individual Results

#### Data Results

Use `data_get()` for results marked as `ResultKind::Data`:

```rust
let reversed = cmd_results
    .data_get(&source.with_segment("reversed"))
    .and_then(|r| r.as_scalar())
    .expect("Expected reversed result");

// reversed is a tuple: (StorePath, &ScalarValue)
println!("Reversed: {}", reversed.1);
```

#### Metadata Results

Use `meta_get()` for results marked as `ResultKind::Meta`:

```rust
let length = cmd_results
    .meta_get(&source.with_segment("length"))
    .expect("Expected length metadata");

println!("Length: {}", length);

// Common metadata (automatically added)
let status = cmd_results
    .meta_get(&source.with_segment("status"))
    .expect("Expected status");

let duration = cmd_results
    .meta_get(&source.with_segment("duration_ms"))
    .expect("Expected duration");

println!("Status: {}", status);
println!("Duration: {}ms", duration);
```

## Complete Working Example

Here's the full example that demonstrates everything:

```rust
//! Example: Implementing a custom Command
//!
//! Run with: cargo run --example custom_command

use panopticon_core::extend::*;
use panopticon_core::prelude::*;

// ─── Schema Definition ─────────────────────────────────────────────────────

static REVERSE_SPEC: CommandSchema = LazyLock::new(|| {
    CommandSpecBuilder::new()
        .attribute(
            AttributeSpecBuilder::new("input", TypeDef::Scalar(ScalarType::String))
                .required()
                .hint("String to reverse (supports Tera template substitution)")
                .reference(ReferenceKind::StaticTeraTemplate)
                .build(),
        )
        .fixed_result(
            "reversed",
            TypeDef::Scalar(ScalarType::String),
            Some("The reversed string"),
            ResultKind::Data,
        )
        .fixed_result(
            "length",
            TypeDef::Scalar(ScalarType::Number),
            Some("Character count of the input"),
            ResultKind::Meta,
        )
        .build()
});

// ─── Command Struct ────────────────────────────────────────────────────────

pub struct ReverseCommand {
    input: String,
}

// ─── Descriptor ────────────────────────────────────────────────────────────

impl Descriptor for ReverseCommand {
    fn command_type() -> &'static str {
        "ReverseCommand"
    }

    fn command_attributes() -> &'static [AttributeSpec<&'static str>] {
        &REVERSE_SPEC.0
    }

    fn command_results() -> &'static [ResultSpec<&'static str>] {
        &REVERSE_SPEC.1
    }
}

// ─── FromAttributes ────────────────────────────────────────────────────────

impl FromAttributes for ReverseCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        let input = attrs.get_required_string("input")?;
        Ok(ReverseCommand { input })
    }
}

// ─── Executable ────────────────────────────────────────────────────────────

#[async_trait]
impl Executable for ReverseCommand {
    async fn execute(
        &self,
        context: &ExecutionContext,
        output_prefix: &StorePath,
    ) -> Result<()> {
        // Resolve any Tera templates in the input
        let resolved = context.substitute(&self.input).await?;

        // Perform the transformation
        let reversed: String = resolved.chars().rev().collect();
        let length = resolved.chars().count() as u64;

        // Write results
        let out = InsertBatch::new(context, output_prefix);
        out.string("reversed", reversed).await?;
        out.u64("length", length).await?;

        Ok(())
    }
}

// ─── Pipeline Demo ─────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create the pipeline
    let mut pipeline = Pipeline::new();

    // Add static input data
    pipeline
        .add_namespace(
            NamespaceBuilder::new("inputs")
                .static_ns()
                .insert("greeting", ScalarValue::String("Hello, world!".to_string())),
        )
        .await?;

    // Add namespace with our custom command
    pipeline
        .add_namespace(NamespaceBuilder::new("demo"))
        .await?;

    // Configure the command with a Tera template reference
    let attrs = ObjectBuilder::new()
        .insert("input", "{{ inputs.greeting }}")
        .build_hashmap();

    pipeline
        .add_command::<ReverseCommand>("reverse", &attrs)
        .await?;

    // Execute the pipeline
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // Retrieve and display results
    let source = StorePath::from_segments(["demo", "reverse"]);
    let cmd_results = results
        .get_by_source(&source)
        .expect("Expected demo.reverse results");

    // Get the reversed string (Data result)
    let reversed = cmd_results
        .data_get(&source.with_segment("reversed"))
        .and_then(|r| r.as_scalar())
        .expect("Expected reversed result");

    // Get metadata
    let length = cmd_results
        .meta_get(&source.with_segment("length"))
        .expect("Expected length metadata");

    let status = cmd_results
        .meta_get(&source.with_segment("status"))
        .expect("Expected status metadata");

    // Print results
    println!("Original: Hello, world!");
    println!("Reversed: {}", reversed.1);
    println!("Length:   {}", length);
    println!("Status:   {}", status);

    Ok(())
}
```

### Expected Output

```text
Original: Hello, world!
Reversed: !dlrow ,olleH
Length:   13
Status:   success
```

## Testing with the "when" Condition

All commands support a `when` attribute for conditional execution:

```rust
let attrs = ObjectBuilder::new()
    .insert("input", "{{ inputs.greeting }}")
    .insert("when", "inputs.should_run")  // Only run if truthy
    .build_hashmap();
```

If the condition evaluates to false, the command is skipped and `status` is set to `"skipped"`.

## Writing Unit Tests

For unit testing your command logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_reverse_command() {
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(NamespaceBuilder::new("test"))
            .await
            .unwrap();

        let attrs = ObjectBuilder::new()
            .insert("input", "abc")
            .build_hashmap();

        pipeline
            .add_command::<ReverseCommand>("rev", &attrs)
            .await
            .unwrap();

        let completed = pipeline.compile().await.unwrap().execute().await.unwrap();
        let results = completed.results(ResultSettings::default()).await.unwrap();

        let source = StorePath::from_segments(["test", "rev"]);
        let cmd_results = results.get_by_source(&source).unwrap();

        let reversed = cmd_results
            .data_get(&source.with_segment("reversed"))
            .and_then(|r| r.as_scalar())
            .unwrap();

        assert_eq!(reversed.1.as_str().unwrap(), "cba");
    }
}
```

## Summary

You have now built a complete custom command that:

1. Declares its schema with `CommandSpecBuilder` and `AttributeSpecBuilder`
2. Implements `Descriptor` to link the struct to its schema
3. Implements `FromAttributes` to parse input attributes
4. Implements `Executable` to perform the actual work
5. Integrates with the pipeline for execution and result retrieval

## Next Steps

- Learn about the [Spec System](../spec-system/index.md) for advanced schema features
- Explore [Working with ExecutionContext](../execution-context/index.md) for more context capabilities
- See [Advanced Patterns](../advanced-patterns/index.md) for derived results and complex scenarios
