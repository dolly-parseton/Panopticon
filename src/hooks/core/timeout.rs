use crate::imports::*;
use std::time::{Duration, Instant};

pub struct Timeout {
    name: String,
    limit: Duration,
}

impl Timeout {
    pub fn new(limit: Duration) -> Self {
        Timeout {
            name: "timeout".into(),
            limit,
        }
    }

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
