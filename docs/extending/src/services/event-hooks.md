# Event Hooks

The `EventHooks` trait lets you observe and react to pipeline lifecycle events without modifying command logic. Hooks are ideal for cross-cutting concerns like logging, metrics, auditing, and debugging.

## The EventHooks Trait

```rust
#[async_trait]
pub trait EventHooks: Send + Sync {
    // Draft phase
    async fn after_added_namespace(&self, event: &hook_events::NamespaceInit) -> Result<()> { Ok(()) }
    async fn after_added_command(&self, event: &hook_events::CommandInit) -> Result<()> { Ok(()) }
    async fn before_compile_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> { Ok(()) }
    async fn after_compile_pipeline(&self, event: &hook_events::PipelineCompiled) -> Result<()> { Ok(()) }

    // Ready phase
    async fn before_execute_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> { Ok(()) }
    async fn after_execute_pipeline(&self, event: &hook_events::PipelineExecuted) -> Result<()> { Ok(()) }
    async fn before_execute_namespace(&self, event: &hook_events::NamespaceInfo) -> Result<()> { Ok(()) }
    async fn after_execute_namespace(&self, event: &hook_events::NamespaceExecuted) -> Result<()> { Ok(()) }
    async fn before_execute_command(&self, event: &hook_events::CommandInfo) -> Result<()> { Ok(()) }
    async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> { Ok(()) }

    // Completed phase
    async fn on_results_start(&self, event: &hook_events::PipelineInfo) -> Result<()> { Ok(()) }
    async fn on_results_finish(&self, event: &hook_events::PipelineCompleted) -> Result<()> { Ok(()) }
}
```

All methods have default no-op implementations. Only override the hooks you care about.

### Trait Bounds

- **`Send + Sync`** - Hooks must be thread-safe for concurrent execution
- **`async`** - All hooks are async for consistency and to support async operations (logging to remote services, etc.)

## Hook Reference Table

| Hook | Phase | Event Type | Fires When |
|------|-------|-----------|------------|
| `after_added_namespace` | Draft | `NamespaceInit` | A namespace is added to the pipeline |
| `after_added_command` | Draft | `CommandInit` | A command is added to a namespace |
| `before_compile_pipeline` | Draft | `PipelineInfo` | Just before `compile()` validates the pipeline |
| `after_compile_pipeline` | Draft | `PipelineCompiled` | After successful compilation, pipeline is now Ready |
| `before_execute_pipeline` | Ready | `PipelineInfo` | Just before `execute()` starts |
| `after_execute_pipeline` | Ready | `PipelineExecuted` | After all namespaces have executed |
| `before_execute_namespace` | Ready | `NamespaceInfo` | Just before a namespace starts executing |
| `after_execute_namespace` | Ready | `NamespaceExecuted` | After a namespace finishes executing |
| `before_execute_command` | Ready | `CommandInfo` | Just before a command executes |
| `after_execute_command` | Ready | `CommandExecuted` | After a command finishes executing |
| `on_results_start` | Completed | `PipelineInfo` | When `results()` is called |
| `on_results_finish` | Completed | `PipelineCompleted` | After results collection completes |

## Pipeline Phases

Hooks are organized by the pipeline state machine phase in which they fire.

### Draft Phase

The Draft phase covers pipeline construction and compilation.

```
Pipeline::new()
    │
    ├─► add_namespace() ──► after_added_namespace
    │       │
    │       └─► add_command() ──► after_added_command
    │
    └─► compile()
            │
            ├─► before_compile_pipeline
            │
            └─► after_compile_pipeline
                    │
                    ▼
              Pipeline<Ready>
```

**`after_added_namespace`** - Fires each time a namespace is registered. Useful for:
- Validating namespace names against a whitelist
- Recording namespace registration for debugging
- Initializing per-namespace state in your hooks

**`after_added_command`** - Fires each time a command is added. Useful for:
- Logging command configuration
- Validating command types are allowed
- Building command inventories

**`before_compile_pipeline`** - Fires before validation runs. Useful for:
- Pre-compilation logging
- Injecting additional validation
- Recording pipeline structure before any transformation

**`after_compile_pipeline`** - Fires after successful compilation. Useful for:
- Logging the compiled pipeline structure
- Sending "pipeline ready" notifications
- Initializing execution-phase state

### Ready Phase

The Ready phase covers pipeline execution.

