use super::{Draft, Ready};
use crate::imports::*;
use crate::namespace::sealed::Build;

impl Pipeline<Draft> {
    pub fn new() -> Self {
        Pipeline {
            namespaces: Vec::new(),
            commands: Vec::new(),
            state: Draft,
        }
    }

    pub fn add_namespace<T>(
        &mut self,
        namespace: NamespaceBuilder<T>,
    ) -> Result<NamespaceHandle<'_, T>>
    where
        NamespaceBuilder<T>: Build,
    {
        let marker = std::marker::PhantomData::<T>;
        let namespace = namespace.build()?;
        let (name, ty) = (
            namespace.name().to_string(),
            match namespace.ty() {
                ExecutionMode::Once => "Once",
                ExecutionMode::Iterative { .. } => "Iterative",
                ExecutionMode::Static { .. } => "Static",
            },
        );
        tracing::debug!(
            namespace_name = %name,
            namespace_type = %ty,
            "Adding namespace to Pipeline"
        );

        // Check if namespace name already exists
        for ns in self.namespaces.iter() {
            if ns.name() == namespace.name() {
                tracing::warn!(
                    namespace_name = %name,
                    namespace_type = %ty,
                    "Duplicate namespace name",
                );
                return Err(anyhow::anyhow!(
                    "Namespace with name '{}' already exists",
                    namespace.name()
                ));
            }
        }
        // Check if namespace is reserved
        if RESERVED_NAMESPACES.contains(&namespace.name()) {
            tracing::warn!(
                namespace_name = %name,
                namespace_type = %ty,
                "Reserved namespace name",
            );
            return Err(anyhow::anyhow!(
                "Namespace name '{}' is reserved",
                namespace.name()
            ));
        }
        self.namespaces.push(namespace);
        let index = self.namespaces.len() - 1;
        tracing::debug!(
            namespace_name = %name,
            namespace_type = %ty,
            namespace_index = %index,
            "Namespace added to Pipeline"
        );
        Ok(NamespaceHandle {
            commands: self,
            namespace_index: index,
            _marker: marker,
        })
    }

    // Used by the namespace handle to add commands - hence pub(crate)
    pub(crate) fn add_command<T>(
        &mut self,
        namespace: usize,
        name: &str,
        attrs: &Attributes,
    ) -> Result<()>
    where
        T: Command,
    {
        let (ns_name, cmd_name, cmd_type) = (
            self.namespaces[namespace].name(),
            name.to_string(),
            T::command_type(),
        );
        tracing::debug!(
            namespace = %ns_name,
            command_name = %cmd_name,
            command_type = %cmd_type,
            "Adding command to Commands"
        );

        // Check if step name already exists in this namespace
        for cmd in self.commands.iter() {
            if cmd.namespace_index == namespace && cmd.name == name {
                tracing::warn!(
                    namespace = %ns_name,
                    command_name = %cmd_name,
                    command_type = %cmd_type,
                    "Duplicate command name in namespace",
                );
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
        )?);

        tracing::debug!(
            namespace = %ns_name,
            command_name = %cmd_name,
            command_type = %cmd_type,
            "Command added to Pipeline"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self), err, fields(command_count = self.commands.len(), namespace_count = self.namespaces.len()))]
    pub fn compile(self) -> Result<Pipeline<Ready>> {
        // Consolidate as much validation here as possible
        // Some are pretty unlikely given how the API is designed but I'm usabilitymaxxing.

        // Namespace + Command name validation
        let mut namespace_names = HashSet::new();
        let mut command_names_per_namespace: HashMap<&str, HashSet<&str>> = HashMap::new();
        for (ns_name, cmd_name) in self.command_ns_pairs_iter() {
            // Check namespace names
            if !namespace_names.insert(ns_name) {
                tracing::warn!(
                    namespace_name = %ns_name,
                    "Duplicate namespace name found",
                );
                return Err(anyhow::anyhow!(
                    "Duplicate namespace name found during compilation: '{}'",
                    ns_name
                ));
            }

            // Check reserved namespaces haven't been used
            if RESERVED_NAMESPACES.contains(&ns_name) {
                tracing::warn!(
                    namespace_name = %ns_name,
                    "Reserved namespace name used",
                );
                return Err(anyhow::anyhow!(
                    "Reserved namespace name '{}' used during compilation",
                    ns_name
                ));
            }

            // Check command names within namespace
            let cmd_set = command_names_per_namespace.entry(ns_name).or_default();
            if !cmd_set.insert(cmd_name) {
                tracing::warn!(
                    namespace_name = %ns_name,
                    command_name = %cmd_name,
                    "Duplicate command name found in namespace",
                );
                return Err(anyhow::anyhow!(
                    "Duplicate command name '{}' found in namespace '{}' during compilation",
                    cmd_name,
                    ns_name
                ));
            }
        }

        // Namespace type validation
        for namespace in &self.namespaces {
            match &namespace.ty() {
                ExecutionMode::Once => {
                    // No specific validation for Once namespaces currently
                }
                ExecutionMode::Iterative { store_path, .. } => {
                    // Check that store_path and source have been set
                    if store_path.segments().is_empty() {
                        tracing::warn!(
                            namespace_name = %namespace.name(),
                            "Iterative namespace has empty store_path",
                        );
                        return Err(anyhow::anyhow!(
                            "Iterative namespace '{}' has an empty store_path",
                            namespace.name()
                        ));
                    }
                }
                ExecutionMode::Static { .. } => {
                    // No specific validation for Static namespaces currently
                }
            }
        }

        // Attribute checks - there's an arugment that this isn't really performant butttt measure twice cut once
        for command in &self.commands {
            command.validate_attributes()?;
        }

        // Execution plan validation
        if let Err(e) = ExecutionPlan::new(&self.namespaces, &self.commands) {
            tracing::warn!("Execution plan validation failed during compilation: {}", e);
            return Err(anyhow::anyhow!(
                "Execution plan validation failed during compilation: {}",
                e
            ));
        }

        /*
            TODO - here a handful of additional validation checks that I need to consider adding later:
            * Validation of types against the kind of Iterator ExecutionMode::Iterative is using
            * Validation of command dependencies against available outputs in the execution plan

            None of which are super easy but doable if I make a proper type for looking ahead at the specs by StorePath.
        */

        tracing::debug!("Pipeline compilation successful");
        Ok(Pipeline::<Ready> {
            namespaces: self.namespaces,
            commands: self.commands,
            state: Ready,
        })
    }
}
