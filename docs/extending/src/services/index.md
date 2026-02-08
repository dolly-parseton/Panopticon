# Services Overview

Pipeline services provide extensibility points for interacting with the outside world during pipeline execution. The `PipelineServices` struct bundles two categories of functionality that you can customize:

- **PipelineIO** - Handles user interaction (notifications and prompts)
- **EventHooks** - Responds to pipeline lifecycle events

This section covers how to implement your own services. For using the built-in services, see the [Guide: Services & IO](../../guide/src/services-and-io/index.md).

## Architecture

The services architecture is designed around two key principles:

1. **Multiple services can coexist** - You can register multiple IO services and multiple hook implementations. They all receive events and notifications.
2. **Errors are aggregated, not short-circuited** - When multiple services are registered, all of them are called even if one fails. Errors are collected and returned together.

```
┌─────────────────────────────────────────────────────────────────────┐
│                        PipelineServices                             │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    IO Services (Vec)                         │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │   │
│  │  │ StdoutIO     │  │ WebSocketIO  │  │ ChannelIO    │       │   │
│  │  │ (built-in)   │  │ (custom)     │  │ (custom)     │       │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                   Event Hooks (Vec)                          │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │   │
│  │  │ DebugHooks   │  │ MetricsHooks │  │ AuditHooks   │       │   │
│  │  │ (built-in)   │  │ (custom)     │  │ (custom)     │       │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘       │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## The PipelineServices Struct

`PipelineServices` is the container that holds all registered services:

```rust
use panopticon_core::prelude::*;

// Create empty services
let mut services = PipelineServices::new();

// Or use defaults (includes debug hooks in debug builds)
let mut services = PipelineServices::defaults();
```

### Registering Services

Add your custom services using `add_io()` and `add_hook()`:

```rust
use panopticon_core::prelude::*;

let mut services = PipelineServices::new();

// Register IO services
services.add_io(MyCustomIO::new());
services.add_io(AnotherIO::new());

// Register event hooks
services.add_hook(MetricsHooks::new());
services.add_hook(AuditHooks::new());

// Attach to pipeline
let pipeline = Pipeline::with_services(services);
```

### Default Services by Build Mode

`PipelineServices::defaults()` provides different services depending on compilation mode:

| Build Mode | IO Service | Event Hooks |
|------------|-----------|-------------|
| Debug (`cfg(debug_assertions)`) | `StdoutInteraction` | `DebugEventHooks` |
| Release | None | None |

This gives you verbose logging during development without impacting production performance.

## Service Dispatch Behavior

Understanding how services are dispatched helps you design robust implementations.

### IO Dispatch: Notify vs Prompt

**`notify(message)`** - Broadcasts to all registered IO services:

```rust
// All IO services receive the notification
// Errors are collected, not short-circuited
ctx.services().notify("Processing complete").await?;
```

**`prompt(message)`** - Returns first non-None response:

```rust
// Services are tried in order until one returns Some(response)
if let Some(answer) = ctx.services().prompt("Continue?").await? {
    // First service that returned Some wins
}
```

### Hook Dispatch: Aggregate Errors

All registered hooks are called for every event. If any hook returns an error, errors are collected and returned as a combined error message:

```rust
// Internal dispatch logic (simplified)
let mut errors = Vec::new();
for hook in &self.hooks {
    if let Err(e) = hook.before_execute_command(&event).await {
        errors.push(e);
    }
}
// All hooks ran - errors are combined at the end
```

This ensures that one failing hook does not prevent other hooks from running. For example, a logging hook failure should not prevent a metrics hook from recording data.

## Implementing Custom Services

The following sections cover implementation details for each service type:

- **[PipelineIO](./pipeline-io.md)** - Implement custom notification and prompting
- **[Event Hooks](./event-hooks.md)** - Implement lifecycle callbacks
- **[Hook Events](./hook-events.md)** - Reference for all event types and their fields

## Complete Example: Custom Services

Here is a complete example showing both custom services working together:

```rust
use panopticon_core::prelude::*;
use async_trait::async_trait;

// Custom IO that logs to a file
pub struct FileLoggerIO {
    path: std::path::PathBuf,
}

#[async_trait]
impl PipelineIO for FileLoggerIO {
    async fn notify(&self, message: &str) -> Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "[NOTIFY] {}", message)?;
        Ok(())
    }

    async fn prompt(&self, _message: &str) -> Result<Option<String>> {
        // File logger cannot prompt - return None to let other services handle it
        Ok(None)
    }
}

// Custom hooks that track execution timing
pub struct TimingHooks {
    start_times: std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>,
}

#[async_trait]
impl EventHooks for TimingHooks {
    async fn before_execute_command(&self, event: &hook_events::CommandInfo) -> Result<()> {
        let mut times = self.start_times.lock().unwrap();
        times.insert(event.command_name.clone(), std::time::Instant::now());
        Ok(())
    }

    async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
        let times = self.start_times.lock().unwrap();
        if let Some(start) = times.get(&event.command_name) {
            let elapsed = start.elapsed();
            println!("Command '{}' took {:?}", event.command_name, elapsed);
        }
        Ok(())
    }
}

// Usage
fn main() {
    let mut services = PipelineServices::new();
    services.add_io(FileLoggerIO { path: "pipeline.log".into() });
    services.add_hook(TimingHooks {
        start_times: std::sync::Mutex::new(std::collections::HashMap::new())
    });

    let pipeline = Pipeline::with_services(services);
}
```

## When to Use Services

Services are appropriate for cross-cutting concerns that should not be embedded in command logic:

| Use Case | Service Type | Example |
|----------|-------------|---------|
| Progress reporting | PipelineIO | WebSocket updates to UI |
| User confirmation | PipelineIO | CLI prompts before destructive operations |
| Logging | EventHooks | Structured logging to observability platform |
| Metrics | EventHooks | Execution timing, command counts |
| Auditing | EventHooks | Recording who ran what and when |
| Debugging | EventHooks | Printing pipeline state during development |

For command-specific logic (e.g., "notify when this specific file is processed"), consider handling it within the command itself rather than in a service.