```
Pipeline<Ready>::execute()
    │
    ├─► before_execute_pipeline
    │
    ├─► for each namespace:
    │       │
    │       ├─► before_execute_namespace
    │       │
    │       ├─► for each command:
    │       │       │
    │       │       ├─► before_execute_command
    │       │       │
    │       │       └─► after_execute_command
    │       │
    │       └─► after_execute_namespace
    │
    └─► after_execute_pipeline
            │
            ▼
      Pipeline<Completed>
```

**`before_execute_pipeline`** - Fires once when execution starts. Useful for:
- Starting execution timers
- Logging "pipeline started"
- Acquiring resources needed during execution

**`after_execute_pipeline`** - Fires once when all commands complete. Useful for:
- Stopping execution timers
- Logging "pipeline finished"
- Releasing execution resources

**`before_execute_namespace`** / **`after_execute_namespace`** - Fire for each namespace. Useful for:
- Per-namespace timing
- Progress reporting ("Processing namespace 2 of 5")
- Namespace-level metrics

**`before_execute_command`** / **`after_execute_command`** - Fire for each command. Useful for:
- Per-command timing
- Detailed execution logging
- Command-level metrics

### Completed Phase

The Completed phase covers results collection.

```
Pipeline<Completed>::results()
    │
    ├─► on_results_start
    │
    └─► on_results_finish
            │
            ▼
        Results returned
```

**`on_results_start`** - Fires when results collection begins. Useful for:
- Logging "collecting results"
- Preparing result processors

**`on_results_finish`** - Fires when results collection completes. Useful for:
- Final pipeline metrics
- Cleanup operations
- Sending "pipeline complete" notifications

## Built-in Implementation: DebugEventHooks

Panopticon includes `DebugEventHooks` for development:

```rust
pub struct DebugEventHooks;

#[async_trait]
impl EventHooks for DebugEventHooks {
    async fn after_added_namespace(&self, event: &hook_events::NamespaceInit) -> Result<()> {
        println!("DebugEventHooks - after_added_namespace: {:?}", event);
        Ok(())
    }

    async fn after_added_command(&self, event: &hook_events::CommandInit) -> Result<()> {
        println!("DebugEventHooks - after_added_command: {:?}", event);
        Ok(())
    }

    async fn before_compile_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        println!("DebugEventHooks - before_compile_pipeline: {:?}", event);
        Ok(())
    }

    async fn after_compile_pipeline(&self, event: &hook_events::PipelineCompiled) -> Result<()> {
        println!("DebugEventHooks - after_compile_pipeline: {:?}", event);
        Ok(())
    }

    async fn before_execute_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        println!("DebugEventHooks - before_execute_pipeline: {:?}", event);
        Ok(())
    }

    async fn after_execute_pipeline(&self, event: &hook_events::PipelineExecuted) -> Result<()> {
        println!("DebugEventHooks - after_execute_pipeline: {:?}", event);
        Ok(())
    }

    async fn before_execute_namespace(&self, event: &hook_events::NamespaceInfo) -> Result<()> {
        println!("DebugEventHooks - before_execute_namespace: {:?}", event);
        Ok(())
    }

    async fn after_execute_namespace(&self, event: &hook_events::NamespaceExecuted) -> Result<()> {
        println!("DebugEventHooks - after_execute_namespace: {:?}", event);
        Ok(())
    }

    async fn before_execute_command(&self, event: &hook_events::CommandInfo) -> Result<()> {
        println!("DebugEventHooks - before_execute_command: {:?}", event);
        Ok(())
    }

    async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
        println!("DebugEventHooks - after_execute_command: {:?}", event);
        Ok(())
    }

    async fn on_results_start(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        println!("DebugEventHooks - on_results_start: {:?}", event);
        Ok(())
    }

    async fn on_results_finish(&self, event: &hook_events::PipelineCompleted) -> Result<()> {
        println!("DebugEventHooks - on_results_finish: {:?}", event);
        Ok(())
    }
}
```

This is included automatically in debug builds via `PipelineServices::defaults()`.

## Example: Metrics Hooks

Collect execution timing metrics:

