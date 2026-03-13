use crate::execution::runner::execute_steps;
use crate::imports::*;

impl Pipeline<Ready> {
    pub fn run(self) -> Pipeline<Running> {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();

        let result = Arc::new(Mutex::new(None));
        let result_clone = result.clone();

        let Ready {
            mut variables,
            parameters,
            returns,
            execution_order,
            registry,
            hooks,
            extensions,
        } = self.state;

        let handle = thread::spawn(move || {
            let outcome = (|| -> Result<Complete, OperationError> {
                execute_steps(
                    &execution_order,
                    &parameters,
                    &registry,
                    &hooks,
                    &extensions,
                    &mut variables,
                    &cancel,
                    None,
                )?;

                let mut returns_store = Store::<StoreEntry>::new();
                for return_name in returns.keys() {
                    let params = returns.get(return_name).map_err(|_| {
                        OperationError::MissingParameters {
                            name: return_name.clone(),
                        }
                    })?;

                    emit_all(
                        &hooks,
                        &HookEvent::BeforeReturns {
                            return_name,
                            params,
                        },
                        &variables,
                    )?;

                    params.resolve_to_store(return_name, &variables, &mut returns_store)?;

                    emit_all(
                        &hooks,
                        &HookEvent::AfterReturns {
                            return_name,
                            params,
                            outputs: &returns_store,
                        },
                        &variables,
                    )?;
                }

                emit_all(&hooks, &HookEvent::Complete, &variables)?;

                Ok(Complete {
                    variables,
                    returns: returns_store,
                })
            })();

            if let Err(ref e) = outcome {
                let empty = Store::<StoreEntry>::new();
                let _ = emit_all(&hooks, &HookEvent::Error { error: e }, &empty);
            }

            *result_clone.lock().unwrap() = Some(outcome);
        });

        Pipeline {
            state: Running {
                handle,
                cancel: cancel_clone,
                result,
            },
        }
    }
}
