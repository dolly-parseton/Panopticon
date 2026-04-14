use crate::imports::*;

enum EntryKind {
    Var(Type),
    Array,
    Map,
}

struct Expectation {
    key: String,
    kind: EntryKind,
    after_step: Option<String>,
}

/// A built-in interceptor hook that asserts store state after steps.
///
/// Constructed fluently: chain [`expect`](Self::expect),
/// [`expect_array`](Self::expect_array), and
/// [`expect_map`](Self::expect_map) to declare the keys and shapes that
/// must be present. Use [`after_step`](Self::after_step) to scope
/// expectations to a specific step. When any expectation fails after a
/// step, the pipeline aborts with [`OperationError::HookAbort`].
pub struct StoreValidator {
    name: String,
    expectations: Vec<Expectation>,
    scope_step: Option<String>,
}

impl Default for StoreValidator {
    fn default() -> Self {
        StoreValidator::new()
    }
}

impl StoreValidator {
    /// Constructs a validator with no expectations and no scope.
    pub fn new() -> Self {
        StoreValidator {
            name: "store_validator".into(),
            expectations: Vec::new(),
            scope_step: None,
        }
    }

    /// Overrides the hook name used in abort messages.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Only validate after a specific step. Applies to all subsequent `expect` calls
    /// until `after_step` is called again or `after_any_step` resets it.
    pub fn after_step(mut self, step_name: impl Into<String>) -> Self {
        self.scope_step = Some(step_name.into());
        self
    }

    /// Reset so subsequent `expect` calls validate after every step.
    pub fn after_any_step(mut self) -> Self {
        self.scope_step = None;
        self
    }

    /// Expect a store key to exist with the given scalar type.
    pub fn expect(mut self, key: impl Into<String>, ty: Type) -> Self {
        self.expectations.push(Expectation {
            key: key.into(),
            kind: EntryKind::Var(ty),
            after_step: self.scope_step.clone(),
        });
        self
    }

    /// Expect a store key to hold an array.
    pub fn expect_array(mut self, key: impl Into<String>) -> Self {
        self.expectations.push(Expectation {
            key: key.into(),
            kind: EntryKind::Array,
            after_step: self.scope_step.clone(),
        });
        self
    }

    /// Expect a store key to hold a map.
    pub fn expect_map(mut self, key: impl Into<String>) -> Self {
        self.expectations.push(Expectation {
            key: key.into(),
            kind: EntryKind::Map,
            after_step: self.scope_step.clone(),
        });
        self
    }
}

impl From<StoreValidator> for Hook {
    fn from(validator: StoreValidator) -> Hook {
        let expectations: Arc<Vec<Expectation>> = Arc::new(validator.expectations);

        Hook::interceptor(validator.name, move |event, store| {
            let (step_name, operation_outputs, global_outputs) = match event {
                HookEvent::AfterStep {
                    step_name,
                    operation_outputs,
                    global_outputs,
                    ..
                } => (*step_name, Some(*operation_outputs), Some(*global_outputs)),
                _ => return HookAction::Continue,
            };

            // Look up a key across the main store and the step's pending outputs.
            let lookup = |key: &str| -> Option<&StoreEntry> {
                if let Ok(entry) = store.get(key) {
                    return Some(entry);
                }
                if let Some(outputs) = global_outputs
                    && let Ok(entry) = outputs.get(key)
                {
                    return Some(entry);
                }

                if let Some(outputs) = operation_outputs
                    && let Ok(entry) = outputs.get(key)
                {
                    return Some(entry);
                }

                None
            };

            for exp in expectations.iter() {
                // Skip if this expectation is scoped to a different step.
                if let Some(ref scoped) = exp.after_step
                    && scoped != step_name
                {
                    continue;
                }

                match lookup(&exp.key) {
                    None => {
                        return HookAction::Abort(format!(
                            "Expected key '{}' to exist in store after step '{}'",
                            exp.key, step_name,
                        ));
                    }
                    Some(entry) => match &exp.kind {
                        EntryKind::Var(expected_ty) => match entry.as_var() {
                            Ok((_val, actual_ty)) => {
                                if *expected_ty != Type::Any && *actual_ty != *expected_ty {
                                    return HookAction::Abort(format!(
                                        "Key '{}' has type {} but expected {} after step '{}'",
                                        exp.key, actual_ty, expected_ty, step_name,
                                    ));
                                }
                            }
                            Err(_) => {
                                return HookAction::Abort(format!(
                                    "Key '{}' is not a scalar variable after step '{}'",
                                    exp.key, step_name,
                                ));
                            }
                        },
                        EntryKind::Array => {
                            if entry.as_array().is_err() {
                                return HookAction::Abort(format!(
                                    "Key '{}' is not an array after step '{}'",
                                    exp.key, step_name,
                                ));
                            }
                        }
                        EntryKind::Map => {
                            if entry.as_map().is_err() {
                                return HookAction::Abort(format!(
                                    "Key '{}' is not a map after step '{}'",
                                    exp.key, step_name,
                                ));
                            }
                        }
                    },
                }
            }

            HookAction::Continue
        })
    }
}
