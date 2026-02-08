# Composite Commands

A composite command is a command that internally builds and executes a nested `Pipeline` to orchestrate multiple lower-level commands. This pattern is useful when wrapping complex APIs where individual operations map cleanly to commands, but users often need higher-level workflows that combine several operations.

## The `attrs!` Macro

This chapter uses the `attrs!` macro to construct `Attributes` maps concisely. Import it from the extend prelude:

```rust
use panopticon_core::extend::attrs;

let attributes = attrs! {
    "user_id" => "abc123",
    "limit" => 100i64,
    "enabled" => true,
};
```

Values are converted via `Into<ScalarValue>`, supporting `&str`, `String`, integers, floats, and booleans.

## The Problem Composite Commands Solve

Consider an API integration where:

1. Individual API endpoints map naturally to individual commands
2. Common workflows require multiple sequential API calls
3. Users want a simple interface that hides the orchestration complexity

Without composite commands, users must manually construct pipelines for every workflow:

```rust
// User has to wire up the details every time
let mut pipeline = Pipeline::new();
let ns = pipeline.add_namespace(once("fetch")).await?;
ns.command::<GetUser>("user", attrs!{ "id" => user_id }).await?;
ns.command::<GetUserPermissions>("perms", attrs!{ "user_id" => user_id }).await?;
ns.command::<GetOrganization>("org", attrs!{ "org_id" => org_id }).await?;
// ... more boilerplate
```

A composite command encapsulates this orchestration:

```rust
// User just specifies what they want
ns.command::<GetUserContext>("context", attrs!{
    "user_id" => user_id,
    "include_org" => true
}).await?;
```

## How It Works

The key insight is that a command's `execute()` method receives an `ExecutionContext` containing `PipelineServices`. These services can be cloned and used to construct a nested pipeline that shares the same I/O and event infrastructure.

```rust
#[async_trait]
impl Executable for GetUserContext {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        // Clone services from the parent context
        let services = context.services().clone();

        // Build a nested pipeline
        let mut pipeline = Pipeline::with_services(services);

        let ns = pipeline.add_namespace(once("inner")).await?;
        ns.command::<GetUser>("user", attrs!{ "id" => &self.user_id }).await?;
        ns.command::<GetUserPermissions>("perms", attrs!{ "user_id" => &self.user_id }).await?;

        if self.include_org {
            ns.command::<GetOrganization>("org", attrs!{ "org_id" => &self.org_id }).await?;
        }

        // Compile and execute
        let completed = pipeline.compile().await?.execute().await?;

        // Extract results and write to parent context
        let results = completed.results();
        // ... marshal results to output_prefix

        Ok(())
    }
}
```

## Performance Characteristics

### Fixed Overhead Per Nested Pipeline

| Operation | Cost |
|-----------|------|
| `Pipeline::with_services()` | 3 empty Vecs, Arc clones of services |
| `ExecutionContext::new()` | 2 empty stores (`Arc<RwLock<HashMap>>`), empty Extensions |
| Hook callbacks | One async call per lifecycle event (if configured) |

### Per-Compile Overhead

| Operation | Complexity |
|-----------|------------|
| Namespace/command validation | O(n) scans |
| Dependency graph construction | O(namespaces × commands) |
| Topological sort | O(V + E) |

### When Overhead Matters (and When It Doesn't)

For **API wrappers**, the dominant cost is network latency. A nested pipeline with 5-10 commands making HTTP requests will spend 99%+ of its time waiting on I/O. The pipeline machinery overhead is negligible.

For **CPU-bound operations**, consider whether the abstraction is worth it. If commands are doing heavy computation, the validation and sorting overhead becomes more noticeable (though still typically small).

```text
API-heavy workflow (10 HTTP calls @ 100ms each):
  Network I/O: ~1000ms
  Pipeline overhead: ~1ms
  Overhead ratio: 0.1%

CPU-heavy workflow (10 data transforms @ 10ms each):
  Computation: ~100ms
  Pipeline overhead: ~1ms
  Overhead ratio: 1%
```

## Complete Example: API Wrapper

Here's a realistic example wrapping a hypothetical user management API.

### Low-Level Commands