```rust
use panopticon_core::prelude::*;
use async_trait::async_trait;
use std::sync::Mutex;
use std::time::Instant;
use std::collections::HashMap;

pub struct MetricsHooks {
    pipeline_start: Mutex<Option<Instant>>,
    namespace_starts: Mutex<HashMap<usize, Instant>>,
    command_starts: Mutex<HashMap<String, Instant>>,
    metrics: Mutex<Metrics>,
}

#[derive(Default)]
pub struct Metrics {
    pub pipeline_duration_ms: u64,
    pub namespace_durations_ms: HashMap<String, u64>,
    pub command_durations_ms: HashMap<String, u64>,
    pub total_commands: usize,
}

impl MetricsHooks {
    pub fn new() -> Self {
        Self {
            pipeline_start: Mutex::new(None),
            namespace_starts: Mutex::new(HashMap::new()),
            command_starts: Mutex::new(HashMap::new()),
            metrics: Mutex::new(Metrics::default()),
        }
    }

    pub fn get_metrics(&self) -> Metrics {
        self.metrics.lock().unwrap().clone()
    }
}

#[async_trait]
impl EventHooks for MetricsHooks {
    async fn before_execute_pipeline(&self, _event: &hook_events::PipelineInfo) -> Result<()> {
        *self.pipeline_start.lock().unwrap() = Some(Instant::now());
        Ok(())
    }

    async fn after_execute_pipeline(&self, _event: &hook_events::PipelineExecuted) -> Result<()> {
        if let Some(start) = *self.pipeline_start.lock().unwrap() {
            self.metrics.lock().unwrap().pipeline_duration_ms =
                start.elapsed().as_millis() as u64;
        }
        Ok(())
    }

    async fn before_execute_namespace(&self, event: &hook_events::NamespaceInfo) -> Result<()> {
        self.namespace_starts
            .lock()
            .unwrap()
            .insert(event.namespace_index, Instant::now());
        Ok(())
    }

    async fn after_execute_namespace(&self, event: &hook_events::NamespaceExecuted) -> Result<()> {
        if let Some(start) = self.namespace_starts.lock().unwrap().get(&event.namespace_index) {
            self.metrics
                .lock()
                .unwrap()
                .namespace_durations_ms
                .insert(event.namespace_name.clone(), start.elapsed().as_millis() as u64);
        }
        Ok(())
    }

    async fn before_execute_command(&self, event: &hook_events::CommandInfo) -> Result<()> {
        self.command_starts
            .lock()
            .unwrap()
            .insert(event.command_name.clone(), Instant::now());
        Ok(())
    }

    async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.total_commands += 1;

        if let Some(start) = self.command_starts.lock().unwrap().get(&event.command_name) {
            metrics
                .command_durations_ms
                .insert(event.command_name.clone(), start.elapsed().as_millis() as u64);
        }
        Ok(())
    }
}
```

## Example: Progress Hooks

Report execution progress to a UI:

```rust
use panopticon_core::prelude::*;
use async_trait::async_trait;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    PipelineStarted { total_namespaces: usize, total_commands: usize },
    NamespaceStarted { name: String, index: usize, command_count: usize },
    NamespaceFinished { name: String, index: usize },
    CommandStarted { name: String, command_type: String },
    CommandFinished { name: String },
    PipelineFinished,
}

pub struct ProgressHooks {
    sender: mpsc::Sender<ProgressEvent>,
}

impl ProgressHooks {
    pub fn new(sender: mpsc::Sender<ProgressEvent>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl EventHooks for ProgressHooks {
    async fn before_execute_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        let _ = self.sender.send(ProgressEvent::PipelineStarted {
            total_namespaces: event.namespace_count,
            total_commands: event.command_count,
        }).await;
        Ok(())
    }

    async fn after_execute_pipeline(&self, _event: &hook_events::PipelineExecuted) -> Result<()> {
        let _ = self.sender.send(ProgressEvent::PipelineFinished).await;
        Ok(())
    }

    async fn before_execute_namespace(&self, event: &hook_events::NamespaceInfo) -> Result<()> {
        let _ = self.sender.send(ProgressEvent::NamespaceStarted {
            name: event.namespace_name.clone(),
            index: event.namespace_index,
            command_count: event.command_count,
        }).await;
        Ok(())
    }

    async fn after_execute_namespace(&self, event: &hook_events::NamespaceExecuted) -> Result<()> {
        let _ = self.sender.send(ProgressEvent::NamespaceFinished {
            name: event.namespace_name.clone(),
            index: event.namespace_index,
        }).await;
        Ok(())
    }

    async fn before_execute_command(&self, event: &hook_events::CommandInfo) -> Result<()> {
        let _ = self.sender.send(ProgressEvent::CommandStarted {
            name: event.command_name.clone(),
            command_type: event.command_type.clone(),
        }).await;
        Ok(())
    }

    async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
        let _ = self.sender.send(ProgressEvent::CommandFinished {
            name: event.command_name.clone(),
        }).await;
        Ok(())
    }
}
```

