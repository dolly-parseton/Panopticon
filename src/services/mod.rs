/*
    Pipeline services module:
    * A pipeline service is a type that impls a given Service trait.
    * A service trait defines some functionality that can be used by commands during execution.
    * PipelineServices is a struct that's stored in the ExecutionContext and provides access to the various services.

    We're going to start with two services:
    * A IO service for user interaction (notify/prompt).
    * A hooks service for registering pre/post-execution hooks. - Not sure how this looks just yet.

    This will enable PipelineServices to support multiple IOs, hooks, etc without requiring commands to know about the specifics.
    We'll then also provide some built-in implementations of these services, e.g. CLI interaction, channel-based interaction, etc. Extend crates can then provide their own.

    We'll have to also look at how pipeline can be extended to accept these services during definition, I think it's a Draft concern not a Ready concern.
    Tho we're looking at command use there's no reason not to treat this as something Pipeline can call independently, e.g. IO for commands/namespace add events, UI feedback
*/
use crate::imports::*;

pub mod hook_events;
pub mod io;

mod event_hooks; // Built-in Implementations of EventHooks

/*
    Hook dispatch helper - collects errors from multiple hook calls and aggregates them into a single Result.
*/
fn collect_hook_errors(errors: Vec<anyhow::Error>) -> Result<()> {
    if errors.is_empty() {
        Ok(())
    } else {
        let msg = errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        Err(anyhow::anyhow!("Hook service errors: {msg}"))
    }
}

macro_rules! hook_dispatch {
    ($method:ident, $event_ty:ty) => {
        pub async fn $method(&self, event: $event_ty) -> Result<()> {
            let mut errors = Vec::new();
            for hook in &self.hooks {
                if let Err(e) = hook.$method(&event).await {
                    errors.push(e);
                }
            }
            collect_hook_errors(errors)
        }
    };
}

#[derive(Clone, Default)]
pub struct PipelineServices {
    io: Vec<Arc<dyn PipelineIO>>,
    hooks: Vec<Arc<dyn EventHooks>>,
}

impl std::fmt::Debug for PipelineServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineServices")
            .field("io_count", &self.io.len())
            .field("hooks_count", &self.hooks.len())
            .finish()
    }
}

impl PipelineServices {
    // Builder methods
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_io<T: PipelineIO + 'static>(&mut self, io: T) {
        self.io.push(Arc::new(io));
    }

    pub fn add_hook<T: EventHooks + 'static>(&mut self, hook: T) {
        self.hooks.push(Arc::new(hook));
    }

    // Single notify method, applies to all registered IO services
    pub async fn notify(&self, message: &str) -> Result<()> {
        let mut errors = Vec::new();
        for io in &self.io {
            if let Err(e) = io.notify(message).await {
                errors.push(e);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            let msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            Err(anyhow::anyhow!("IO service errors: {msg}"))
        }
    }

    // Single prompt method, applies to all registered IO services until one returns a response
    pub async fn prompt(&self, message: &str) -> Result<Option<String>> {
        for io in &self.io {
            if let Some(response) = io.prompt(message).await? {
                return Ok(Some(response));
            }
        }
        Ok(None)
    }

    // Hook methods
    // Draft phase
    hook_dispatch!(after_added_namespace, hook_events::NamespaceInit);
    hook_dispatch!(after_added_command, hook_events::CommandInit);
    hook_dispatch!(before_compile_pipeline, hook_events::PipelineInfo);
    hook_dispatch!(after_compile_pipeline, hook_events::PipelineCompiled);

    // Ready phase
    hook_dispatch!(before_execute_pipeline, hook_events::PipelineInfo);
    hook_dispatch!(after_execute_pipeline, hook_events::PipelineExecuted);
    hook_dispatch!(before_execute_namespace, hook_events::NamespaceInfo);
    hook_dispatch!(after_execute_namespace, hook_events::NamespaceExecuted);
    hook_dispatch!(before_execute_command, hook_events::CommandInfo);
    hook_dispatch!(after_execute_command, hook_events::CommandExecuted);

    // Completed phase
    hook_dispatch!(on_results_start, hook_events::PipelineInfo);
    hook_dispatch!(on_results_finish, hook_events::PipelineCompleted);

    pub fn defaults() -> Self {
        let mut services = Self::new();
        // Check if we're using the debug build, if so add the debug event hook service
        #[cfg(debug_assertions)]
        {
            services.add_hook(event_hooks::debug::DebugEventHooks);
            services.add_io(io::StdoutInteraction);
        }
        services
    }
}

/*
    Service traits:
    * PipelineIO - Support for interaction outside of the pipeline.
    * Hook - Support for pre/post execution hooks.

    Default impls on all methods so that types only need to implement what they care about.
*/
#[async_trait]
pub trait PipelineIO: Send + Sync {
    async fn notify(&self, message: &str) -> Result<()> {
        Ok(())
    }
    async fn prompt(&self, message: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

// Default IMPL on all as not all types will need to implement all methods.
#[async_trait]
pub trait EventHooks: Send + Sync {
    // Pipeline - Draft phase
    async fn after_added_namespace(&self, event: &hook_events::NamespaceInit) -> Result<()> {
        Ok(())
    } // Should really be sync but keeping it consistent to reduce upstream API complexity.
    async fn after_added_command(&self, event: &hook_events::CommandInit) -> Result<()> {
        Ok(())
    }
    async fn before_compile_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        Ok(())
    }
    async fn after_compile_pipeline(&self, event: &hook_events::PipelineCompiled) -> Result<()> {
        Ok(())
    }

    // Pipeline - Ready phase (async)
    async fn before_execute_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        Ok(())
    }
    async fn after_execute_pipeline(&self, event: &hook_events::PipelineExecuted) -> Result<()> {
        Ok(())
    }
    async fn before_execute_namespace(&self, event: &hook_events::NamespaceInfo) -> Result<()> {
        Ok(())
    }
    async fn after_execute_namespace(&self, event: &hook_events::NamespaceExecuted) -> Result<()> {
        Ok(())
    }
    async fn before_execute_command(&self, event: &hook_events::CommandInfo) -> Result<()> {
        Ok(())
    }
    async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
        Ok(())
    }

    // Pipeline - Completed phase
    async fn on_results_start(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        Ok(())
    }
    async fn on_results_finish(&self, event: &hook_events::PipelineCompleted) -> Result<()> {
        Ok(())
    }
}
