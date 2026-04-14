use crate::imports::*;

/// Built-in operation that writes a global variable derived from its inputs.
///
/// **Inputs:**
/// - `name` (Text, required) — the variable name to set.
/// - `value` (Any, required) — the value to assign.
///
/// **Outputs:**
/// - A global output named after the `name` input holding the assigned
///   value, making it visible to every subsequent step and return block.
#[derive(Debug, Clone, Default)]
pub struct SetVar;

impl Operation for SetVar {
    fn metadata() -> OperationMetadata {
        OperationMetadata {
            name: "SetVar",
            description: "Sets a variable in the context's output store",
            inputs: &[
                InputSpec {
                    name: "name",
                    ty: Type::Text,
                    required: true,
                    default: None,
                    description: "Variable name to set",
                },
                InputSpec {
                    name: "value",
                    ty: Type::Any,
                    required: true,
                    default: None,
                    description: "Value to assign",
                },
            ],
            outputs: &[OutputSpec {
                name: NameSpec::DerivedFrom("name"),
                ty: Type::Any,
                description: "The variable that was set",
                scope: OutputScope::Global,
            }],
            requires_extensions: &[],
        }
    }

    fn execute(context: &mut Context) -> Result<(), OperationError> {
        let value = context.input("value")?.get_value()?.clone();
        context.set_derived_output("name", value)?;
        Ok(())
    }
}
