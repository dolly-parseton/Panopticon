# Hook Events

Hook events are the data structures passed to `EventHooks` methods. Each event type contains information relevant to its lifecycle point. This page documents all event types and their fields.

## Event Types Overview

| Event Type | Used By | Description |
|------------|---------|-------------|
| `PipelineInfo` | `before_compile_pipeline`, `before_execute_pipeline`, `on_results_start` | Basic pipeline statistics |
| `PipelineCompiled` | `after_compile_pipeline` | Pipeline info with compilation timestamp |
| `PipelineExecuted` | `after_execute_pipeline` | Pipeline info with execution timestamp |
| `PipelineCompleted` | `on_results_finish` | Pipeline info with completion timestamp |
| `NamespaceInit` | `after_added_namespace` | Namespace registration details |
| `NamespaceInfo` | `before_execute_namespace` | Namespace execution context |
| `NamespaceExecuted` | `after_execute_namespace` | Namespace completion details |
| `CommandInit` | `after_added_command` | Command registration details |
| `CommandInfo` | `before_execute_command` | Command execution context |
| `CommandExecuted` | `after_execute_command` | Command completion details |

## Pipeline Events

These events provide information about the pipeline as a whole.

### PipelineInfo

Basic pipeline statistics, used at multiple lifecycle points.

```rust
#[derive(Debug)]
pub struct PipelineInfo {
    pub namespace_count: usize,
    pub command_count: usize,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_count` | `usize` | Total number of namespaces in the pipeline |
| `command_count` | `usize` | Total number of commands across all namespaces |

**Used by:**
- `before_compile_pipeline` - Pipeline structure before validation
- `before_execute_pipeline` - Pipeline structure at execution start
- `on_results_start` - Pipeline structure when collecting results

**Example usage:**

```rust
async fn before_execute_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> {
    println!(
        "Starting pipeline with {} namespaces and {} commands",
        event.namespace_count,
        event.command_count
    );
    Ok(())
}
```

### PipelineCompiled

Extends `PipelineInfo` with compilation timing.

```rust
#[derive(Debug)]
pub struct PipelineCompiled {
    pub namespace_count: usize,
    pub command_count: usize,
    pub compiled_at: Instant,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_count` | `usize` | Total number of namespaces |
| `command_count` | `usize` | Total number of commands |
| `compiled_at` | `Instant` | When compilation completed |

**Used by:** `after_compile_pipeline`

**Example usage:**

```rust
async fn after_compile_pipeline(&self, event: &hook_events::PipelineCompiled) -> Result<()> {
    println!(
        "Pipeline compiled at {:?} with {} commands ready for execution",
        event.compiled_at,
        event.command_count
    );
    Ok(())
}
```

### PipelineExecuted

Extends `PipelineInfo` with execution timing.

```rust
#[derive(Debug)]
pub struct PipelineExecuted {
    pub namespace_count: usize,
    pub command_count: usize,
    pub executed_at: Instant,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_count` | `usize` | Total number of namespaces executed |
| `command_count` | `usize` | Total number of commands executed |
| `executed_at` | `Instant` | When execution completed |

**Used by:** `after_execute_pipeline`

**Example usage:**

```rust
async fn after_execute_pipeline(&self, event: &hook_events::PipelineExecuted) -> Result<()> {
    let duration = event.executed_at.elapsed();
    println!(
        "Pipeline execution finished. {} commands completed in {:?}",
        event.command_count,
        duration
    );
    Ok(())
}
```

### PipelineCompleted

Extends `PipelineInfo` with completion timing, marking the end of the pipeline lifecycle.

```rust
#[derive(Debug)]
pub struct PipelineCompleted {
    pub namespace_count: usize,
    pub command_count: usize,
    pub completed_at: Instant,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_count` | `usize` | Total number of namespaces |
| `command_count` | `usize` | Total number of commands |
| `completed_at` | `Instant` | When results collection completed |

**Used by:** `on_results_finish`

**Example usage:**

```rust
async fn on_results_finish(&self, event: &hook_events::PipelineCompleted) -> Result<()> {
    println!(
        "Pipeline fully completed at {:?}. All {} commands processed.",
        event.completed_at,
        event.command_count
    );
    Ok(())
}
```

## Namespace Events

These events provide information about individual namespaces.

### NamespaceInit

Information about a namespace when it is added to the pipeline (Draft phase).

