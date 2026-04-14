mod complete;
mod draft;
mod ready;
mod running;

use crate::imports::*;

/// Typestate marker for the draft phase.
///
/// A pipeline enters the draft phase on construction and stays there while
/// variables, steps, collection handles, guards, iterations, returns, hooks,
/// and extensions are being added. The draft phase is the only phase in which
/// the pipeline's shape can change; it is left by calling
/// [`Pipeline::compile`](crate::prelude::Pipeline#impl-Pipeline<Draft>), which
/// validates the draft and promotes the pipeline to [`Ready`].
///
/// # Lifecycle
///
/// A pipeline moves through four phases, each represented by a distinct
/// typestate marker:
///
/// | Phase       | Reached by                | What it allows                          |
/// |-------------|---------------------------|------------------------------------------|
/// | [`Draft`]   | `Pipeline::default()`     | Adding variables, steps, and control flow |
/// | [`Ready`]   | `Pipeline::compile()`     | Inspecting the validated execution graph  |
/// | [`Running`] | `Pipeline::run()`         | Polling status or cancelling the thread   |
/// | [`Complete`] | `Pipeline::wait()`        | Reading variables and returns             |
///
/// The `Draft` struct holds the in-progress state: the variables and
/// parameters stores, the execution order, the registry of known operations,
/// the attached hooks, and the registered extensions. None of these fields
/// are public; they are manipulated exclusively through the construction
/// methods on `Pipeline<Draft>`.
pub struct Draft {
    pub(crate) variables: Store<StoreEntry>,
    pub(crate) parameters: Store<Parameters>,
    pub(crate) returns: Store<Parameters>,
    pub(crate) execution_order: Vec<ExecutionNode>,
    pub(crate) registry: Registry,
    pub(crate) hooks: Vec<Hook>,
    pub(crate) extensions: Extensions,
}

/// Typestate marker for the ready phase.
///
/// A pipeline reaches the ready phase after [`Pipeline::compile`] has
/// validated the draft — simulating execution to catch unresolved references,
/// forward references, and missing sources. Ready pipelines are immutable:
/// their execution order, parameters, variables, and returns can be inspected
/// (useful for serialisers and other tooling) but not changed. Call
/// `Pipeline::run` to move to [`Running`].
pub struct Ready {
    pub(crate) variables: Store<StoreEntry>,
    pub(crate) parameters: Store<Parameters>,
    pub(crate) returns: Store<Parameters>,
    pub(crate) execution_order: Vec<ExecutionNode>,
    pub(crate) registry: Registry,
    pub(crate) hooks: Vec<Hook>,
    pub(crate) extensions: Extensions,
}

/// Typestate marker for the running phase.
///
/// A pipeline enters the running phase when [`Pipeline::run`] spawns a
/// background worker thread to execute the validated graph. In this phase the
/// pipeline can be polled via `Pipeline::poll`, cancelled via
/// `Pipeline::cancel`, or joined via `Pipeline::wait`. The three fields hold
/// the worker thread handle, a shared atomic cancellation flag, and a shared
/// result slot the worker fills on completion.
pub struct Running {
    pub(crate) handle: thread::JoinHandle<()>,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) result: Arc<Mutex<Option<Result<Complete, OperationError>>>>,
}

/// Status snapshot returned by `Pipeline::poll` on a running pipeline.
///
/// Reflects the state of the worker thread at the moment of the call. A
/// pipeline that reports `Completed`, `Cancelled`, or `Failed` has already
/// finished; calling `Pipeline::wait` at that point returns immediately.
pub enum RunningStatus {
    /// The worker thread is still executing the graph.
    Running,
    /// The worker thread finished successfully.
    Completed,
    /// The worker thread observed the cancellation flag and exited early.
    Cancelled,
    /// The worker thread produced an [`OperationError`].
    Failed,
}

/// Typestate marker for the complete phase.
///
/// A pipeline reaches the complete phase when [`Pipeline::wait`] joins the
/// worker thread successfully. The two stores — variables (the full state at
/// the end of execution) and returns (the values produced by declared return
/// blocks) — are accessible through `Pipeline<Complete>`'s read-only
/// accessors and, with the `serde` feature, `deserialize_returns`.
pub struct Complete {
    variables: Store<StoreEntry>,
    returns: Store<StoreEntry>,
}

/// A typestate pipeline. The generic parameter `S` identifies the current
/// lifecycle phase and determines which methods are callable.
///
/// A pipeline is constructed empty in the [`Draft`] phase, populated with
/// variables and steps, compiled to [`Ready`] for validation, run to
/// [`Running`] on a background thread, and finally joined to [`Complete`] to
/// read results. Each phase exposes only the methods that make sense for
/// that phase — calling `step` on a `Pipeline<Ready>` is a compile error,
/// not a runtime failure.
///
/// ```no_run
/// use panopticon_core::prelude::*;
///
/// let mut pipe = Pipeline::default();
/// pipe.var("name", "world")?;
/// pipe.step::<SetVar>(
///     "greet",
///     params!("name" => "greeting", "value" => "hello, world"),
/// )?;
/// let complete = pipe.compile()?.run().wait()?;
/// let greeting = complete.variables().get("greet.greeting")?;
/// # let _ = greeting;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Pipeline<S = Draft> {
    state: S,
}

impl Default for Pipeline {
    fn default() -> Self {
        Pipeline::new()
    }
}
