use crate::imports::*;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct EventRecord {
    pub kind: EventKind,
    pub step_name: Option<String>,
    pub iter_info: Option<String>,
    pub elapsed: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    BeforeStep,
    AfterStep,
    BeforeIteration,
    AfterIteration,
    BeforeReturns,
    AfterReturns,
    GuardPassed,
    GuardFailed,
    Complete,
    Error,
}

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
    pub fn new() -> Self {
        EventLog {
            name: "event_log".into(),
            log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

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
