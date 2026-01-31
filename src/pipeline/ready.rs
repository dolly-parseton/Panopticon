use super::{Completed, Draft, Ready};
use crate::imports::*;

impl Pipeline<Ready> {
    #[tracing::instrument(skip(self))]
    pub async fn execute(self) -> Result<Pipeline<Completed>> {
        // Create a new execution context
        let context = ExecutionContext::new();
        // Add in all "values" from Namespaces of type Static
        for namespace in &self.namespaces {
            if let ExecutionMode::Static { values } = &namespace.ty() {
                for (key, value) in values {
                    {
                        let store_path = StorePath::from_segments([namespace.name(), key]);
                        context.scalar().insert(&store_path, value.clone()).await?;
                        tracing::debug!(
                            namespace = namespace.name(),
                            key = key.as_str(),
                            "Inserted static value into ExecutionContext scalar store"
                        );
                    }
                }
            }
        }

        tracing::debug!("Starting execution of Commands");

        let plan = ExecutionPlan::new(&self.namespaces, &self.commands)?;

        for group_result in plan {
            let ExecutionGroup {
                namespace,
                namespace_index,
                commands,
            } = group_result?;
            tracing::debug!(
                namespace_index = namespace_index,
                command_count = commands.len(),
                "Executing command group"
            );
            match &namespace.ty() {
                ExecutionMode::Once => {
                    self.execute_commands(&commands, namespace.name(), &context, None)
                        .await?;
                }
                ExecutionMode::Iterative {
                    store_path,
                    source: _,
                    iter_var,
                    index_var,
                } => {
                    tracing::debug!(
                        namespace = store_path
                            .namespace()
                            .unwrap_or(&"<no-namespace>".to_string())
                            .as_str(),
                        store_path = store_path.to_dotted(),
                        "Processing iterative namespace"
                    );
                    let iter_items: Vec<ScalarValue> =
                        namespace.ty().resolve_iter_values(&context).await?;
                    tracing::debug!(
                        iteration_count = iter_items.len(),
                        "Extracted items for iterative namespace"
                    );
                    for (index, item) in iter_items.iter().enumerate() {
                        // Add the item and index values to the context for substitution.
                        if let Some(var_name) = iter_var {
                            context.scalar().insert_raw(var_name, item.clone()).await?;
                        }
                        if let Some(index_name) = index_var {
                            context
                                .scalar()
                                .insert_raw(index_name, to_scalar::i64(index as i64))
                                .await?;
                        }
                        self.execute_commands(&commands, namespace.name(), &context, Some(index))
                            .await?;
                        // Remove the iteration variables from the context.
                        if let Some(var_name) = iter_var {
                            context
                                .scalar()
                                .remove(&StorePath::from_segments([var_name]))
                                .await?;
                        }
                        if let Some(_index_name) = index_var {
                            context
                                .scalar()
                                .remove(&StorePath::from_segments([_index_name]))
                                .await?;
                        }
                    }
                }
                ExecutionMode::Static { values: _ } => {
                    // Variables namespace does not execute commands.
                    tracing::debug!(
                        namespace = namespace.name(),
                        "Variables namespace - skipping command execution"
                    );
                }
            }
        }

        Ok(Pipeline::<Completed> {
            namespaces: self.namespaces,
            commands: self.commands,
            state: Completed { context },
        })
    }

    #[tracing::instrument(skip(self, commands, context), fields(namespace, command_count = commands.len()))]
    async fn execute_commands(
        &self,
        commands: &[&CommandSpec],
        namespace: &str,
        context: &ExecutionContext,
        iteration_index: Option<usize>,
    ) -> Result<()> {
        for command_spec in commands {
            tracing::debug!(
                command_name = %command_spec.name,
                iteration_index = ?iteration_index,
                "Executing command"
            );
            // Run substitution on all string attributes.
            tracing::debug!(
                command_name = %command_spec.name,
                "Substituting command attributes"
            );
            let substituted_attrs =
                substitute_attributes(&command_spec.attributes, context, &command_spec.name)
                    .await?;
            let command = (command_spec.builder)(&substituted_attrs)?;
            tracing::debug!(
                command_name = %command_spec.name,
                "Substituted command attributes and built command instance"
            );
            // Create output prefix as [namespace, command_name] or [namespace, command_name, index]
            let mut output_prefix = StorePath::from_segments([namespace, &command_spec.name]);
            if let Some(idx) = iteration_index {
                output_prefix = output_prefix.with_index(idx);
            }
            command.execute(context, &output_prefix).await?;
        }
        Ok(())
    }

    pub fn edit(self) -> Pipeline<Draft> {
        Pipeline::<Draft> {
            namespaces: self.namespaces,
            commands: self.commands,
            state: Draft,
        }
    }
}

async fn substitute_attributes(
    attrs: &Attributes,
    context: &ExecutionContext,
    command_name: &str,
) -> Result<Attributes> {
    let mut substituted = Attributes::new();
    for (key, value) in attrs.iter() {
        tracing::debug!(attribute_key = key.as_str(), "Substituting attribute value");
        let new_value = match value {
            ScalarValue::String(s) => {
                let rendered = context.substitute(s).await.with_context(|| {
                    format!(
                        "Failed to substitute attribute '{}' for command '{}'",
                        key, command_name
                    )
                })?;
                tracing::debug!(
                    attribute_key = key.as_str(),
                    "Substituted attribute value using template rendering"
                );
                ScalarValue::String(rendered)
            }
            _ => value.clone(),
        };
        substituted.insert(key.clone(), new_value);
    }
    Ok(substituted)
}