```rust
#[derive(Debug)]
pub struct NamespaceInit {
    pub namespace_index: usize,
    pub namespace_name: String,
    pub namespace_type: String,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_index` | `usize` | Position of this namespace (0-indexed) |
| `namespace_name` | `String` | Name given to the namespace |
| `namespace_type` | `String` | Type of namespace ("Once", "Iterative", "Static") |

**Used by:** `after_added_namespace`

**Example usage:**

```rust
async fn after_added_namespace(&self, event: &hook_events::NamespaceInit) -> Result<()> {
    println!(
        "Namespace #{} '{}' added (type: {})",
        event.namespace_index,
        event.namespace_name,
        event.namespace_type
    );

    // Validate namespace naming conventions
    if !event.namespace_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(anyhow::anyhow!(
            "Namespace name '{}' contains invalid characters",
            event.namespace_name
        ));
    }

    Ok(())
}
```

### NamespaceInfo

Information about a namespace just before it executes.

```rust
#[derive(Debug)]
pub struct NamespaceInfo {
    pub namespace_index: usize,
    pub namespace_name: String,
    pub command_count: usize,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_index` | `usize` | Position of this namespace (0-indexed) |
| `namespace_name` | `String` | Name of the namespace |
| `command_count` | `usize` | Number of commands in this namespace |

**Used by:** `before_execute_namespace`

**Example usage:**

```rust
async fn before_execute_namespace(&self, event: &hook_events::NamespaceInfo) -> Result<()> {
    println!(
        "Executing namespace '{}' ({} commands)",
        event.namespace_name,
        event.command_count
    );
    Ok(())
}
```

### NamespaceExecuted

Information about a namespace after it finishes executing.

```rust
#[derive(Debug)]
pub struct NamespaceExecuted {
    pub namespace_index: usize,
    pub namespace_name: String,
    pub executed_at: Instant,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_index` | `usize` | Position of this namespace (0-indexed) |
| `namespace_name` | `String` | Name of the namespace |
| `executed_at` | `Instant` | When namespace execution completed |

**Used by:** `after_execute_namespace`

**Example usage:**

```rust
async fn after_execute_namespace(&self, event: &hook_events::NamespaceExecuted) -> Result<()> {
    println!(
        "Namespace '{}' (index {}) completed execution",
        event.namespace_name,
        event.namespace_index
    );
    Ok(())
}
```

## Command Events

These events provide information about individual commands.

### CommandInit

Information about a command when it is added to a namespace (Draft phase).

```rust
#[derive(Debug)]
pub struct CommandInit {
    pub namespace_index: usize,
    pub command_name: String,
    pub command_type: String,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_index` | `usize` | Index of the parent namespace |
| `command_name` | `String` | Name given to this command |
| `command_type` | `String` | Type of command (e.g., "FileCommand", "SqlCommand") |

**Used by:** `after_added_command`

**Example usage:**

```rust
async fn after_added_command(&self, event: &hook_events::CommandInit) -> Result<()> {
    println!(
        "Command '{}' (type: {}) added to namespace #{}",
        event.command_name,
        event.command_type,
        event.namespace_index
    );

    // Track command types for metrics
    self.command_types
        .lock()
        .unwrap()
        .entry(event.command_type.clone())
        .and_modify(|c| *c += 1)
        .or_insert(1);

    Ok(())
}
```

### CommandInfo

Information about a command just before it executes.

```rust
#[derive(Debug)]
pub struct CommandInfo {
    pub namespace_index: usize,
    pub command_name: String,
    pub command_type: String,
    pub command_count: usize,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_index` | `usize` | Index of the parent namespace |
| `command_name` | `String` | Name of this command |
| `command_type` | `String` | Type of command |
| `command_count` | `usize` | Total commands in the parent namespace |

**Used by:** `before_execute_command`

**Example usage:**

```rust
async fn before_execute_command(&self, event: &hook_events::CommandInfo) -> Result<()> {
    println!(
        "Executing '{}' ({}) - command in namespace #{}",
        event.command_name,
        event.command_type,
        event.namespace_index
    );

    // Start timing this command
    self.start_times
        .lock()
        .unwrap()
        .insert(event.command_name.clone(), std::time::Instant::now());

    Ok(())
}
```

### CommandExecuted

Information about a command after it finishes executing.

