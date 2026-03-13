pub mod core;

use crate::imports::*;

#[derive(Debug)]
pub enum HookEvent<'a> {
    BeforeStep {
        step_name: &'a str,
        metadata: &'a OperationMetadata,
        params: &'a Parameters,
        iter_context: Option<&'a IterContext<'a>>,
    },
    AfterStep {
        step_name: &'a str,
        metadata: &'a OperationMetadata,
        params: &'a Parameters,
        operation_outputs: &'a Store<StoreEntry>,
        global_outputs: &'a Store<StoreEntry>,
        iter_context: Option<&'a IterContext<'a>>,
    },
    BeforeIteration {
        iter_context: &'a IterContext<'a>,
    },
    AfterIteration {
        iter_context: &'a IterContext<'a>,
    },
    BeforeReturns {
        return_name: &'a str,
        params: &'a Parameters,
    },
    AfterReturns {
        return_name: &'a str,
        params: &'a Parameters,
        outputs: &'a Store<StoreEntry>,
    },
    GuardPassed {
        guard_name: &'a str,
    },
    GuardFailed {
        guard_name: &'a str,
    },
    Complete,
    Error {
        error: &'a OperationError,
    },
}

pub enum HookAction {
    Continue,
    Abort(String),
}

pub type ObserverCallback = Box<dyn Fn(&HookEvent, &Store<StoreEntry>) + Send>;
pub type InterceptorCallback = Box<dyn Fn(&HookEvent, &Store<StoreEntry>) -> HookAction + Send>;

#[non_exhaustive]
pub enum HookCallback {
    Observer(ObserverCallback),
    Interceptor(InterceptorCallback),
}

pub struct Hook {
    pub name: String,
    pub callback: HookCallback,
}

impl Hook {
    pub fn observer(
        name: impl Into<String>,
        f: impl Fn(&HookEvent, &Store<StoreEntry>) + Send + 'static,
    ) -> Self {
        Hook {
            name: name.into(),
            callback: HookCallback::Observer(Box::new(f)),
        }
    }

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
