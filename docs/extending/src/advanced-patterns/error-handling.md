# Error Handling

Panopticon uses `anyhow::Result` throughout for flexible error handling. This section covers the error patterns used in built-in commands and best practices for your own implementations.

## The anyhow Approach

All command methods return `anyhow::Result<T>`:

```rust
impl FromAttributes for MyCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> { ... }
}

#[async_trait]
impl Executable for MyCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> { ... }
}
```

This provides:

- **Flexible error types**: Any error implementing `std::error::Error` can be returned
- **Context chaining**: Add context to errors as they propagate up
- **Formatted messages**: Rich error messages with `anyhow::anyhow!()` macro
- **Downcasting**: Callers can extract specific error types if needed

## Error Patterns from Built-in Commands

### Pattern 1: Early Validation with Clear Messages

Check preconditions early and fail with descriptive messages:

```rust
// From FileCommand
if !file_spec.file.exists() {
    tracing::warn!(missing_file = %file_spec.file.display(), "File does not exist");
    return Err(anyhow::anyhow!(
        "File does not exist: {}",
        file_spec.file.display()
    ));
}

if file_spec.file.is_dir() {
    tracing::warn!(directory_path = %file_spec.file.display(), "Path is a directory, not a file");
    return Err(anyhow::anyhow!(
        "Path is a directory, not a file: {}",
        file_spec.file.display()
    ));
}
```

Key points:
- Log with `tracing` before returning (helps with debugging)
- Include the problematic value in the error message
- Check multiple conditions separately for specific error messages

### Pattern 2: Using context() for Error Chains

The `context()` method adds information as errors propagate:

```rust
// From FileCommand::from_attributes
let name = file_obj
    .get_required_string("name")
    .context(format!("files[{}]", i))?;

let file = file_obj
    .get_required_string("file")
    .context(format!("files[{}]", i))?;
```

This produces error chains like:
```
Error: files[2]

Caused by:
    required field 'file' is missing
```

Use `context()` when:
- Processing arrays (add the index)
- Nested operations (add the parent context)
- The underlying error lacks location information

### Pattern 3: Converting External Errors

When calling external libraries, convert their errors with context:

```rust
// From FileCommand
polars::prelude::CsvReadOptions::default()
    .try_into_reader_with_file_path(Some(path.clone()))?
    .finish()
    .map_err(|e| {
        anyhow::anyhow!("Failed to read CSV file {}: {}", path.display(), e)
    })?

// From SqlCommand
let lazy_result = match sql_ctx.execute(&query) {
    Ok(lazy_df) => lazy_df,
    Err(e) => {
        tracing::warn!(query = %query, "SQL execution error");
        return Err(anyhow::anyhow!("SQL execution failed: {}", e));
    }
};
```

Choose between `map_err` and `match`:
- Use `map_err` for simple conversions
- Use `match` when you need to log or perform additional actions on error

### Pattern 4: Missing Required Data

When data should exist but doesn't:

```rust
// From SqlCommand
let df = context.tabular().get(&source_path).await?.ok_or_else(|| {
    anyhow::anyhow!("Table source '{}' not found in tabular store", table.source)
})?;

// From AggregateCommand
let col_name = agg.column.as_ref().ok_or_else(|| {
    anyhow::anyhow!("Aggregation '{}': n_unique requires a column", agg.name)
})?;
```

The pattern: `option.ok_or_else(|| anyhow::anyhow!(...))?`

Include enough context to identify:
- What was being looked for
- Where it was expected
- Why it matters

### Pattern 5: Async Task Errors

When spawning blocking tasks:

```rust
// From FileCommand
let df = tokio::task::spawn_blocking(move || -> Result<polars::prelude::DataFrame> {
    // ... blocking work that returns Result
})
.await
.map_err(|e| anyhow::anyhow!("Task join error: {}", e))??;
```

Note the double `?`:
1. First `?` handles the `JoinError` (task panicked or was cancelled)
2. Second `?` handles the `Result` returned by the closure

### Pattern 6: Mutually Exclusive Options

When attributes are mutually exclusive:

```rust
// From TemplateCommand
match (content, file) {
    (Some(content), None) => {
        templates.push(TemplateSource::Raw { name, content });
    }
    (None, Some(file)) => {
        templates.push(TemplateSource::File { name, path: PathBuf::from(file) });
    }
    (Some(_), Some(_)) => {
        return Err(anyhow::anyhow!(
            "templates[{}]: 'content' and 'file' are mutually exclusive",
            i
        ));
    }
    (None, None) => {
        return Err(anyhow::anyhow!(
            "templates[{}]: must specify either 'content' or 'file'",
            i
        ));
    }
}
```

