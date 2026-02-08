# Implementing PipelineIO

The `PipelineIO` trait defines how pipelines communicate with the outside world for user interaction. Implement this trait to create custom notification channels, prompting mechanisms, or integration with external systems.

## The PipelineIO Trait

```rust
#[async_trait]
pub trait PipelineIO: Send + Sync {
    async fn notify(&self, message: &str) -> Result<()> {
        Ok(())
    }

    async fn prompt(&self, message: &str) -> Result<Option<String>> {
        Ok(None)
    }
}
```

Both methods have default implementations that do nothing. This means you only need to implement the methods relevant to your use case.

### Trait Bounds

- **`Send + Sync`** - IO services must be thread-safe because pipelines may execute across multiple threads
- **`async`** - Both methods are async to support network-based IO (WebSocket, HTTP, etc.)

## Method Reference

### `notify(message: &str) -> Result<()>`

Sends a one-way notification. The caller does not expect a response.

**Behavior:**
- Called on all registered IO services
- Errors from all services are aggregated
- Fire-and-forget from the command's perspective

**Common implementations:**
- Print to stdout/stderr
- Write to a log file
- Send via WebSocket to a UI
- Post to a message queue

### `prompt(message: &str) -> Result<Option<String>>`

Requests input from the user. Returns `Some(response)` if the service can provide an answer, or `None` to defer to other services.

**Behavior:**
- Services are tried in order of registration
- First `Some(response)` is returned to the caller
- If all services return `None`, the prompt returns `None`

**Return values:**
- `Ok(Some(string))` - This service handled the prompt
- `Ok(None)` - This service cannot handle prompts (defer to others)
- `Err(...)` - Something went wrong

## Built-in Implementation: StdoutInteraction

Panopticon includes `StdoutInteraction` for CLI applications:

```rust
pub struct StdoutInteraction;

#[async_trait]
impl PipelineIO for StdoutInteraction {
    async fn notify(&self, message: &str) -> Result<()> {
        println!("{message}");
        Ok(())
    }

    async fn prompt(&self, message: &str) -> Result<Option<String>> {
        println!("{message}");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(match input.trim() {
            "" => None,
            _ => Some(input.trim().to_string()),
        })
    }
}
```

This implementation:
- Prints notifications to stdout
- Displays prompt messages and reads from stdin
- Returns `None` for empty input (just pressing Enter)

## Example: WebSocket IO

For web applications, you might send notifications over WebSocket:

```rust
use panopticon_core::prelude::*;
use async_trait::async_trait;
use tokio::sync::mpsc;

pub struct WebSocketIO {
    sender: mpsc::Sender<String>,
}

impl WebSocketIO {
    pub fn new(sender: mpsc::Sender<String>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl PipelineIO for WebSocketIO {
    async fn notify(&self, message: &str) -> Result<()> {
        self.sender
            .send(format!(r#"{{"type":"notify","message":"{}"}}"#, message))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send notification: {}", e))?;
        Ok(())
    }

    async fn prompt(&self, _message: &str) -> Result<Option<String>> {
        // WebSocket IO is one-way in this example
        // For bidirectional prompts, you would need request/response channels
        Ok(None)
    }
}
```

## Example: Channel-based IO

For embedding pipelines in larger applications, channels provide clean integration:

```rust
use panopticon_core::prelude::*;
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

pub enum IOMessage {
    Notify(String),
    Prompt {
        message: String,
        response: oneshot::Sender<Option<String>>,
    },
}

pub struct ChannelIO {
    sender: mpsc::Sender<IOMessage>,
}

impl ChannelIO {
    pub fn new(sender: mpsc::Sender<IOMessage>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl PipelineIO for ChannelIO {
    async fn notify(&self, message: &str) -> Result<()> {
        self.sender
            .send(IOMessage::Notify(message.to_string()))
            .await
            .map_err(|e| anyhow::anyhow!("Channel send failed: {}", e))?;
        Ok(())
    }

    async fn prompt(&self, message: &str) -> Result<Option<String>> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(IOMessage::Prompt {
                message: message.to_string(),
                response: tx,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Channel send failed: {}", e))?;

        rx.await.map_err(|e| anyhow::anyhow!("Response channel closed: {}", e))
    }
}
```

Usage in your application:

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<IOMessage>(100);

    // Spawn handler for IO messages
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                IOMessage::Notify(message) => {
                    // Update your UI
                    println!("Pipeline says: {}", message);
                }
                IOMessage::Prompt { message, response } => {
                    // Show dialog, get user input
                    let answer = show_dialog(&message).await;
                    let _ = response.send(answer);
                }
            }
        }
    });

    // Create pipeline with channel IO
    let mut services = PipelineServices::new();
    services.add_io(ChannelIO::new(tx));

    let pipeline = Pipeline::with_services(services);
    // ... use pipeline
}
```

## Example: Quiet IO (No-op)

For batch processing where you want to suppress all output:

```rust
use panopticon_core::prelude::*;
use async_trait::async_trait;

