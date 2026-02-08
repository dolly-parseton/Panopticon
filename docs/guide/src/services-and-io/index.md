# Services & IO

Panopticon pipelines can interact with the outside world through **services**. The `PipelineServices` struct bundles two categories of functionality:

- **PipelineIO** - Send notifications and prompt for user input
- **EventHooks** - React to pipeline lifecycle events (compilation, execution, completion)

Services are optional. A pipeline without services runs silently, which is often what you want for batch processing or automated workflows. When you need feedback or interactivity, services provide a clean abstraction.

## Attaching Services to a Pipeline

Use `Pipeline::with_services()` to attach a `PipelineServices` instance:

```rust
use panopticon_core::prelude::*;

let services = PipelineServices::defaults();
let mut pipeline = Pipeline::with_services(services);
```

## Using PipelineIO

The `PipelineIO` trait provides two methods for interacting with users:

- `notify(message)` - Display a message (fire-and-forget)
- `prompt(message)` - Display a message and wait for a response

Commands access these through the `ExecutionContext`:

```rust
// Inside a command's execute method
ctx.services().notify("Processing started...").await?;

if let Some(answer) = ctx.services().prompt("Continue? (y/n)").await? {
    if answer.to_lowercase() == "y" {
        // proceed
    }
}
```

Multiple IO services can be registered. When you call `notify`, all registered services receive the message. When you call `prompt`, services are tried in order until one returns a response.

## Event Hooks

Event hooks let you observe pipeline lifecycle events without modifying command logic. Hooks fire at key moments:

- **Draft phase**: After namespaces/commands are added, before/after compilation
- **Ready phase**: Before/after pipeline, namespace, and command execution
- **Completed phase**: When results are being collected

This is useful for logging, metrics, progress reporting, or custom debugging.

## Default Services

`PipelineServices::defaults()` behaves differently based on build mode:

| Build Mode | IO Service | Event Hooks |
|------------|-----------|-------------|
| Debug (`cfg(debug_assertions)`) | `StdoutInteraction` | `DebugEventHooks` |
| Release | None | None |

In debug builds, you get console output and lifecycle logging out of the box. In release builds, services start empty for maximum control.

To start with no services regardless of build mode:

```rust
let services = PipelineServices::new();
```

## Implementing Custom Services

This guide covers *using* services. For implementing your own `PipelineIO` or `EventHooks`:

**[See the Extend guide: Implementing Services](../../extending/src/services/index.md)**

The Extend guide covers:
- Implementing the `PipelineIO` trait for custom notification channels
- Implementing `EventHooks` for lifecycle callbacks
- Available hook events and their payloads
- Registering multiple services with `add_io()` and `add_hook()`
