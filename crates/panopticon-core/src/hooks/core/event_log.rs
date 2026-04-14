use crate::imports::*;
use std::time::Instant;

/// A single record captured by an [`EventLog`] hook.
///
/// Records one observed [`HookEvent`] with enough context to identify
/// which step or iteration it came from and when it occurred relative to
/// the start of the run.
#[derive(Debug, Clone)]
pub struct EventRecord {
    /// The category of event observed.
    pub kind: EventKind,
    /// The step or return-block name, if the event originated from one.
    pub step_name: Option<String>,
    /// The iteration context as rendered by [`IterContext`]'s `Display`
    /// impl, if the event fired inside an iteration.
    pub iter_info: Option<String>,
    /// How long after the [`EventLog`] was attached the event fired.
    pub elapsed: std::time::Duration,
}

/// The category of a captured [`EventRecord`].
///
/// Mirrors the variants of [`HookEvent`] but without the borrowed
/// references, so records can be stored and inspected after the
/// pipeline has finished.
#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    /// A step is about to execute.
    BeforeStep,
    /// A step has just executed.
    AfterStep,
    /// An iteration body is about to start.
    BeforeIteration,
    /// An iteration body has just finished.
    AfterIteration,
    /// A return block is about to resolve.
    BeforeReturns,
    /// A return block has just resolved.
    AfterReturns,
    /// A guard's predicate evaluated to true.
    GuardPassed,
    /// A guard's predicate evaluated to false.
    GuardFailed,
    /// The pipeline finished successfully.
    Complete,
    /// The pipeline produced an error.
    Error,
}

/// A built-in hook that captures every observed event into an in-memory
/// log for post-hoc inspection.
///
/// Useful in tests that assert on the sequence of steps and iterations
/// a pipeline produced. Attach the `EventLog` with
/// `Pipeline::hook(event_log)` and retain a clone of the `log()` handle
/// to read entries once the pipeline is complete.
pub struct EventLog {
    name: String,
    log: Arc<Mutex<Vec<EventRecord>>>,
}

impl Default for EventLog {
    fn default() -> Self {
        EventLog::new()
    }
}

impl EventLog {
    /// Constructs a new event log with the default hook name.
    pub fn new() -> Self {
        EventLog {
            name: "event_log".into(),
            log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Overrides the hook name used in [`HookEvent`] abort messages.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Returns a cloned handle to the underlying log. Retain this before
    /// moving the `EventLog` into [`Pipeline::hook`] so the records can
    /// be inspected once the pipeline finishes.
    pub fn log(&self) -> Arc<Mutex<Vec<EventRecord>>> {
        Arc::clone(&self.log)
    }
}

impl From<EventLog> for Hook {
    fn from(event_log: EventLog) -> Hook {
        let log = event_log.log;
        let start = Instant::now();

        Hook::observer(event_log.name, move |event, _store| {
            let elapsed = start.elapsed();

            let iter_info_from =
                |ctx: &Option<&IterContext>| -> Option<String> { ctx.map(|c| c.to_string()) };

            let (kind, step_name, iter_info) = match event {
                HookEvent::BeforeStep {
                    step_name,
                    iter_context,
                    ..
                } => (
                    EventKind::BeforeStep,
                    Some(step_name.to_string()),
                    iter_info_from(iter_context),
                ),
                HookEvent::AfterStep {
                    step_name,
                    iter_context,
                    ..
                } => (
                    EventKind::AfterStep,
                    Some(step_name.to_string()),
                    iter_info_from(iter_context),
                ),
                HookEvent::BeforeIteration { iter_context } => (
                    EventKind::BeforeIteration,
                    None,
                    Some(iter_context.to_string()),
                ),
                HookEvent::AfterIteration { iter_context } => (
                    EventKind::AfterIteration,
                    None,
                    Some(iter_context.to_string()),
                ),
                HookEvent::BeforeReturns { return_name, .. } => (
                    EventKind::BeforeReturns,
                    Some(return_name.to_string()),
                    None,
                ),
                HookEvent::AfterReturns { return_name, .. } => {
                    (EventKind::AfterReturns, Some(return_name.to_string()), None)
                }
                HookEvent::GuardPassed { guard_name } => {
                    (EventKind::GuardPassed, Some(guard_name.to_string()), None)
                }
                HookEvent::GuardFailed { guard_name } => {
                    (EventKind::GuardFailed, Some(guard_name.to_string()), None)
                }
                HookEvent::Complete => (EventKind::Complete, None, None),
                HookEvent::Error { .. } => (EventKind::Error, None, None),
                #[allow(unreachable_patterns)]
                _ => return,
            };

            let mut l = log.lock().unwrap();
            l.push(EventRecord {
                kind,
                step_name,
                iter_info,
                elapsed,
            });
        })
    }
}
