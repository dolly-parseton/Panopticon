use crate::imports::*;

/// The draft-time collection of [`Operation`] types known to a
/// pipeline.
///
/// Every call to
/// [`Pipeline::step`](crate::prelude::Pipeline#method.step) registers
/// the operation type (idempotently), validates its
/// [`OperationMetadata`], and records a factory that the runtime uses
/// to dispatch execution. Child pipelines created by the `*_from_child`
/// methods merge their registries into the parent. End users do not
/// typically touch the registry directly — it is exposed primarily so
/// tooling that walks compiled pipelines can inspect what operations
/// are available.
#[derive(Default)]
pub struct Registry {
    entries: HashMap<std::any::TypeId, OperationEntry>,
}

pub(crate) struct OperationEntry {
    pub metadata: OperationMetadata,
    pub factory: fn(&mut Context) -> Result<(), OperationError>,
}

impl OperationEntry {
    pub fn execute(&self, context: &mut Context) -> Result<(), OperationError> {
        (self.factory)(context)
    }
}

impl Registry {
    /// Constructs an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an [`Operation`] type. Idempotent — subsequent calls
    /// for the same type are no-ops. Validates the returned
    /// [`OperationMetadata`] and fails with
    /// [`DraftError::InvalidMetadata`] if any declared output or
    /// extension uses a [`NameSpec::DerivedFrom`] that references a
    /// non-existent or wrongly-typed input.
    pub fn register<O: Operation + 'static>(&mut self) -> Result<(), DraftError> {
        let id = std::any::TypeId::of::<O>();
        if self.entries.contains_key(&id) {
            return Ok(());
        }
        let metadata = O::metadata();
        Self::validate_metadata(&metadata)?;
        self.entries.insert(
            id,
            OperationEntry {
                metadata,
                factory: |context| O::execute(context),
            },
        );
        Ok(())
    }

    fn validate_metadata(metadata: &OperationMetadata) -> Result<(), DraftError> {
        for output in metadata.outputs {
            Self::validate_derived_name(metadata, &output.name)?;
        }
        for ext in metadata.requires_extensions {
            Self::validate_derived_name(metadata, &ext.name)?;
        }
        Ok(())
    }

    fn validate_derived_name(
        metadata: &OperationMetadata,
        name: &NameSpec,
    ) -> Result<(), DraftError> {
        if let NameSpec::DerivedFrom(input_name)
        | NameSpec::DerivedWithDefault { input_name, .. } = name
        {
            let input = metadata.inputs.iter().find(|i| i.name == *input_name);
            match input {
                None => {
                    return Err(DraftError::InvalidMetadata {
                        operation: metadata.name,
                        reason: format!(
                            "DerivedFrom('{}') references a non-existent input",
                            input_name
                        ),
                    });
                }
                Some(spec) if spec.ty != Type::Text => {
                    return Err(DraftError::InvalidMetadata {
                        operation: metadata.name,
                        reason: format!(
                            "DerivedFrom('{}') requires input type Text, found {}",
                            input_name, spec.ty
                        ),
                    });
                }
                _ => {}
            }
        }
        if let NameSpec::DerivedWithDefault { input_name, .. } = name {
            let input = metadata.inputs.iter().find(|i| i.name == *input_name);
            if let Some(input) = input
                && input.required
            {
                return Err(DraftError::InvalidMetadata {
                    operation: metadata.name,
                    reason: format!(
                        "DerivedWithDefault('{}') requires an optional input, but '{}' is required",
                        input_name, input_name
                    ),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn get(&self, id: &std::any::TypeId) -> Option<&OperationEntry> {
        self.entries.get(id)
    }

    pub(crate) fn merge(&mut self, other: Registry) {
        for (id, entry) in other.entries {
            self.entries.entry(id).or_insert(entry);
        }
    }
}
