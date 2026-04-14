use crate::execution::runner::execute_steps;
use crate::imports::*;

impl Pipeline<Ready> {
    /// Read-only access to the validated execution order. Intended for
    /// external callers that walk the pipeline graph (e.g. serialisers).
    pub fn execution_order(&self) -> &[ExecutionNode] {
        &self.state.execution_order
    }
    /// Read-only access to the parameters store keyed by (prefixed) step name.
    pub fn parameters(&self) -> &Store<Parameters> {
        &self.state.parameters
    }
    /// Read-only access to the variables store as defined at draft time.
    pub fn variables(&self) -> &Store<StoreEntry> {
        &self.state.variables
    }
    /// Read-only access to the returns store keyed by return-block name.
    pub fn returns(&self) -> &Store<Parameters> {
        &self.state.returns
    }
}

impl Pipeline<Ready> {
    /// Promotes the pipeline from [`Ready`] to [`Running`], spawning a
    /// background worker thread that executes the validated graph.
    ///
    /// Returns immediately — use [`Pipeline::wait`] to join the thread and
    /// recover the [`Complete`] pipeline, [`Pipeline::poll`] to check the
    /// current [`RunningStatus`], or [`Pipeline::cancel`] to request early
    /// termination. All hook callbacks fire on the worker thread.
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
