/*
    A registry store that holds factories for the operations.
*/

use crate::imports::*;

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
        // TODO - Add in 'when' condition checking here, using metadata and context extensions.
        (self.factory)(context)
    }
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

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
            // Input validation
            if let NameSpec::DerivedFrom(input_name)
            | NameSpec::DerivedWithDefault { input_name, .. } = &output.name
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
            if let NameSpec::DerivedWithDefault { input_name, .. } = &output.name {
                // Input should be optional
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
