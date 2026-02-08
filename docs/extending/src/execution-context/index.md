# Working with ExecutionContext

The `ExecutionContext` is passed to your command's `execute` method and provides access to runtime resources needed during command execution. It serves as the central hub for:

- Reading and writing scalar values
- Reading and writing tabular data
- Template substitution with Tera
- Shared state via type-indexed extensions
- Pipeline services (IO and hooks)

## The ExecutionContext Struct

```rust
pub struct ExecutionContext {
    services: PipelineServices,
    extensions: Extensions,
    scalar_store: ScalarStore,
    tabular_store: TabularStore,
}
```

All methods on `ExecutionContext` are designed to be called from async contexts and use internal `RwLock`s to ensure safe concurrent access.

## Accessing Data Stores

### ScalarStore

The `ScalarStore` holds key-value pairs where values are JSON-compatible scalars (strings, numbers, booleans, nulls, arrays, and objects). Access it via `context.scalar()`:

```rust
#[async_trait]
impl Executable for MyCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        let scalar_store = context.scalar();

        // Insert a value
        let path = output_prefix.with_segment("my_result");
        scalar_store.insert(&path, ScalarValue::String("hello".to_string())).await?;

        // Retrieve a value
        if let Some(value) = scalar_store.get(&path).await? {
            println!("Retrieved: {:?}", value);
        }

        Ok(())
    }
}
```

### TabularStore

The `TabularStore` holds Polars DataFrames for tabular data. Access it via `context.tabular()`:

```rust
use polars::prelude::*;

async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
    let tabular_store = context.tabular();

    // Create a DataFrame
    let df = df![
        "name" => ["Alice", "Bob"],
        "score" => [95, 87]
    ]?;

    // Insert the DataFrame
    let path = output_prefix.with_segment("results_table");
    tabular_store.insert(&path, df).await?;

    // Retrieve later
    if let Some(retrieved_df) = tabular_store.get(&path).await? {
        println!("Rows: {}", retrieved_df.height());
    }

    Ok(())
}
```

### Using InsertBatch for Convenience

The `InsertBatch` helper provides a cleaner API for writing multiple values under a common prefix:

```rust
async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
    let out = InsertBatch::new(context, output_prefix);

    // These insert at output_prefix.reversed, output_prefix.length, etc.
    out.string("reversed", "dlrow olleh".to_string()).await?;
    out.u64("length", 11).await?;
    out.bool("success", true).await?;
    out.f64("processing_time", 0.042).await?;

    // For tabular data
    let df = /* ... */;
    out.tabular("data", df).await?;

    Ok(())
}
```

Available `InsertBatch` methods:
- `string(segment, String)` - Insert a string value
- `i64(segment, i64)` - Insert a signed integer
- `u64(segment, u64)` - Insert an unsigned integer
- `f64(segment, f64)` - Insert a floating point number
- `bool(segment, bool)` - Insert a boolean
- `null(segment)` - Insert a null value
- `scalar(segment, ScalarValue)` - Insert any ScalarValue
- `tabular(segment, DataFrame)` - Insert a DataFrame

## Template Substitution

The `substitute` method resolves Tera template expressions against the current scalar store:

```rust
async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
    // Assume self.input contains "Hello, {{ inputs.name }}!"
    // And inputs.name was set to "World" in a static namespace

    let resolved = context.substitute(&self.input).await?;
    // resolved == "Hello, World!"

    Ok(())
}
```

This is particularly useful when command attributes support Tera template references. The substitution happens against all values currently in the scalar store, allowing commands to reference:

- Values from static namespaces
- Results from previously executed commands
- Any other scalar data in the context

### Template Syntax

Templates use the Tera templating language:

```
{{ namespace.key }}              # Simple variable access
{{ namespace.nested.value }}     # Nested object access
{{ value | upper }}              # Filters
{{ value | default(value="N/A") }}  # Default values
```

## Using Extensions for Shared State

The `Extensions` type provides a type-indexed registry for sharing state across commands. This is useful for:

- HTTP clients that should be reused
- Authentication tokens
- Database connection pools
- Cancellation tokens
- Any shared runtime state

### Reading from Extensions