Handle all cases explicitly for clear error messages.

### Pattern 7: Validation with Detailed Context

Aggregate errors with operation context:

```rust
// From AggregateCommand
let column = df.column(col_name).map_err(|e| {
    anyhow::anyhow!(
        "Aggregation '{}': column '{}' not found: {}",
        agg.name,
        col_name,
        e
    )
})?;

let count = column.n_unique().map_err(|e| {
    anyhow::anyhow!("Aggregation '{}': n_unique failed: {}", agg.name, e)
})?;
```

Include:
- The operation being performed (`n_unique`)
- The context (aggregation name)
- The underlying error

## When to Return Errors vs. Panic

### Return Errors For:

- **User input problems**: Missing files, invalid formats, wrong types
- **External failures**: Network errors, database errors, API failures
- **Business logic violations**: Mutually exclusive options, invalid combinations
- **Runtime state issues**: Missing dependencies, expired tokens

```rust
// User input - return error
if format != "csv" && format != "json" && format != "parquet" {
    return Err(anyhow::anyhow!("Unsupported file format: {}", format));
}

// External failure - return error
let response = client.get(&url).send().await
    .map_err(|e| anyhow::anyhow!("HTTP request failed: {}", e))?;
```

### Panic For:

- **Internal invariants**: States that should be impossible if the code is correct
- **Schema validation**: Caught by `CommandSpecBuilder::build()` at initialization
- **Programming errors**: Bugs in your command implementation

```rust
// From CommandSpecBuilder::build() - schema validation
let attr = self.attributes.iter()
    .find(|a| &a.name == attribute)
    .unwrap_or_else(|| panic!(
        "Derived result references unknown attribute '{:?}'",
        attribute
    ));

// Internal invariant - something is very wrong
match self.internal_state {
    State::Ready => { /* proceed */ }
    _ => unreachable!("execute() called in non-ready state"),
}
```

The rule: **panic for programmer errors, return errors for runtime problems**.

## Logging and Errors

Use `tracing` to log before returning errors when the error might be hard to diagnose:

```rust
// Log context that helps debugging
tracing::warn!(
    path = %file_spec.file.display(),
    "File does not exist"
);
return Err(anyhow::anyhow!(
    "File does not exist: {}",
    file_spec.file.display()
));

// Log the query that failed
tracing::warn!(query = %query, "SQL execution error");
return Err(anyhow::anyhow!("SQL execution failed: {}", e));
```

Logging is especially valuable when:
- The error might be intermittent
- Multiple similar operations could fail
- The error message alone doesn't capture all relevant state

## Error Message Guidelines

### Do Include:

- What operation failed
- What specific value caused the problem
- What was expected vs. what was found

```rust
// GOOD
Err(anyhow::anyhow!(
    "Aggregation '{}': column '{}' not found in DataFrame with columns {:?}",
    agg.name,
    col_name,
    df.get_column_names()
))
```

### Avoid:

- Generic messages without context
- Technical jargon when user-facing
- Stack traces in error messages (that's what `RUST_BACKTRACE` is for)

```rust
// BAD - no context
Err(anyhow::anyhow!("Column not found"))

// BAD - too technical
Err(anyhow::anyhow!("HashMap::get returned None for TypeId"))

// BAD - error in error message
Err(anyhow::anyhow!("Error: something went wrong"))
```

## Testing Error Conditions

Test that your commands fail appropriately:

```rust
#[tokio::test]
async fn test_missing_file_error() {
    let attrs = Attributes::from_json(json!({
        "files": [{
            "name": "missing",
            "file": "/nonexistent/path.csv",
            "format": "csv"
        }]
    })).unwrap();

    let cmd = FileCommand::from_attributes(&attrs).unwrap();

    let result = cmd.execute(&context, &output_prefix).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("does not exist"));
    assert!(error.to_string().contains("/nonexistent/path.csv"));
}
```

## Summary

- Use `anyhow::Result` for all fallible operations
- Add context with `.context()` when processing arrays or nested structures
- Convert external errors with `map_err` and include relevant context
- Log with `tracing` before returning hard-to-diagnose errors
- Return errors for runtime problems, panic for programming errors
- Include specific values and operation names in error messages
- Test error conditions to ensure messages are helpful