```rust
#[derive(Debug)]
pub struct CommandExecuted {
    pub namespace_index: usize,
    pub command_name: String,
    pub command_type: String,
    pub executed_at: Instant,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `namespace_index` | `usize` | Index of the parent namespace |
| `command_name` | `String` | Name of this command |
| `command_type` | `String` | Type of command |
| `executed_at` | `Instant` | When command execution completed |

**Used by:** `after_execute_command`

**Example usage:**

```rust
async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
    // Calculate duration if we tracked the start time
    if let Some(start) = self.start_times.lock().unwrap().get(&event.command_name) {
        let duration = start.elapsed();
        println!(
            "Command '{}' completed in {:?}",
            event.command_name,
            duration
        );

        // Record metrics
        self.metrics
            .lock()
            .unwrap()
            .command_durations
            .insert(event.command_name.clone(), duration);
    }

    Ok(())
}
```

## Working with Instant

Several events include an `Instant` field for timing. Here are common patterns:

### Calculating Duration

```rust
use std::time::Instant;

// Store the instant when you receive it
let start = event.compiled_at;

// Later, calculate elapsed time
let duration = start.elapsed();
println!("Time since compilation: {:?}", duration);
```

### Comparing Instants

```rust
// Store instants from before/after hooks
let compile_time = self.compile_instant.lock().unwrap();
let execute_time = event.executed_at;

// Calculate time between events
if let Some(compile) = *compile_time {
    let time_to_execute = execute_time.duration_since(compile);
    println!("Time from compile to execute: {:?}", time_to_execute);
}
```

### Metrics Collection Pattern

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct TimingMetrics {
    compile_start: Mutex<Option<Instant>>,
    execute_start: Mutex<Option<Instant>>,
    namespace_starts: Mutex<HashMap<usize, Instant>>,
    command_starts: Mutex<HashMap<String, Instant>>,

    // Collected metrics
    pub compile_duration: Mutex<Option<Duration>>,
    pub execute_duration: Mutex<Option<Duration>>,
    pub namespace_durations: Mutex<HashMap<String, Duration>>,
    pub command_durations: Mutex<HashMap<String, Duration>>,
}
```

## Complete Event Flow Example

Here is how events flow through a typical pipeline execution:

```rust
// Pipeline construction (Draft phase)
pipeline.add_namespace("data", NamespaceType::Once);
// → after_added_namespace(NamespaceInit { index: 0, name: "data", type: "Once" })

pipeline.add_command("data", "load", FileCommand::new());
// → after_added_command(CommandInit { namespace_index: 0, name: "load", type: "FileCommand" })

// Compilation
pipeline.compile();
// → before_compile_pipeline(PipelineInfo { namespace_count: 1, command_count: 1 })
// → after_compile_pipeline(PipelineCompiled { namespace_count: 1, command_count: 1, compiled_at: ... })

// Execution (Ready phase)
pipeline.execute().await;
// → before_execute_pipeline(PipelineInfo { namespace_count: 1, command_count: 1 })
//   → before_execute_namespace(NamespaceInfo { index: 0, name: "data", command_count: 1 })
//     → before_execute_command(CommandInfo { namespace_index: 0, name: "load", type: "FileCommand", count: 1 })
//     → after_execute_command(CommandExecuted { namespace_index: 0, name: "load", type: "FileCommand", executed_at: ... })
//   → after_execute_namespace(NamespaceExecuted { index: 0, name: "data", executed_at: ... })
// → after_execute_pipeline(PipelineExecuted { namespace_count: 1, command_count: 1, executed_at: ... })

// Results collection (Completed phase)
pipeline.results();
// → on_results_start(PipelineInfo { namespace_count: 1, command_count: 1 })
// → on_results_finish(PipelineCompleted { namespace_count: 1, command_count: 1, completed_at: ... })
```

## Importing Event Types

Event types are available through the `hook_events` module:

```rust
use panopticon_core::services::hook_events::{
    PipelineInfo,
    PipelineCompiled,
    PipelineExecuted,
    PipelineCompleted,
    NamespaceInit,
    NamespaceInfo,
    NamespaceExecuted,
    CommandInit,
    CommandInfo,
    CommandExecuted,
};
```

Or import the module and use qualified names:

```rust
use panopticon_core::services::hook_events;

async fn before_execute_command(&self, event: &hook_events::CommandInfo) -> Result<()> {
    // ...
}
```