```rust
// Define your extension type
struct HttpClient(reqwest::Client);

async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
    // Acquire a read lock
    let extensions = context.extensions().read().await;

    // Get the typed extension (returns Option<&T>)
    if let Some(client) = extensions.get::<HttpClient>() {
        let response = client.0.get("https://api.example.com/data").send().await?;
        // ... process response
    }

    // Check if an extension exists
    if extensions.contains::<HttpClient>() {
        // ...
    }

    Ok(())
}
```

### Writing to Extensions

```rust
async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
    // Acquire a write lock
    let mut extensions = context.extensions().write().await;

    // Insert a new extension
    extensions.insert(HttpClient(reqwest::Client::new()));

    // Modify an existing extension
    if let Some(state) = extensions.get_mut::<MyState>() {
        state.counter += 1;
    }

    // Remove an extension
    let removed: Option<HttpClient> = extensions.remove::<HttpClient>();

    Ok(())
}
```

### Built-in Extensions

By default, `Extensions` includes a `CancellationToken` from `tokio_util`. You can check for cancellation in long-running operations:

```rust
async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
    // Check if cancellation was requested
    if context.extensions().is_canceled().await {
        return Err(anyhow::anyhow!("Operation cancelled"));
    }

    // For long loops, check periodically
    for item in large_dataset {
        if context.extensions().is_canceled().await {
            break;
        }
        // ... process item
    }

    Ok(())
}
```

### Extension Type Requirements

Extension types must satisfy:
- `Send + Sync + 'static` - Safe to share across threads
- Each type can only have one instance in the registry (indexed by `TypeId`)

A common pattern is to wrap standard types in newtypes:

```rust
// Wrap reqwest::Client so it has a unique TypeId
struct ApiClient(reqwest::Client);

// Wrap a String to store an auth token
struct AuthToken(String);

// Now these can coexist in Extensions
extensions.insert(ApiClient(reqwest::Client::new()));
extensions.insert(AuthToken("bearer xyz...".to_string()));
```

## Accessing PipelineServices

The `services()` method provides access to `PipelineServices`, which manages IO and event hooks:

```rust
async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
    let services = context.services();

    // Send a notification to all registered IO services
    services.notify("Processing started...").await?;

    // Prompt for user input (returns first response from any IO service)
    if let Some(response) = services.prompt("Continue? (y/n)").await? {
        if response != "y" {
            return Ok(());
        }
    }

    Ok(())
}
```

### PipelineIO Trait

Commands can interact with users through any registered `PipelineIO` implementation:

- `notify(message)` - Send a one-way notification
- `prompt(message)` - Request input and wait for a response

Multiple IO services can be registered (CLI, GUI, channel-based, etc.). Notifications go to all services; prompts return the first response.

## Complete Example

Here is a complete command demonstrating multiple `ExecutionContext` features:

```rust
pub struct FetchDataCommand {
    url: String,
    output_name: String,
}

#[async_trait]
impl Executable for FetchDataCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        // Resolve any templates in the URL
        let url = context.substitute(&self.url).await?;

        // Get the HTTP client from extensions
        let client = {
            let extensions = context.extensions().read().await;
            extensions.get::<HttpClient>()
                .map(|c| c.0.clone())
                .unwrap_or_else(reqwest::Client::new)
        };

        // Notify the user
        context.services().notify(&format!("Fetching data from {}", url)).await?;

        // Check for cancellation before the request
        if context.extensions().is_canceled().await {
            return Err(anyhow::anyhow!("Cancelled before fetch"));
        }

        // Make the request
        let response = client.get(&url).send().await?;
        let status = response.status().as_u16();
        let body = response.text().await?;

        // Write results using InsertBatch
        let out = InsertBatch::new(context, output_prefix);
        out.string(&self.output_name, body).await?;
        out.u64("status_code", status as u64).await?;
        out.bool("success", status >= 200 && status < 300).await?;

        Ok(())
    }
}
```

## Summary

| Method | Returns | Purpose |
|--------|---------|---------|
| `scalar()` | `&ScalarStore` | Access scalar key-value storage |
| `tabular()` | `&TabularStore` | Access DataFrame storage |
| `substitute(template)` | `Result<String>` | Resolve Tera templates |
| `extensions()` | `&Extensions` | Access type-indexed shared state |
| `services()` | `&PipelineServices` | Access IO and hook services |

The async nature of store operations ensures thread-safe access when commands execute concurrently. Always `.await` the store methods and handle potential errors appropriately.
