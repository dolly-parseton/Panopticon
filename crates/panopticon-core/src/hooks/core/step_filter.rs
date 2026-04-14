use crate::imports::*;
use std::collections::HashSet;

enum FilterMode {
    Allow(HashSet<String>),
    Deny(HashSet<String>),
}

/// A built-in interceptor hook that permits or blocks steps by name.
///
/// Construct with [`allow`](Self::allow) for an allow-list or
/// [`deny`](Self::deny) for a deny-list. Blocked steps cause the
/// pipeline to abort with [`OperationError::HookAbort`]. Useful for
/// gating optional work and for tests that want to skip expensive
/// steps.
pub struct StepFilter {
    name: String,
    mode: FilterMode,
}

impl StepFilter {
    /// Constructs a filter that permits only the listed steps. Any step
    /// not in the list triggers a hook abort.
    pub fn allow(steps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        StepFilter {
            name: "step_filter".into(),
            mode: FilterMode::Allow(steps.into_iter().map(Into::into).collect()),
        }
    }

    /// Constructs a filter that blocks the listed steps. Any step in
    /// the list triggers a hook abort.
    pub fn deny(steps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        StepFilter {
            name: "step_filter".into(),
            mode: FilterMode::Deny(steps.into_iter().map(Into::into).collect()),
        }
    }

    /// Overrides the hook name used in the abort message.
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
