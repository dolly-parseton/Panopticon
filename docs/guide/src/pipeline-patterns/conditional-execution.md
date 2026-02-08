# Conditional Execution

**Problem**: You want certain commands to execute only when specific conditions are met at runtime.

**Solution**: Use the `when` attribute with a Tera expression. If it evaluates to a falsy value, the command is skipped.

## How the `when` Attribute Works

Every command in Panopticon supports an optional `when` attribute:

1. Before executing the command, the runtime evaluates the `when` expression
2. If the result is falsy (`false`, `null`, empty string, `0`), the command is skipped
3. Skipped commands have `status = "skipped"` in their metadata
4. Data results are absent for skipped commands

## Basic Pattern: Feature Flags

The most common use case is feature-flagging parts of your pipeline.

```rust
use panopticon_core::prelude::*;

async fn run_with_feature_flag(enabled: bool) -> anyhow::Result<()> {
    let mut pipeline = Pipeline::with_services(PipelineServices::defaults());

    // Static namespace with configuration
    pipeline
        .add_namespace(
            NamespaceBuilder::new("inputs")
                .static_ns()
                .insert("feature_enabled", ScalarValue::Bool(enabled))
                .insert("user_name", ScalarValue::String("Alice".to_string())),
        )
        .await?;

    // Command with `when` guard
    let attrs = ObjectBuilder::new()
        .insert("when", "inputs.feature_enabled")  // <-- The condition
        .insert(
            "branches",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "greeting")
                    .insert("if", "true")
                    .insert("then", "Hello, {{ inputs.user_name }}! Feature is active.")
                    .build_scalar(),
            ]),
        )
        .insert("default", "Fallback message")
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("example"))
        .await?
        .add_command::<ConditionCommand>("greeting", &attrs)
        .await?;

    // Execute
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // Check the status
    let source = StorePath::from_segments(["example", "greeting"]);
    let cmd_results = results
        .get_by_source(&source)
        .expect("Expected results");

    let status = cmd_results
        .meta_get(&source.with_segment("status"))
        .expect("Expected status");

    println!("status = {}", status);

    // Data is only present when the command executed
    if let Some(result) = cmd_results
        .data_get(&source.with_segment("result"))
        .and_then(|r| r.as_scalar())
    {
        println!("result = {}", result.1);
    } else {
        println!("(no data - command was skipped)");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Feature flag: TRUE ===");
    run_with_feature_flag(true).await?;

    println!("\n=== Feature flag: FALSE ===");
    run_with_feature_flag(false).await?;

    Ok(())
}
```

**Output**:
```
=== Feature flag: TRUE ===
status = "completed"
result = Hello, Alice! Feature is active.

=== Feature flag: FALSE ===
status = "skipped"
(no data - command was skipped)
```

## Tera Expression Syntax

The `when` value is a Tera expression that has access to the entire scalar store. Common patterns:

### Boolean Checks

```rust
// Direct boolean value
.insert("when", "config.debug_mode")

// Negation
.insert("when", "not config.production")

// Comparison
.insert("when", "config.log_level == \"debug\"")
```

### Numeric Comparisons

```rust
// Greater than
.insert("when", "stats.row_count > 0")

// Range check
.insert("when", "inputs.threshold >= 10 and inputs.threshold <= 100")
```

### String Tests

```rust
// Equality
.insert("when", "config.environment == \"production\"")

// Prefix check
.insert("when", "region is starting_with(\"us-\")")

// Contains
.insert("when", "config.tags is containing(\"important\")")
```

### Existence Checks

```rust
// Check if a value is defined
.insert("when", "optional_config is defined")

// Check for null
.insert("when", "maybe_value is not none")
```

### Combined Conditions

```rust
// AND
.insert("when", "config.enabled and stats.count > 0")

// OR
.insert("when", "config.mode == \"full\" or config.force")

// Complex
.insert("when", "(env == \"prod\" or env == \"staging\") and feature_flags.new_flow")
```

## Handling Skipped Commands

When a command is skipped, you need to handle its absence in downstream commands.

### Check Status in Results

```rust
let status = cmd_results
    .meta_get(&source.with_segment("status"))
    .expect("Expected status");

match status.as_str() {
    Some("completed") => {
        // Process data results
    }
    Some("skipped") => {
        // Handle skipped case
    }
    _ => {
        // Unexpected status
    }
}
```

### Use Tera Defaults in Templates

When referencing potentially-skipped command outputs in templates:

```rust
// Use default filter to handle missing values
.insert("message", "Count: {{ stats.count | default(value=0) }}")
```

### Chain Conditions

If command B depends on command A's output, and A might be skipped:

```rust
// Command A: might be skipped
let a_attrs = ObjectBuilder::new()
    .insert("when", "config.run_expensive_query")
    // ...
    .build_hashmap();

handle.add_command::<SqlCommand>("query_a", &a_attrs).await?;

// Command B: only runs if A ran successfully
let b_attrs = ObjectBuilder::new()
    .insert("when", "ns.query_a.status == \"completed\"")
    // ...
    .build_hashmap();

handle.add_command::<AggregateCommand>("aggregate_b", &b_attrs).await?;
```

## Use Cases

### Debug-Only Commands

Skip verbose logging in production:

```rust
.insert("when", "config.debug")
```

### Empty Data Guards

Skip processing when there is no data:

```rust
.insert("when", "data.load.rows > 0")
```

### Environment-Specific Logic

Run different commands per environment:

```rust
// Production-only
.insert("when", "config.env == \"production\"")

// Development-only
.insert("when", "config.env == \"development\"")
```

### Conditional Exports

Only export when there are results worth saving:

```rust
.insert("when", "stats.significant_findings > 0")
```

### Iteration Guards

Within an iterative namespace, skip certain items:

```rust
// Skip disabled regions
.insert("when", "region is not starting_with(\"disabled-\")")

// Only process items meeting criteria
.insert("when", "item.status == \"active\"")
```

## Best Practices

### Keep Conditions Simple

Complex conditions are hard to debug. Prefer:

```rust
// Good: simple, readable
.insert("when", "config.feature_enabled")

// Avoid: complex nested logic
.insert("when", "((a and b) or (c and not d)) and (e or f)")
```

If you need complex logic, compute a boolean in an earlier command and reference it.

### Document Skip Behavior

When a command might be skipped, document what happens:

```rust
// This command is skipped when feature_x is disabled.
// Downstream commands that reference its output use default values.
let attrs = ObjectBuilder::new()
    .insert("when", "config.feature_x")
    // ...
```

### Test Both Paths

Always test your pipeline with conditions evaluating to both true and false to ensure proper handling.

### Use Status Checks for Dependencies

Instead of duplicating conditions, check the upstream command's status:

```rust
// Instead of repeating the condition:
// .insert("when", "config.feature_x")

// Check if the dependency actually ran:
.insert("when", "upstream.cmd.status == \"completed\"")
```

This ensures consistency even if the original condition logic changes.
