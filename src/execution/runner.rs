use crate::imports::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_steps(
    execution_order: &[ExecutionNode],
    parameters: &Store<Parameters>,
    registry: &Registry,
    hooks: &[Hook],
    extensions: &Extensions,
    variables: &mut Store<StoreEntry>,
    cancel: &AtomicBool,
    iter_context: Option<&IterContext>,
) -> Result<(), OperationError> {
    for node in execution_order {
        match node {
            ExecutionNode::Step {
                name: step_name,
                type_id,
            } => {
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(OperationError::Cancelled);
                }

                let params =
                    parameters
                        .get(step_name)
                        .map_err(|_| OperationError::MissingParameters {
                            name: step_name.clone(),
                        })?;

                let op_entry = registry.get(type_id).expect("validated during compile");

                emit_all(
                    hooks,
                    &HookEvent::BeforeStep {
                        step_name,
                        metadata: &op_entry.metadata,
                        params,
                        iter_context,
                    },
                    variables,
                )?;

                let param_prefix = params.resolve_in_store(step_name, variables)?;

                let mut ctx = Context::new(
                    op_entry.metadata.clone(),
                    &param_prefix,
                    variables,
                    extensions,
                );
                op_entry.execute(&mut ctx)?;

                let (operation_outputs, global_outputs) = ctx.consume();

                emit_all(
                    hooks,
                    &HookEvent::AfterStep {
                        step_name,
                        metadata: &op_entry.metadata,
                        params,
                        operation_outputs: &operation_outputs,
                        global_outputs: &global_outputs,
                        iter_context,
                    },
                    variables,
                )?;

                variables.merge(Some(step_name), operation_outputs)?;
                variables.merge(None, global_outputs)?;
            }
            ExecutionNode::IterArray {
                name,
                source,
                index_binding,
                item_binding,
                body,
            } => {
                let array = variables
                    .get(&source.reference)
                    .map_err(|_| OperationError::IterSourceNotFound {
                        iter_name: name.clone(),
                        source_ref: source.reference.clone(),
                    })?
                    .as_array()
                    .map_err(|_| OperationError::IterSourceTypeMismatch {
                        iter_name: name.clone(),
                        source_ref: source.reference.clone(),
                        expected: "an array",
                    })?
                    .clone();

                for (i, item) in array.into_iter().enumerate() {
                    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                        return Err(OperationError::Cancelled);
                    }

                    let ctx = IterContext {
                        iter_name: name,
                        index: IterIndex::Array(i),
                        depth: iter_context.map_or(0, |c| c.depth + 1),
                    };

                    emit_all(
                        hooks,
                        &HookEvent::BeforeIteration { iter_context: &ctx },
                        variables,
                    )?;

                    // Clone parent store — each iteration is independent
                    let mut iter_store = variables.clone();

                    // Inject iteration bindings
                    iter_store.insert_or_replace(
                        index_binding.clone(),
                        StoreEntry::Var {
                            value: Value::Integer(i as i64),
                            ty: Type::Integer,
                        },
                    );
                    iter_store.insert_or_replace(item_binding.clone(), item);

                    // Execute body against the cloned store
                    execute_steps(
                        body,
                        parameters,
                        registry,
                        hooks,
                        extensions,
                        &mut iter_store,
                        cancel,
                        Some(&ctx),
                    )
                    .map_err(|e| OperationError::Iteration {
                        iter_name: name.clone(),
                        index: i.to_string(),
                        depth: ctx.depth,
                        source: Box::new(e),
                    })?;

                    // Diff: collect keys new to this iteration and merge under index prefix
                    let iter_prefix = format!("{}.{}", name, i);
                    for (key, value) in iter_store {
                        if variables.get(&key).is_err() {
                            variables.insert_or_replace(format!("{}.{}", iter_prefix, key), value);
                        }
                    }

                    emit_all(
                        hooks,
                        &HookEvent::AfterIteration { iter_context: &ctx },
                        variables,
                    )?;
                }
            }
            ExecutionNode::IterMap {
                name,
                source,
                key_binding,
                value_binding,
                body,
            } => {
                let map = variables
                    .get(&source.reference)
                    .map_err(|_| OperationError::IterSourceNotFound {
                        iter_name: name.clone(),
                        source_ref: source.reference.clone(),
                    })?
                    .as_map()
                    .map_err(|_| OperationError::IterSourceTypeMismatch {
                        iter_name: name.clone(),
                        source_ref: source.reference.clone(),
                        expected: "a map",
                    })?
                    .clone();

                for (key, value) in map.into_iter() {
                    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                        return Err(OperationError::Cancelled);
                    }

                    let ctx = IterContext {
                        iter_name: name,
                        index: IterIndex::Map(&key),
                        depth: iter_context.map_or(0, |c| c.depth + 1),
                    };

                    emit_all(
                        hooks,
                        &HookEvent::BeforeIteration { iter_context: &ctx },
                        variables,
                    )?;

                    let mut iter_store = variables.clone();

                    iter_store.insert_or_replace(
                        key_binding.clone(),
                        StoreEntry::Var {
                            value: Value::Text(key.clone()),
                            ty: Type::Text,
                        },
                    );
                    iter_store.insert_or_replace(value_binding.clone(), value);

                    execute_steps(
                        body,
                        parameters,
                        registry,
                        hooks,
                        extensions,
                        &mut iter_store,
                        cancel,
                        Some(&ctx),
                    )
                    .map_err(|e| OperationError::Iteration {
                        iter_name: name.clone(),
                        index: key.clone(),
                        depth: ctx.depth,
                        source: Box::new(e),
                    })?;

                    let iter_prefix = format!("{}.{}", name, key);
                    for (k, v) in iter_store {
                        if variables.get(&k).is_err() {
                            variables.insert_or_replace(format!("{}.{}", iter_prefix, k), v);
                        }
                    }

                    emit_all(
                        hooks,
                        &HookEvent::AfterIteration { iter_context: &ctx },
                        variables,
                    )?;
                }
            }
            ExecutionNode::Guard { name, source, body } => {
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(OperationError::Cancelled);
                }

                let condition = variables
                    .get(&source.reference)
                    .map_err(|_| OperationError::Guard {
                        guard_name: name.clone(),
                        source: Box::new(OperationError::ReferenceNotFound {
                            reference: source.reference.clone(),
                        }),
                    })?
                    .get_value()
                    .map_err(|_| OperationError::GuardTypeMismatch {
                        guard_name: name.clone(),
                        reference: source.reference.clone(),
                    })?
                    .as_boolean()
                    .map_err(|_| OperationError::GuardTypeMismatch {
                        guard_name: name.clone(),
                        reference: source.reference.clone(),
                    })?;

                if condition {
                    emit_all(
                        hooks,
                        &HookEvent::GuardPassed { guard_name: name },
                        variables,
                    )?;

                    execute_steps(
                        body,
                        parameters,
                        registry,
                        hooks,
                        extensions,
                        variables,
                        cancel,
                        iter_context,
                    )
                    .map_err(|e| OperationError::Guard {
                        guard_name: name.clone(),
                        source: Box::new(e),
                    })?;
                } else {
                    emit_all(
                        hooks,
                        &HookEvent::GuardFailed { guard_name: name },
                        variables,
                    )?;
                }
            }
        }
    }

    Ok(())
}
