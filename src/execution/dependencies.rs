use crate::imports::*;

const PLACEHOLDER: &str = "__placeholder__";

fn placeholder() -> StoreEntry {
    StoreEntry::Var {
        value: Value::Text(PLACEHOLDER.into()),
        ty: Type::Text,
    }
}

fn validate_nodes(
    nodes: &[ExecutionNode],
    compile_store: &mut Store<StoreEntry>,
    parameters: &Store<Parameters>,
    registry: &Registry,
    // TODO ADD EXTENSIONS
) -> Result<(), DraftError> {
    for node in nodes {
        match node {
            ExecutionNode::Step {
                name: step_name,
                type_id,
            } => {
                let params = parameters.get(step_name).expect("inserted during step()");

                // Phase A: Resolve params into compile-time store (mirrors runtime)
                params
                    .resolve_in_store(step_name, compile_store)
                    .map_err(|e| {
                        let reference = match &e {
                            OperationError::ReferenceNotFound { reference } => reference.clone(),
                            other => other.to_string(),
                        };
                        DraftError::UnresolvedReference {
                            step: step_name.clone(),
                            reference,
                        }
                    })?;

                let entry = registry.get(type_id).expect("registered during step()");

                for output in entry.metadata.outputs {
                    match (&output.name, &output.scope) {
                        (NameSpec::Static(name), OutputScope::Operation) => {
                            compile_store.insert_or_replace(
                                format!("{}.{}", step_name, name),
                                placeholder(),
                            );
                        }
                        (NameSpec::Static(name), OutputScope::Global) => {
                            compile_store.insert_or_replace(name.to_string(), placeholder());
                        }
                        (output_spec, scope) => {
                            let derived_name: Option<&str> = match output_spec {
                                NameSpec::DerivedFrom(input_name) => {
                                    let param_path = format!("{}.{}", step_name, input_name);
                                    compile_store
                                        .get(&param_path)
                                        .ok()
                                        .and_then(|e| e.get_value().ok())
                                        .and_then(|v| v.as_text().ok())
                                }
                                NameSpec::DerivedWithDefault {
                                    input_name: derived_from,
                                    default,
                                } => {
                                    let param_path = format!("{}.{}", step_name, derived_from);
                                    let from_input = compile_store
                                        .get(&param_path)
                                        .ok()
                                        .and_then(|e| e.get_value().ok())
                                        .and_then(|v| v.as_text().ok());
                                    // Fall back to the literal default, not another store lookup
                                    Some(from_input.unwrap_or(default))
                                }
                                _ => unreachable!(),
                            };

                            if let Some(derived_name) = derived_name {
                                let output_key = match scope {
                                    OutputScope::Operation => {
                                        format!("{}.{}", step_name, derived_name)
                                    }
                                    OutputScope::Global => derived_name.to_string(),
                                };
                                compile_store.insert_or_replace(output_key, placeholder());
                            }
                        }
                    }
                }
            }
            ExecutionNode::IterArray {
                name,
                source,
                index_binding,
                item_binding,
                body,
            } => {
                // Validate source reference exists
                if compile_store.get(&source.reference).is_err() {
                    return Err(DraftError::UnresolvedReference {
                        step: name.clone(),
                        reference: source.reference.clone(),
                    });
                }

                // Validate body in an isolated scope (clone + bindings)
                let mut body_store = compile_store.clone();
                body_store.insert_or_replace(index_binding.clone(), placeholder());
                body_store.insert_or_replace(item_binding.clone(), placeholder());
                validate_nodes(body, &mut body_store, parameters, registry)?;
            }
            ExecutionNode::IterMap {
                name,
                source,
                key_binding,
                value_binding,
                body,
            } => {
                if compile_store.get(&source.reference).is_err() {
                    return Err(DraftError::UnresolvedReference {
                        step: name.clone(),
                        reference: source.reference.clone(),
                    });
                }

                let mut body_store = compile_store.clone();
                body_store.insert_or_replace(key_binding.clone(), placeholder());
                body_store.insert_or_replace(value_binding.clone(), placeholder());
                validate_nodes(body, &mut body_store, parameters, registry)?;
            }
            ExecutionNode::Guard {
                name,
                source,
                body,
            } => {
                // Validate boolean reference exists
                if compile_store.get(&source.reference).is_err() {
                    return Err(DraftError::UnresolvedReference {
                        step: name.clone(),
                        reference: source.reference.clone(),
                    });
                }

                // Validate body in same scope (not isolated like iteration)
                // Body outputs register directly in compile_store for downstream use
                validate_nodes(body, compile_store, parameters, registry)?;
            }
        }
    }
    Ok(())
}

pub(crate) struct DependencyTracker;

impl DependencyTracker {
    pub(crate) fn validate(
        variables: &Store<StoreEntry>,
        execution_order: &[ExecutionNode],
        parameters: &Store<Parameters>,
        returns: &Store<Parameters>,
        registry: &Registry,
    ) -> Result<(), DraftError> {
        let mut compile_store = variables.clone();

        validate_nodes(execution_order, &mut compile_store, parameters, registry)?;

        // Validate returns references
        for return_name in returns.keys() {
            let params = returns.get(return_name).expect("iterated from keys()");

            for reference in params.values().flat_map(|p| p.get_references()) {
                if compile_store.get(&reference).is_err() {
                    return Err(DraftError::UnresolvedReturn {
                        returns: return_name.clone(),
                        reference,
                    });
                }
            }
        }

        Ok(())
    }
}