```rust
// Individual API endpoint commands
pub struct GetUser {
    user_id: String,
}

pub struct GetUserPermissions {
    user_id: String,
}

pub struct GetOrganization {
    org_id: String,
}

pub struct GetAuditLog {
    user_id: String,
    limit: i64,
}
```

### Composite Command

```rust
pub struct GetUserWithContext {
    user_id: String,
    org_id: String,
    include_audit: bool,
    audit_limit: i64,
}

impl Descriptor for GetUserWithContext {
    fn command_type() -> &'static str { "GetUserWithContext" }

    fn command_attributes() -> &'static [AttributeSpec<&'static str>] {
        &[
            attr("user_id").scalar().required(),
            attr("org_id").scalar().required(),
            attr("include_audit").scalar().optional(),
            attr("audit_limit").scalar().optional(),
        ]
    }

    fn command_results() -> &'static [ResultSpec<&'static str>] {
        &[
            result("user").scalar(),
            result("permissions").tabular(),
            result("organization").scalar(),
            result("audit_log").tabular(),  // Only populated if include_audit
        ]
    }
}

impl FromAttributes for GetUserWithContext {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        Ok(Self {
            user_id: attrs.get_required("user_id")?,
            org_id: attrs.get_required("org_id")?,
            include_audit: attrs.get_optional("include_audit")?.unwrap_or(false),
            audit_limit: attrs.get_optional("audit_limit")?.unwrap_or(100),
        })
    }
}

#[async_trait]
impl Executable for GetUserWithContext {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        // Build nested pipeline with shared services
        let mut pipeline = Pipeline::with_services(context.services().clone());

        let ns = pipeline.add_namespace(once("fetch")).await?;

        // Core user data (always fetched)
        ns.command::<GetUser>("user", attrs!{
            "user_id" => &self.user_id
        }).await?;

        ns.command::<GetUserPermissions>("permissions", attrs!{
            "user_id" => &self.user_id
        }).await?;

        ns.command::<GetOrganization>("organization", attrs!{
            "org_id" => &self.org_id
        }).await?;

        // Optional audit log
        if self.include_audit {
            ns.command::<GetAuditLog>("audit", attrs!{
                "user_id" => &self.user_id,
                "limit" => self.audit_limit
            }).await?;
        }

        // Execute the nested pipeline
        let completed = pipeline.compile().await?.execute().await?;
        let inner_ctx = completed.results();

        // Marshal results to the parent context
        let mut batch = InsertBatch::new();

        // Copy scalar results
        if let Some(user) = inner_ctx.scalar().get(&StorePath::parse("fetch.user.data")).await? {
            batch.scalar(output_prefix.extend("user"), user);
        }

        if let Some(org) = inner_ctx.scalar().get(&StorePath::parse("fetch.organization.data")).await? {
            batch.scalar(output_prefix.extend("organization"), org);
        }

        // Copy tabular results
        if let Some(perms) = inner_ctx.tabular().get(&StorePath::parse("fetch.permissions.data")).await? {
            batch.tabular(output_prefix.extend("permissions"), perms);
        }

        if self.include_audit {
            if let Some(audit) = inner_ctx.tabular().get(&StorePath::parse("fetch.audit.data")).await? {
                batch.tabular(output_prefix.extend("audit_log"), audit);
            }
        }

        batch.commit(context).await?;

        Ok(())
    }
}
```

## Design Considerations

### Isolated Execution Contexts

The nested pipeline creates its own `ExecutionContext`. This means:

- **Data isolation**: The inner pipeline cannot directly access values from the outer context
- **Explicit marshalling**: Results must be explicitly copied from inner to outer context
- **Clean namespacing**: Inner command outputs don't pollute the outer namespace

If you need data from the outer context inside the nested pipeline, pass it through attributes or copy it into the inner context's stores before execution.

### Services Are Shared

When you use `Pipeline::with_services(context.services().clone())`:

- The same `PipelineIO` handles file operations
- The same event hooks receive lifecycle events
- The same configuration applies

This means hook implementations will see events from nested pipelines. Depending on your use case, this is either desirable (full observability) or noisy (redundant events). Consider whether your hooks should filter based on nesting depth.

### Extensions Are Not Inherited

