# Implementing Traits

With the schema defined, we need to implement three traits to make our command functional:

1. **`Descriptor`** - Links the struct to its schema
2. **`FromAttributes`** - Parses attributes into struct fields
3. **`Executable`** - Performs the actual work

Together, these traits satisfy the `Command` trait, which is a blanket implementation:

```rust
// You don't implement Command directly - it's automatic
pub trait Command: FromAttributes + Descriptor + Executable {}
impl<T: FromAttributes + Descriptor + Executable> Command for T {}
```

## The Command Struct

First, define a struct to hold the parsed attribute values:

```rust
pub struct ReverseCommand {
    input: String,
}
```

The struct fields correspond to the attributes we defined in our schema. In this case, we have one attribute (`input`) that we'll store as a `String`.

**Design tip**: Store the raw attribute values in your struct. Template substitution (resolving `{{ ... }}` references) happens later in `execute()`, not during construction.

## Implementing Descriptor

The `Descriptor` trait connects your struct to its schema:

```rust
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
```

### Method Breakdown

#### `command_type()`

Returns a unique identifier for this command type. This is used in logging, error messages, and internally for command registration:

```rust
fn command_type() -> &'static str {
    "ReverseCommand"
}
```

Convention: Use the struct name as the command type.

#### `command_attributes()`

Returns the attribute specifications from the schema. Since `CommandSchema` is a tuple `(attributes, results)`, we return the first element:

```rust
fn command_attributes() -> &'static [AttributeSpec<&'static str>] {
    &REVERSE_SPEC.0  // .0 is the attributes vector
}
```

#### `command_results()`

Returns the result specifications from the schema:

```rust
fn command_results() -> &'static [ResultSpec<&'static str>] {
    &REVERSE_SPEC.1  // .1 is the results vector
}
```

### Default Methods

`Descriptor` provides several default methods you get for free:

```rust
// All attributes (including common ones like "when")
fn available_attributes() -> Vec<&'static AttributeSpec<&'static str>>

// Only required attributes
fn required_attributes() -> Vec<&'static AttributeSpec<&'static str>>

// Only optional attributes
fn optional_attributes() -> Vec<&'static AttributeSpec<&'static str>>

// All results (including common ones like "status", "duration_ms")
fn available_results() -> Vec<&'static ResultSpec<&'static str>>
```

## Implementing FromAttributes

The `FromAttributes` trait constructs your command from the provided attributes:

```rust
impl FromAttributes for ReverseCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        let input = attrs.get_required_string("input")?;
        Ok(ReverseCommand { input })
    }
}
```

### The Attributes Type

`Attributes` is a type alias for `HashMap<String, ScalarValue>`. The `ScalarMapExt` trait (automatically available via imports) provides helper methods:

```rust
// Required getters - return Err if missing or wrong type
attrs.get_required_string("key")?    // -> String
attrs.get_required_i64("key")?       // -> i64
attrs.get_required_bool("key")?      // -> bool
attrs.get_required("key")?           // -> &ScalarValue

// Optional getters - return None if missing
attrs.get_optional_string("key")     // -> Option<String>
attrs.get_optional_i64("key")        // -> Option<i64>
attrs.get_optional_bool("key")       // -> Option<bool>
attrs.get("key")                     // -> Option<&ScalarValue>
```

### Error Handling

The `?` operator propagates errors with descriptive messages:

```rust
// If "input" is missing, this returns:
// Err(anyhow::Error: missing required key 'input')
let input = attrs.get_required_string("input")?;

// If "count" is present but not an integer:
// Err(anyhow::Error: 'count' must be an integer)
let count = attrs.get_required_i64("count")?;
```

### Working with Complex Types

For optional attributes with defaults:

```rust
let separator = attrs.get_optional_string("separator")
    .unwrap_or_else(|| ",".to_string());
```

For arrays:

```rust
let values = attrs.get_required("items")?
    .as_array_or_err("items")?
    .iter()
    .map(|v| v.as_str_or_err("items[i]").map(String::from))
    .collect::<Result<Vec<_>>>()?;
```

### The ScalarAsExt Trait

When you have a `ScalarValue` and need to convert it to a specific type, use `ScalarAsExt` methods:

```rust
let value: &ScalarValue = attrs.get_required("field")?;

// These return Result with helpful error messages
value.as_str_or_err("field")?       // -> &str
value.as_i64_or_err("field")?       // -> i64
value.as_f64_or_err("field")?       // -> f64
value.as_bool_or_err("field")?      // -> bool
value.as_array_or_err("field")?     // -> &Vec<ScalarValue>
value.as_object_or_err("field")?    // -> &Map<String, ScalarValue>
```

### Default Method: extract_dependencies

`FromAttributes` provides a default implementation of `extract_dependencies()` that automatically parses Tera templates in attribute values to find store path references:

```rust
fn extract_dependencies(attrs: &Attributes) -> Result<HashSet<StorePath>> {
    // Automatically implemented based on ReferenceKind in your schema
}
```

This is used internally to build the dependency graph for execution ordering.

## Implementing Executable

The `Executable` trait defines what your command actually does:

```rust
#[async_trait]
impl Executable for ReverseCommand {
    async fn execute(
        &self,
        context: &ExecutionContext,
        output_prefix: &StorePath,
    ) -> Result<()> {
        // 1. Resolve any Tera templates in the input
        let resolved = context.substitute(&self.input).await?;

        // 2. Perform the business logic
        let reversed: String = resolved.chars().rev().collect();
        let length = resolved.chars().count() as u64;

        // 3. Write results to the store
        let out = InsertBatch::new(context, output_prefix);
        out.string("reversed", reversed).await?;
        out.u64("length", length).await?;

        Ok(())
    }
}
```

### The async_trait Macro

Since `Executable` is an async trait, you need the `#[async_trait]` attribute:

```rust
use panopticon_core::extend::async_trait;  // Re-exported for convenience

#[async_trait]
impl Executable for ReverseCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        // ...
    }
}
```

### Method Parameters

#### `&self`

Your command instance with parsed attribute values. This was created by `from_attributes()`.

#### `context: &ExecutionContext`

The execution context provides:

- **Template substitution**: `context.substitute(&template).await?`
- **Scalar store access**: `context.scalar()` for reading/writing scalar values
- **Tabular store access**: `context.tabular()` for reading/writing DataFrames
- **Extensions**: `context.extensions()` for cancellation checks and custom services

#### `output_prefix: &StorePath`

The store path where this command should write its results. For a command named `reverse` in namespace `demo`, this would be `demo.reverse`.

Your results should be written as children of this prefix:
- `demo.reverse.reversed`
- `demo.reverse.length`

### Template Substitution

If your attribute supports Tera templates (`ReferenceKind::StaticTeraTemplate`), resolve them using `context.substitute()`:

```rust
// self.input might be "{{ inputs.greeting }}"
let resolved = context.substitute(&self.input).await?;
// resolved is now "Hello, world!" (the actual value from the store)
```

This is crucial - without substitution, you'd operate on the literal template string.

### Writing Results with InsertBatch

`InsertBatch` provides a convenient API for writing results under the output prefix:

```rust
let out = InsertBatch::new(context, output_prefix);

// Write different types
out.string("name", "value".to_string()).await?;
out.i64("count", 42).await?;
out.u64("size", 100).await?;
out.f64("ratio", 0.75).await?;
out.bool("success", true).await?;
out.null("empty").await?;

// Write arbitrary ScalarValue
out.scalar("data", some_scalar_value).await?;

// Write tabular data
out.tabular("table", dataframe).await?;
```

Each method writes to `output_prefix.segment`. For example, with `output_prefix = demo.reverse`:

```rust
out.string("reversed", reversed).await?;  // Writes to demo.reverse.reversed
out.u64("length", length).await?;          // Writes to demo.reverse.length
```

### Checking for Cancellation

For long-running commands, periodically check if the pipeline was cancelled:

```rust
#[async_trait]
impl Executable for LongRunningCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        for item in items {
            // Check cancellation before each iteration
            if context.extensions().is_canceled().await {
                return Ok(());  // Exit gracefully
            }

            // Process item...
        }
        Ok(())
    }
}
```

### Return Value

Return `Ok(())` on success. The wrapper automatically sets:
- `status` to `"success"`
- `duration_ms` to the execution time

Return `Err(...)` on failure. The wrapper automatically sets:
- `status` to `"error"`
- `duration_ms` to the execution time

The error is propagated up to the pipeline.

## Complete Implementation

Here's everything together:

```rust
use panopticon_core::extend::*;
use panopticon_core::prelude::*;

// Schema (from previous section)
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

// Command struct
pub struct ReverseCommand {
    input: String,
}

// Descriptor implementation
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

// FromAttributes implementation
impl FromAttributes for ReverseCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        let input = attrs.get_required_string("input")?;
        Ok(ReverseCommand { input })
    }
}

// Executable implementation
#[async_trait]
impl Executable for ReverseCommand {
    async fn execute(
        &self,
        context: &ExecutionContext,
        output_prefix: &StorePath,
    ) -> Result<()> {
        // Resolve Tera templates
        let resolved = context.substitute(&self.input).await?;

        // Business logic
        let reversed: String = resolved.chars().rev().collect();
        let length = resolved.chars().count() as u64;

        // Write results
        let out = InsertBatch::new(context, output_prefix);
        out.string("reversed", reversed).await?;
        out.u64("length", length).await?;

        Ok(())
    }
}
```

## Next Steps

Our command is complete. Now let's [test it in a pipeline](./testing-command.md) to see it in action.
