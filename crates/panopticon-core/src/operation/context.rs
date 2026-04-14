use crate::imports::*;

/// The runtime handle passed to [`Operation::execute`].
///
/// A `Context` is a per-step view of the pipeline: it borrows the
/// operation's [`OperationMetadata`], the runtime [`Store`], the
/// registered [`Extensions`](crate::extend::Extension), and a scoped
/// prefix for looking up parameters. Operations use it to read
/// declared inputs, fetch extensions, emit errors, and record outputs.
/// Outputs are staged in two internal stores (operation-scoped and
/// global) and merged back into the runtime store after the step
/// finishes.
///
/// A `Context` validates every access against its metadata — calling
/// [`input`](Self::input), [`set_static_output`](Self::set_static_output),
/// [`set_derived_output`](Self::set_derived_output), or
/// [`extension`](Self::extension) with a name the operation did not
/// declare returns an [`OperationError`] rather than silently
/// succeeding.
pub struct Context<'a> {
    metadata: OperationMetadata,
    param_prefix: &'a str,
    store: &'a Store<StoreEntry>,
    operation_outputs: Store<StoreEntry>,
    global_outputs: Store<StoreEntry>,
    extensions: &'a Extensions,
}

impl Context<'_> {
    /// Constructs a fresh `Context` with empty staged-output stores.
    /// Called by the runtime immediately before a step executes; not
    /// typically invoked from operation code.
    pub fn new<'a>(
        metadata: OperationMetadata,
        param_prefix: &'a str,
        store: &'a Store<StoreEntry>,
        extensions: &'a Extensions,
    ) -> Context<'a> {
        Context {
            metadata,
            param_prefix,
            store,
            operation_outputs: Store::<StoreEntry>::new(),
            global_outputs: Store::<StoreEntry>::new(),
            extensions,
        }
    }

    /// Looks up a declared input by name, applying the step's parameter
    /// prefix automatically. Fails with
    /// [`OperationError::UndeclaredInput`] if the name is not in the
    /// operation's metadata.
    pub fn input(&self, name: &str) -> Result<&StoreEntry, OperationError> {
        let _spec = self
            .metadata
            .inputs
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| OperationError::UndeclaredInput {
                operation: self.metadata.name,
                input: name.to_string(),
            })?;

        let path = format!("{}.{}", self.param_prefix, name);
        Ok(self.store.get(&path)?)
    }

    /// Records a statically-named output declared on the operation's
    /// metadata as [`NameSpec::Static`]. Fails with
    /// [`OperationError::UndeclaredOutput`] if no matching output is
    /// declared.
    pub fn set_static_output(
        &mut self,
        name: &'static str,
        entry: impl Into<StoreEntry>,
    ) -> Result<(), OperationError> {
        let spec = self
            .metadata
            .outputs
            .iter()
            .find(|s| matches!(&s.name, NameSpec::Static(n) if *n == name))
            .ok_or_else(|| OperationError::UndeclaredOutput {
                operation: self.metadata.name,
                output: name.to_string(),
            })?;

        self.insert_by_scope(&spec.scope, name.to_string(), entry.into())
    }

    /// Records an output whose name is derived from a text input,
    /// declared on the operation's metadata as [`NameSpec::DerivedFrom`]
    /// or [`NameSpec::DerivedWithDefault`]. Fails with
    /// [`OperationError::UndeclaredDerivedOutput`] if no matching output
    /// is declared.
    pub fn set_derived_output(
        &mut self,
        input_name: &str,
        entry: impl Into<StoreEntry>,
    ) -> Result<(), OperationError> {
        let spec = self
            .metadata
            .outputs
            .iter()
            .find(|s| matches!(&s.name, NameSpec::DerivedFrom(n) | NameSpec::DerivedWithDefault { input_name: n, .. } if *n == input_name))
            .ok_or_else(|| OperationError::UndeclaredDerivedOutput {
                operation: self.metadata.name,
                input: input_name.to_string(),
            })?;

        let output_key = match &spec.name {
            NameSpec::DerivedFrom(input_name) => {
                let path = format!("{}.{}", self.param_prefix, input_name);
                self.store.get(&path)?.get_value()?.as_text()?.to_string()
            }
            NameSpec::DerivedWithDefault {
                input_name,
                default,
            } => {
                let path = format!("{}.{}", self.param_prefix, input_name);
                match self.store.get(&path) {
                    Ok(entry) => entry.get_value()?.as_text()?.to_string(),
                    Err(_) => default.to_string(),
                }
            }
            NameSpec::Static(_) => unreachable!(),
        };

        self.insert_by_scope(&spec.scope, output_key, entry.into())
    }

    fn insert_by_scope(
        &mut self,
        scope: &OutputScope,
        name: String,
        entry: StoreEntry,
    ) -> Result<(), OperationError> {
        let store = match scope {
            OutputScope::Operation => &mut self.operation_outputs,
            OutputScope::Global => &mut self.global_outputs,
        };
        Ok(store.insert(name, entry)?)
    }

    /// Looks up a registered [`Extension`] of type `T` under a
    /// declared extension spec. Fails with
    /// [`OperationError::ExtensionNotFound`] if the spec is undeclared
    /// or no extension was registered under the resolved name.
    pub fn extension<T: Extension>(&self, spec_name: &str) -> Result<&T, OperationError> {
        let spec = self
            .metadata
            .requires_extensions
            .iter()
            .find(|s| match &s.name {
                NameSpec::Static(n) => *n == spec_name,
                NameSpec::DerivedFrom(n) => *n == spec_name,
                NameSpec::DerivedWithDefault { input_name, .. } => *input_name == spec_name,
            })
            .ok_or_else(|| OperationError::ExtensionNotFound {
                operation: self.metadata.name.to_string(),
                extension: spec_name.to_string(),
            })?;

        let resolved_name = match &spec.name {
            NameSpec::Static(name) => name.to_string(),
            NameSpec::DerivedFrom(input_name) => {
                let path = format!("{}.{}", self.param_prefix, input_name);
                self.store.get(&path)?.get_value()?.as_text()?.to_string()
            }
            NameSpec::DerivedWithDefault {
                input_name,
                default,
            } => {
                let path = format!("{}.{}", self.param_prefix, input_name);
                match self.store.get(&path) {
                    Ok(entry) => entry.get_value()?.as_text()?.to_string(),
                    Err(_) => default.to_string(),
                }
            }
        };

        self.extensions
            .get::<T>(&resolved_name)
            .ok_or_else(|| OperationError::ExtensionNotFound {
                operation: self.metadata.name.to_string(),
                extension: resolved_name,
            })
    }

    /// Builds an [`OperationError::Custom`] tagged with the operation's
    /// name for ad-hoc failure reporting. Prefer the
    /// [`op_error!`](crate::op_error) macro for `format!`-style
    /// construction.
    pub fn error(&self, message: impl Into<String>) -> OperationError {
        OperationError::Custom {
            operation: self.metadata.name.into(),
            message: message.into(),
        }
    }

    pub(crate) fn consume(self) -> (Store<StoreEntry>, Store<StoreEntry>) {
        (self.operation_outputs, self.global_outputs)
    }
}