The nested pipeline's `ExecutionContext` has its own `Extensions` instance. If your inner commands need access to shared state (HTTP clients, auth tokens), you have two options:

```rust
// Option 1: Copy extensions before execution
let inner_extensions = context.extensions().clone();
// ... then somehow inject into inner context (not directly supported)

// Option 2: Commands fetch from a well-known source
// Inner commands can look up shared state via a service or global
```

In practice, if inner commands need shared resources, consider whether they should access them through the services or whether the composite command should fetch data and pass it via attributes.

### Error Propagation

Errors from the nested pipeline bubble up naturally:

```rust
let completed = pipeline.compile().await?.execute().await?;
//                                 ^              ^
//                      Compilation errors   Execution errors
```

The `?` operator propagates errors with full context. Consider adding additional context for debugging:

```rust
let completed = pipeline
    .compile().await
    .context("Failed to compile inner pipeline for GetUserWithContext")?
    .execute().await
    .context("Failed to execute inner pipeline for GetUserWithContext")?;
```

### Cancellation Token Propagation

The nested pipeline's `Extensions` gets a fresh `CancellationToken`. If you need cancellation to propagate from outer to inner:

```rust
// Before executing inner pipeline
let outer_token = context.extensions()
    .read().await
    .get::<CancellationToken>()
    .cloned();

if let Some(token) = outer_token {
    // Check before starting
    if token.is_cancelled() {
        return Err(anyhow::anyhow!("Operation cancelled"));
    }

    // Or inject into inner context for inner commands to check
    // (requires access to inner context before execute)
}
```

## Anti-Patterns

### Deeply Nested Pipelines

Avoid pipelines within pipelines within pipelines:

```rust
// PROBLEMATIC: Hard to debug, unclear data flow
impl Executable for Level1Command {
    async fn execute(&self, ctx: &ExecutionContext, out: &StorePath) -> Result<()> {
        let mut pipeline = Pipeline::with_services(ctx.services().clone());
        // ... adds Level2Command which itself runs a pipeline with Level3Command
    }
}
```

If you find yourself nesting more than one level deep, consider flattening the structure or rethinking the abstraction boundaries.

### Overusing Composite Commands

Not every multi-step operation needs a composite command:

```rust
// OVERKILL: Just two simple operations
impl Executable for FetchAndTransform {
    async fn execute(&self, ctx: &ExecutionContext, out: &StorePath) -> Result<()> {
        let mut pipeline = Pipeline::with_services(ctx.services().clone());
        ns.command::<Fetch>(...).await?;
        ns.command::<Transform>(...).await?;
        // ...
    }
}

// SIMPLER: Just do both operations inline
impl Executable for FetchAndTransform {
    async fn execute(&self, ctx: &ExecutionContext, out: &StorePath) -> Result<()> {
        let data = fetch_data(&self.url).await?;
        let transformed = transform(data)?;
        // Write results
    }
}
```

Use composite commands when:
- The inner operations are **reusable** as standalone commands
- The orchestration logic is **complex** (conditional execution, iteration, dependencies)
- Users benefit from being able to run the **inner commands independently**

### Ignoring Inner Pipeline Errors

Don't silently swallow errors from the nested pipeline:

```rust
// WRONG: Errors are lost
if let Ok(completed) = pipeline.execute().await {
    // process results
}
// Silently continues if execution failed

// RIGHT: Propagate errors
let completed = pipeline.execute().await?;
```

## When to Use This Pattern

| Scenario | Recommendation |
|----------|----------------|
| Wrapping a REST API with many endpoints | Good fit - each endpoint is a command, workflows are composites |
| Simple two-step operations | Probably overkill - inline the logic |
| Dynamic workflows based on runtime data | Good fit - build pipeline based on conditions |
| Performance-critical inner loops | Consider alternatives - measure the overhead |
| Commands need extensive shared state | Consider Extensions or restructure |

## Summary

- Composite commands encapsulate multi-command workflows behind a single command interface
- Clone `context.services()` to share I/O and hooks with the nested pipeline
- The nested pipeline has its own `ExecutionContext` - marshal results explicitly
- Overhead is negligible for I/O-bound operations (API calls, file operations)
- Avoid deep nesting and overuse - keep the abstraction boundaries clear
- Propagate errors with context for debuggability
