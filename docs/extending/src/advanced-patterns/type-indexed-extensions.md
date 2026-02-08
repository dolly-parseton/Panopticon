# Type-Indexed Extensions

The `Extensions` type provides type-safe storage for shared state that commands can access during execution. It uses Rust's `TypeId` system to create a map where each type can have exactly one value, eliminating the need for string keys or global statics.

## The Problem Extensions Solve

Consider an API integration that needs to:

1. Make HTTP requests (requires a configured client)
2. Authenticate requests (requires access tokens)
3. Handle rate limiting (requires shared state across commands)

Without Extensions, you face awkward choices:

```rust
// Anti-pattern: Global static (not Send + Sync friendly, initialization order issues)
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| { ... });

// Anti-pattern: Pass through every function signature
async fn execute(&self, ctx: &ExecutionContext, client: &Client, token: &str) -> Result<()>

// Anti-pattern: Environment variables for everything
let token = std::env::var("API_TOKEN")?;
```

Extensions solve this by providing a type-indexed container that travels with the `ExecutionContext`:

```rust
// At pipeline setup
extensions.write().await.insert(MyHttpClient::new()?);
extensions.write().await.insert(AuthToken("bearer xyz".into()));

// In any command's execute()
let client = context.extensions()
    .read().await
    .get::<MyHttpClient>()
    .ok_or_else(|| anyhow::anyhow!("HTTP client not configured"))?;
```

## How Extensions Works

Extensions is built on `std::any::TypeId`:

```rust
pub struct Extensions {
    map: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}
```

When you insert a value, its type becomes the key:

```rust
// These are stored under different keys (TypeId::of::<T>())
extensions.insert(reqwest::Client::new());     // Key: TypeId of reqwest::Client
extensions.insert(AuthToken("...".into()));    // Key: TypeId of AuthToken
extensions.insert(RateLimiter::new(100));      // Key: TypeId of RateLimiter
```

Retrieval is type-safe - you get back exactly what you put in:

```rust
let client: Option<&reqwest::Client> = extensions.get::<reqwest::Client>();
let token: Option<&AuthToken> = extensions.get::<AuthToken>();
```

## The Default CancellationToken

Every `Extensions` instance is created with a default `tokio_util::sync::CancellationToken`:

```rust
impl Default for Extensions {
    fn default() -> Self {
        let mut map = HashMap::new();
        map.insert(
            TypeId::of::<tokio_util::sync::CancellationToken>(),
            Box::new(tokio_util::sync::CancellationToken::new()) as Box<dyn Any + Send + Sync>,
        );
        // ...
    }
}
```

This token enables cooperative cancellation across long-running pipelines. Commands can check it periodically:

```rust
async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
    for item in &self.large_dataset {
        // Check for cancellation between iterations
        if context.extensions().is_canceled().await {
            return Err(anyhow::anyhow!("Operation cancelled"));
        }
        process_item(item).await?;
    }
    Ok(())
}
```

The orchestrator can trigger cancellation:

```rust
// From pipeline control code
extensions.cancel().await;
```

## Using Extensions in Commands

### Reading Extensions

Use `read()` for shared access (multiple readers allowed):

```rust
#[async_trait]
impl Executable for MyApiCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        // Acquire read lock
        let ext = context.extensions().read().await;

        // Get typed reference
        let client = ext.get::<reqwest::Client>()
            .ok_or_else(|| anyhow::anyhow!("reqwest::Client not found in extensions"))?;

        // Use the client (lock is held for the scope of `ext`)
        let response = client.get(&self.url).send().await?;

        // Lock released when `ext` goes out of scope
        Ok(())
    }
}
```

### Writing Extensions

Use `write()` when you need to modify the extensions (exclusive access):

```rust
async fn refresh_token_if_expired(extensions: &Extensions) -> Result<()> {
    let mut ext = extensions.write().await;

    // Check current token
    let current = ext.get::<AuthToken>();
    if current.map(|t| t.is_expired()).unwrap_or(true) {
        // Remove old token
        ext.remove::<AuthToken>();

        // Insert new token
        let new_token = fetch_new_token().await?;
        ext.insert(new_token);
    }

    Ok(())
}
```

### Checking Existence

```rust
let ext = context.extensions().read().await;

if ext.contains::<DatabasePool>() {
    // Use database
} else {
    // Fall back to file-based storage
}
```

## Real-World Use Cases

### HTTP Client with Retry Policy

Wrap the client in a newtype to configure it once:

```rust
pub struct ConfiguredClient(reqwest::Client);

impl ConfiguredClient {
    pub fn new(timeout: Duration, max_retries: u32) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()?;
        Ok(Self(client))
    }

    pub fn inner(&self) -> &reqwest::Client {
        &self.0
    }
}

// At setup
extensions.write().await.insert(
    ConfiguredClient::new(Duration::from_secs(30), 3)?
);

// In commands
let client = ext.get::<ConfiguredClient>()
    .ok_or_else(|| anyhow::anyhow!("ConfiguredClient not in extensions"))?
    .inner();
```

### Database Connection Pool

```rust
pub struct DbPool(sqlx::PgPool);

impl DbPool {
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = sqlx::PgPool::connect(url).await?;
        Ok(Self(pool))
    }

    pub fn pool(&self) -> &sqlx::PgPool {
        &self.0
    }
}

// Commands can share the pool
let pool = ext.get::<DbPool>()
    .ok_or_else(|| anyhow::anyhow!("Database pool not configured"))?
    .pool();

let rows = sqlx::query("SELECT * FROM users")
    .fetch_all(pool)
    .await?;
```

### Authentication Tokens

```rust
#[derive(Clone)]
pub struct BearerToken {
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl BearerToken {
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() >= self.expires_at
    }

    pub fn header_value(&self) -> String {
        format!("Bearer {}", self.token)
    }
}

// In command
let token = ext.get::<BearerToken>()
    .ok_or_else(|| anyhow::anyhow!("Authentication token not configured"))?;

if token.is_expired() {
    return Err(anyhow::anyhow!("Authentication token has expired"));
}

let response = client
    .get(&url)
    .header("Authorization", token.header_value())
    .send()
    .await?;
```

## Anti-Patterns

### Using String Keys

Do not try to implement your own string-keyed map:

```rust
// WRONG: Loses type safety
struct StringKeyedExtensions(HashMap<String, Box<dyn Any + Send + Sync>>);

extensions.insert("client", Box::new(client));
let client = extensions.get("client").downcast_ref::<???>();  // What type?
```

The type-indexed approach means the type itself is the key - you cannot request the wrong type.

### Holding Locks Across Await Points

Be careful with lock scope:

```rust
// PROBLEMATIC: Lock held across network call
let ext = context.extensions().read().await;
let client = ext.get::<Client>().unwrap();
let response = client.get(&url).send().await?;  // Lock still held!
drop(ext);

// BETTER: Clone what you need, release lock quickly
let client = {
    let ext = context.extensions().read().await;
    ext.get::<Client>().cloned()
        .ok_or_else(|| anyhow::anyhow!("Client not found"))?
};
// Lock released, now make the network call
let response = client.get(&url).send().await?;
```

### One Type, Multiple Instances

Extensions stores **one value per type**. If you need multiple instances, wrap them:

```rust
// WRONG: Second insert overwrites first
extensions.insert(Client::new("api1.example.com"));
extensions.insert(Client::new("api2.example.com"));  // Overwrites!

// RIGHT: Use newtypes
struct Api1Client(Client);
struct Api2Client(Client);

extensions.insert(Api1Client(Client::new("api1.example.com")));
extensions.insert(Api2Client(Client::new("api2.example.com")));
```

Or use a collection:

```rust
struct ApiClients(HashMap<String, Client>);

extensions.insert(ApiClients(hashmap! {
    "api1".into() => Client::new("api1.example.com"),
    "api2".into() => Client::new("api2.example.com"),
}));
```

## When Not to Use Extensions

Extensions are for **shared, long-lived state**. They're not appropriate for:

- **Per-command configuration**: Use attributes instead
- **Data flowing through the pipeline**: Use the tabular/scalar stores
- **Temporary computation state**: Use local variables in `execute()`

```rust
// WRONG: This is per-command config, use an attribute
extensions.insert(OutputFormat::Json);

// WRONG: This is pipeline data, use the store
extensions.insert(UserList(users));

// RIGHT: Shared infrastructure
extensions.insert(HttpClient::new());
extensions.insert(AuthToken::from_env()?);
```

## Summary

- Extensions provide type-indexed storage for shared state across commands
- Use `read()` for shared access, `write()` for mutations
- Every Extensions instance includes a default `CancellationToken`
- Clone values when you need to release locks before async operations
- Use newtypes to store multiple instances of the same underlying type
- Reserve Extensions for infrastructure (clients, pools, tokens), not data