## Example: Audit Hooks

Record execution history for compliance:

```rust
use panopticon_core::prelude::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub struct AuditRecord {
    timestamp: DateTime<Utc>,
    event_type: String,
    details: serde_json::Value,
}

pub struct AuditHooks {
    records: std::sync::Mutex<Vec<AuditRecord>>,
}

impl AuditHooks {
    pub fn new() -> Self {
        Self {
            records: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn record(&self, event_type: &str, details: serde_json::Value) {
        self.records.lock().unwrap().push(AuditRecord {
            timestamp: Utc::now(),
            event_type: event_type.to_string(),
            details,
        });
    }

    pub fn export(&self) -> Vec<AuditRecord> {
        self.records.lock().unwrap().clone()
    }
}

#[async_trait]
impl EventHooks for AuditHooks {
    async fn before_execute_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        self.record("pipeline_started", serde_json::json!({
            "namespace_count": event.namespace_count,
            "command_count": event.command_count,
        }));
        Ok(())
    }

    async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
        self.record("command_executed", serde_json::json!({
            "command_name": event.command_name,
            "command_type": event.command_type,
            "namespace_index": event.namespace_index,
        }));
        Ok(())
    }

    async fn on_results_finish(&self, event: &hook_events::PipelineCompleted) -> Result<()> {
        self.record("pipeline_completed", serde_json::json!({
            "namespace_count": event.namespace_count,
            "command_count": event.command_count,
        }));
        Ok(())
    }
}
```

## Error Handling

Hook errors are aggregated, not short-circuited. All registered hooks run even if one fails:

```rust
// If you have two hooks registered and both fail:
// Hook 1 returns: Err(anyhow!("Database connection failed"))
// Hook 2 returns: Err(anyhow!("Network timeout"))
//
// The combined error is:
// Err(anyhow!("Hook service errors: Database connection failed; Network timeout"))
```

This ensures that one misbehaving hook does not prevent other hooks from executing. However, if any hook fails, the combined error propagates to the caller.

### Recommended Error Handling Patterns

**Non-critical hooks (logging, metrics):** Handle errors internally:

```rust
async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
    if let Err(e) = self.try_log_metrics(event).await {
        eprintln!("Warning: metrics logging failed: {}", e);
    }
    Ok(()) // Don't propagate - metrics failure shouldn't stop execution
}
```

**Critical hooks (audit, compliance):** Propagate errors:

```rust
async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
    self.write_audit_log(event)
        .await
        .context("Audit logging is required - cannot continue without it")?;
    Ok(())
}
```

## Registering Hooks

```rust
use panopticon_core::prelude::*;

let mut services = PipelineServices::new();

// Register multiple hooks - they all receive events
services.add_hook(MetricsHooks::new());
services.add_hook(AuditHooks::new());
services.add_hook(ProgressHooks::new(progress_sender));

let pipeline = Pipeline::with_services(services);
```

## Testing Hooks

Create a mock hook that records all events:

```rust
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct MockHooks {
    events: Arc<Mutex<Vec<String>>>,
}

impl MockHooks {
    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl EventHooks for MockHooks {
    async fn before_execute_pipeline(&self, _: &hook_events::PipelineInfo) -> Result<()> {
        self.events.lock().unwrap().push("before_execute_pipeline".into());
        Ok(())
    }

    async fn after_execute_pipeline(&self, _: &hook_events::PipelineExecuted) -> Result<()> {
        self.events.lock().unwrap().push("after_execute_pipeline".into());
        Ok(())
    }

    // ... implement other hooks similarly
}

#[tokio::test]
async fn test_hook_order() {
    let hooks = MockHooks::default();
    let mut services = PipelineServices::new();
    services.add_hook(hooks.clone());

    let mut pipeline = Pipeline::with_services(services);
    // ... add namespaces and commands
    let pipeline = pipeline.compile().unwrap();
    let pipeline = pipeline.execute().await.unwrap();

    let events = hooks.events();
    assert!(events.contains(&"before_execute_pipeline".to_string()));
    assert!(events.contains(&"after_execute_pipeline".to_string()));

    // Verify order
    let before_idx = events.iter().position(|e| e == "before_execute_pipeline").unwrap();
    let after_idx = events.iter().position(|e| e == "after_execute_pipeline").unwrap();
    assert!(before_idx < after_idx);
}
```
