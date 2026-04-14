pub mod core;

use crate::imports::*;

/// A structured event fired during pipeline execution.
///
/// `HookEvent` is the input to every [`Hook`] callback. The variants
/// mirror the runtime's timeline: `Before*` events fire immediately
/// before the associated action, `After*` events fire immediately
/// after, and [`Complete`](Self::Complete) / [`Error`](Self::Error)
/// bracket the end of the run. The borrowed references let hooks
/// inspect step metadata, parameters, and outputs without cloning.
#[derive(Debug)]
pub enum HookEvent<'a> {
    /// Fires immediately before a step's operation runs. Parameters
    /// have been resolved but no outputs have been staged yet.
    BeforeStep {
        /// The step's name (prefixed if inside a child pipeline).
        step_name: &'a str,
        /// The operation's static metadata.
        metadata: &'a OperationMetadata,
        /// The unresolved parameters as declared at draft time.
        params: &'a Parameters,
        /// Present when the step is nested inside an iteration.
        iter_context: Option<&'a IterContext<'a>>,
    },
    /// Fires immediately after a step's operation runs. Operation
    /// outputs and global outputs are staged but not yet merged into
    /// the runtime store.
    AfterStep {
        /// The step's name.
        step_name: &'a str,
        /// The operation's static metadata.
        metadata: &'a OperationMetadata,
        /// The unresolved parameters.
        params: &'a Parameters,
        /// Outputs scoped to the operation only (not merged into the
        /// runtime store).
        operation_outputs: &'a Store<StoreEntry>,
        /// Outputs about to be merged into the runtime store.
        global_outputs: &'a Store<StoreEntry>,
        /// Present when the step is nested inside an iteration.
        iter_context: Option<&'a IterContext<'a>>,
    },
    /// Fires before each iteration body executes.
    BeforeIteration {
        /// The current iteration's context.
        iter_context: &'a IterContext<'a>,
    },
    /// Fires after each iteration body finishes.
    AfterIteration {
        /// The current iteration's context.
        iter_context: &'a IterContext<'a>,
    },
    /// Fires before a return block resolves its parameters.
    BeforeReturns {
        /// The return block's name.
        return_name: &'a str,
        /// The unresolved parameters.
        params: &'a Parameters,
    },
    /// Fires after a return block resolves its parameters.
    AfterReturns {
        /// The return block's name.
        return_name: &'a str,
        /// The unresolved parameters.
        params: &'a Parameters,
        /// The returns store with the resolved values.
        outputs: &'a Store<StoreEntry>,
    },
    /// Fires when a guard's boolean source evaluates to true. The
    /// guard body is about to execute.
    GuardPassed {
        /// The guard node's name.
        guard_name: &'a str,
    },
    /// Fires when a guard's boolean source evaluates to false. The
    /// guard body is skipped.
    GuardFailed {
        /// The guard node's name.
        guard_name: &'a str,
    },
    /// Fires once at the end of a successful run, after all return
    /// blocks have resolved.
    Complete,
    /// Fires once when the worker thread produces an error. The
    /// pipeline is about to fail.
    Error {
        /// The error the worker is propagating.
        error: &'a OperationError,
    },
}

/// The decision an interceptor hook returns to the runtime.
///
/// Observer hooks have no return value; interceptor hooks return a
/// `HookAction` to either let execution proceed ([`Continue`](Self::Continue))
/// or stop the pipeline with an error ([`Abort`](Self::Abort)).
pub enum HookAction {
    /// Let execution proceed to the next step or event.
    Continue,
    /// Stop the pipeline. The runtime wraps the message in
    /// [`OperationError::HookAbort`].
    Abort(String),
}

/// An observer callback: read-only access to every event.
pub type ObserverCallback = Box<dyn Fn(&HookEvent, &Store<StoreEntry>) + Send>;

/// An interceptor callback: read-only access to every event plus a
/// [`HookAction`] return value for aborting the pipeline.
pub type InterceptorCallback = Box<dyn Fn(&HookEvent, &Store<StoreEntry>) -> HookAction + Send>;

/// A type-erased hook callback — either an observer or an
/// interceptor. `#[non_exhaustive]` to leave room for future
/// callback flavours.
#[non_exhaustive]
pub enum HookCallback {
    /// A read-only observer.
    Observer(ObserverCallback),
    /// An interceptor that can abort the pipeline.
    Interceptor(InterceptorCallback),
}

/// An observer or interceptor attached to a pipeline via
/// [`Pipeline::hook`](crate::prelude::Pipeline#method.hook).
///
/// Construct with [`Hook::observer`] for read-only event tracing or
/// [`Hook::interceptor`] for callbacks that can abort the pipeline.
/// Built-in hooks ([`Logger`](crate::prelude::Logger),
/// [`Profiler`](crate::prelude::Profiler),
/// [`StepFilter`](crate::prelude::StepFilter), and friends) implement
/// `Into<Hook>` so they can be passed directly to `Pipeline::hook`.
pub struct Hook {
    /// The hook's name, surfaced in abort messages.
    pub name: String,
    /// The underlying callback.
    pub callback: HookCallback,
}

impl Hook {
    /// Wraps a closure as a read-only observer hook. Observers cannot
    /// abort the pipeline.
    pub fn observer(
        name: impl Into<String>,
        f: impl Fn(&HookEvent, &Store<StoreEntry>) + Send + 'static,
    ) -> Self {
        Hook {
            name: name.into(),
            callback: HookCallback::Observer(Box::new(f)),
        }
    }

    /// Wraps a closure as an interceptor hook. Interceptors inspect
    /// each event and return a [`HookAction`] — use [`HookAction::Abort`]
    /// to stop the pipeline.
    pub fn interceptor(
        name: impl Into<String>,
        f: impl Fn(&HookEvent, &Store<StoreEntry>) -> HookAction + Send + 'static,
    ) -> Self {
        Hook {
            name: name.into(),
            callback: HookCallback::Interceptor(Box::new(f)),
        }
    }

    pub(crate) fn emit(
        &self,
        event: &HookEvent,
        store: &Store<StoreEntry>,
    ) -> Result<(), OperationError> {
        match &self.callback {
            HookCallback::Observer(f) => {
                f(event, store);
                Ok(())
            }
            HookCallback::Interceptor(f) => match f(event, store) {
                HookAction::Continue => Ok(()),
                HookAction::Abort(reason) => Err(OperationError::HookAbort {
                    hook: self.name.clone(),
                    reason,
                }),
            },
        }
    }
}

pub(crate) fn emit_all(
    hooks: &[Hook],
    event: &HookEvent,
    store: &Store<StoreEntry>,
) -> Result<(), OperationError> {
    for hook in hooks {
        hook.emit(event, store)?;
    }
    Ok(())
}
