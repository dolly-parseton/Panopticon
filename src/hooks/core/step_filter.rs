use crate::imports::*;
use std::collections::HashSet;

enum FilterMode {
    Allow(HashSet<String>),
    Deny(HashSet<String>),
}

pub struct StepFilter {
    name: String,
    mode: FilterMode,
}

impl StepFilter {
    /// Create a filter that allows only the listed steps to execute.
    pub fn allow(steps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        StepFilter {
            name: "step_filter".into(),
            mode: FilterMode::Allow(steps.into_iter().map(Into::into).collect()),
        }
    }

    /// Create a filter that denies the listed steps from executing.
    pub fn deny(steps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        StepFilter {
            name: "step_filter".into(),
            mode: FilterMode::Deny(steps.into_iter().map(Into::into).collect()),
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl From<StepFilter> for Hook {
    fn from(filter: StepFilter) -> Hook {
        let mode: Arc<FilterMode> = Arc::new(filter.mode);

        Hook::interceptor(filter.name, move |event, _store| {
            let step_name = match event {
                HookEvent::BeforeStep { step_name, .. } => *step_name,
                _ => return HookAction::Continue,
            };

            match mode.as_ref() {
                FilterMode::Allow(allowed) => {
                    if !allowed.contains(step_name) {
                        return HookAction::Abort(format!(
                            "Step '{}' is not in the allow list",
                            step_name,
                        ));
                    }
                }
                FilterMode::Deny(denied) => {
                    if denied.contains(step_name) {
                        return HookAction::Abort(format!(
                            "Step '{}' is in the deny list",
                            step_name,
                        ));
                    }
                }
            }

            HookAction::Continue
        })
    }
}
