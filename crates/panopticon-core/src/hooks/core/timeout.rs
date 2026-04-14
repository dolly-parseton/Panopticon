use crate::imports::*;
use std::time::{Duration, Instant};

/// A built-in interceptor hook that aborts the pipeline once a wall-clock
/// deadline has passed.
///
/// The clock starts when the hook is attached, not when the pipeline
/// runs — typically those are close enough that the distinction does
/// not matter. The check fires before each step; an in-flight step is
/// allowed to finish before the abort takes effect.
pub struct Timeout {
    name: String,
    limit: Duration,
}

impl Timeout {
    /// Constructs a timeout hook with the given wall-clock limit.
    pub fn new(limit: Duration) -> Self {
        Timeout {
            name: "timeout".into(),
            limit,
        }
    }

    /// Overrides the hook name used in the abort message.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl From<Timeout> for Hook {
    fn from(timeout: Timeout) -> Hook {
        let limit = timeout.limit;
        let start: Arc<Instant> = Arc::new(Instant::now());

        Hook::interceptor(timeout.name, move |event, _store| {
            if let HookEvent::BeforeStep { step_name, .. } = event {
                let elapsed = start.elapsed();
                if elapsed > limit {
                    return HookAction::Abort(format!(
                        "Pipeline exceeded timeout of {:.1}s (elapsed: {:.1}s) before step '{}'",
                        limit.as_secs_f64(),
                        elapsed.as_secs_f64(),
                        step_name,
                    ));
                }
            }
            HookAction::Continue
        })
    }
}
