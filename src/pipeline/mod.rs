use crate::imports::*;

pub mod order;
pub mod traits;
pub mod validation;

use order::{ExecutionGroup, ExecutionPlan};

#[derive(Default)]
pub struct Pipeline {
    pub namespaces: Vec<Namespace>,
    pub commands: Vec<CommandSpec>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_namespace(&mut self, namespace: Namespace) -> Result<NamespaceHandle<'_>> {
        let debug_data = (
            namespace.name().to_string(),
            match namespace.ty() {
                ExecutionMode::Once => "Once",
                ExecutionMode::Iterative { .. } => "Iterative",
                ExecutionMode::Static { .. } => "Static",
            },
        );
        // Check if namespace name already exists
        for ns in self.namespaces.iter() {
            if ns.name() == namespace.name() {
                return Err(anyhow::anyhow!(
                    "Namespace with name '{}' already exists",
                    namespace.name()
                ));
            }
        }

        // Check if namespace is reserved
        // It's possible to create an invalid namespace using the Namespace::new() function
        if RESERVED_NAMESPACES.contains(&namespace.name()) {
            return Err(anyhow::anyhow!(
                "Namespace name '{}' is reserved",
                namespace.name()
            ));
        }

        self.namespaces.push(namespace);
        tracing::debug!(
            namespace = debug_data.0,
            ty = debug_data.1,
            "Added namespace to Commands"
        );
        let index = self.namespaces.len() - 1;
        Ok(NamespaceHandle {
            commands: self,
            namespace_index: index,
        })
    }

    fn add_command<T>(&mut self, namespace: usize, name: &str, attrs: &Attributes) -> Result<()>
    where
        T: Command,
    {
        let debug_data = (namespace, name.to_string(), T::command_type());

        // Check if step name already exists in this namespace
        for cmd in self.commands.iter() {
            if cmd.namespace_index == namespace && cmd.name == name {
                return Err(anyhow::anyhow!(
                    "Command with name '{}' already exists in namespace index {}",
                    name,
                    namespace
                ));
            }
        }

        self.commands.push(CommandSpec::new::<T>(
            namespace,
            name.to_string(),
            attrs.clone(),
        ));

        tracing::debug!(
            namespace = ?debug_data.0,
            command_name = ?debug_data.1,
            command_type = ?debug_data.2,
            "Added command to Commands"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn execute(&self) -> Result<ExecutionContext> {
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
                    self.execute_commands(&commands, namespace.name(), &context)
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
                            context
                                .scalar()
                                .insert(&StorePath::from_segments([var_name]), item.clone())
                                .await?;
                        }
                        if let Some(index_name) = index_var {
                            context
                                .scalar()
                                .insert(
                                    &StorePath::from_segments([index_name]),
                                    to_scalar::i64(index as i64),
                                )
                                .await?;
                        }
                        self.execute_commands(&commands, namespace.name(), &context)
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

        Ok(context)
    }

    #[tracing::instrument(skip(self, commands, context), fields(namespace, command_count = commands.len()))]
    async fn execute_commands(
        &self,
        commands: &[&CommandSpec],
        namespace: &str,
        context: &ExecutionContext,
    ) -> Result<()> {
        for command_spec in commands {
            tracing::debug!(
                command_name = %command_spec.name,
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
            // Create output prefix as [namespace, command_name]
            let output_prefix = StorePath::from_segments([namespace, &command_spec.name]);
            command.execute(context, &output_prefix).await?;
        }
        Ok(())
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

pub struct NamespaceHandle<'a> {
    commands: &'a mut Pipeline,
    namespace_index: usize,
}

impl<'a> NamespaceHandle<'a> {
    fn get_namespace_ty(&self) -> &ExecutionMode {
        self.commands.namespaces[self.namespace_index].ty()
    }

    pub fn add_command<T>(&mut self, name: &str, attrs: &Attributes) -> Result<()>
    where
        T: Command,
    {
        // Only allow if namespace_ty is not static
        if let ExecutionMode::Static { .. } = self.get_namespace_ty() {
            return Err(anyhow::anyhow!(
                "Cannot add command to namespace of type Static"
            ));
        }
        self.commands
            .add_command::<T>(self.namespace_index, name, attrs)
    }
}