pub struct QuietIO;

#[async_trait]
impl PipelineIO for QuietIO {
    // Use default implementations - both return Ok with no action
}
```

Or equivalently, just do not register any IO services:

```rust
let services = PipelineServices::new(); // No IO services
```

## Example: Multi-destination IO

Combine multiple outputs in a single service:

```rust
use panopticon_core::prelude::*;
use async_trait::async_trait;
use std::io::Write;

pub struct MultiIO {
    log_path: std::path::PathBuf,
    verbose: bool,
}

#[async_trait]
impl PipelineIO for MultiIO {
    async fn notify(&self, message: &str) -> Result<()> {
        // Always log to file
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        writeln!(file, "[{}] {}", chrono::Utc::now(), message)?;

        // Optionally print to console
        if self.verbose {
            println!("{}", message);
        }

        Ok(())
    }

    async fn prompt(&self, message: &str) -> Result<Option<String>> {
        // Log the prompt
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        writeln!(file, "[{}] PROMPT: {}", chrono::Utc::now(), message)?;

        // Read from stdin
        println!("{}", message);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        let response = input.trim();
        writeln!(file, "[{}] RESPONSE: {}", chrono::Utc::now(), response)?;

        Ok(if response.is_empty() { None } else { Some(response.to_string()) })
    }
}
```

## Error Handling

IO errors are aggregated when multiple services are registered. If your `notify` returns an error, it will be combined with errors from other IO services:

```rust
// If StdoutIO succeeds but FileIO fails:
// Err(anyhow!("IO service errors: Failed to write to file: permission denied"))

// If both fail:
// Err(anyhow!("IO service errors: stdout closed; Failed to write to file: permission denied"))
```

This aggregation ensures all services attempt their operation before failing.

### Recommended Error Handling Patterns

**Non-critical IO (logging):** Consider catching errors internally:

```rust
async fn notify(&self, message: &str) -> Result<()> {
    if let Err(e) = self.try_log(message).await {
        eprintln!("Warning: logging failed: {}", e);
    }
    Ok(()) // Don't propagate - logging failure shouldn't stop the pipeline
}
```

**Critical IO (audit logs):** Propagate errors:

```rust
async fn notify(&self, message: &str) -> Result<()> {
    self.audit_log(message)
        .await
        .context("Audit logging is required for compliance")?;
    Ok(())
}
```

## Registering Your IO Service

```rust
use panopticon_core::prelude::*;

let mut services = PipelineServices::new();

// Add your custom IO
services.add_io(MyCustomIO::new());

// You can add multiple IO services
services.add_io(StdoutInteraction);  // Also print to console
services.add_io(FileLoggerIO::new("pipeline.log"));

let pipeline = Pipeline::with_services(services);
```

## Accessing IO from Commands

Inside a command's `execute` method:

```rust
impl Command for MyCommand {
    async fn execute(&self, ctx: &mut ExecutionContext) -> Result<()> {
        // Send notification
        ctx.services().notify("Starting MyCommand...").await?;

        // Prompt for confirmation
        if let Some(answer) = ctx.services().prompt("Delete all files? (yes/no)").await? {
            if answer.to_lowercase() == "yes" {
                // Proceed with deletion
            }
        }

        Ok(())
    }
}
```

## Testing IO Services

Create a mock IO for testing that captures messages:

```rust
use std::sync::{Arc, Mutex};

pub struct MockIO {
    notifications: Arc<Mutex<Vec<String>>>,
    prompts: Arc<Mutex<Vec<String>>>,
    responses: Arc<Mutex<Vec<Option<String>>>>,
}

impl MockIO {
    pub fn new() -> Self {
        Self {
            notifications: Arc::new(Mutex::new(Vec::new())),
            prompts: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_responses(responses: Vec<Option<String>>) -> Self {
        Self {
            notifications: Arc::new(Mutex::new(Vec::new())),
            prompts: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(responses)),
        }
    }

    pub fn notifications(&self) -> Vec<String> {
        self.notifications.lock().unwrap().clone()
    }

    pub fn prompts(&self) -> Vec<String> {
        self.prompts.lock().unwrap().clone()
    }
}

#[async_trait]
impl PipelineIO for MockIO {
    async fn notify(&self, message: &str) -> Result<()> {
        self.notifications.lock().unwrap().push(message.to_string());
        Ok(())
    }

    async fn prompt(&self, message: &str) -> Result<Option<String>> {
        self.prompts.lock().unwrap().push(message.to_string());
        let mut responses = self.responses.lock().unwrap();
        Ok(if responses.is_empty() {
            None
        } else {
            responses.remove(0)
        })
    }
}

#[test]
fn test_command_notifications() {
    let mock = MockIO::new();
    let mut services = PipelineServices::new();
    services.add_io(mock.clone());

    // Run pipeline...

    assert_eq!(mock.notifications(), vec!["Starting...", "Complete!"]);
}
```
