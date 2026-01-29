use crate::imports::*;

use super::validation::validate_attributes;

/*
    Types:
    * CommandFactory - Factory function type for creating command instances from attributes
    * ExecutableWrapper - Wrapper around Executable trait objects to handle common attributes like 'when
*/
pub type CommandFactory = Box<dyn Fn(&Attributes) -> Result<Box<dyn Executable>>>;

struct ExecutableWrapper {
    inner: Box<dyn Executable>,
    when: Option<String>,
}

// Wrapper to handle 'when' conditional before executing the inner command
// ^ Again might extend later with more common functionality
#[async_trait::async_trait]
impl Executable for ExecutableWrapper {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        if let Some(condition) = &self.when {
            let template = format!("{{{{ {} }}}}", condition);
            let result = context.substitute(&template).await?;
            if !is_truthy(&parse_scalar(&result)) {
                tracing::debug!(condition = %condition, "Skipping command - 'when' condition is false");
                return Ok(());
            }
        }
        self.inner.execute(context, output_prefix).await
    }
}

/*
    Consts
    * COMMON_ATTRIBUTES - Common attributes shared by all commands
    ... Might extend later with more common attributes/functionality.

*/
pub const COMMON_ATTRIBUTES: &[AttributeSpec<&'static str>] = &[AttributeSpec {
    name: "when",
    ty: TypeDef::Scalar(ScalarType::String),
    required: false,
    hint: Some("Evaluates a tera conditional to determine if the command should run"),
    default_value: None,
    reference_kind: ReferenceKind::RuntimeTeraTemplate,
}];

/*
    Traits:
    * Command - Marker trait for commands implementing FromAttributes, Descriptor, and Executable
    * FromAttributes - Trait for constructing command instances from attributes
    * Descriptor - Trait for providing command metadata like type, attributes, and outputs
    * Executable - Async trait for executing commands within an execution context
*/

pub trait Command: FromAttributes + Descriptor + Executable {}
// Blanket implementation for any type that implements the required traits
impl<T: FromAttributes + Descriptor + Executable> Command for T {}

pub trait FromAttributes: Sized + Descriptor {
    fn from_attributes(attrs: &Attributes) -> Result<Self>;

    fn extract_dependencies(attrs: &Attributes) -> std::collections::HashSet<StorePath> {
        use crate::dependencies::helpers;
        // Todo change to use available attributes? Current approach wont consider common attributes
        helpers::extract_dependencies_from_spec(attrs, Self::command_attributes())
    }

    fn factory() -> CommandFactory
    where
        Self: Executable + Descriptor + 'static,
    {
        Box::new(|attrs| {
            validate_attributes(attrs, Self::available_attributes())?;

            let when = attrs.get("when").and_then(|v| v.as_str()).map(String::from);

            let instance = Self::from_attributes(attrs)?;
            let wrapped = ExecutableWrapper {
                inner: Box::new(instance),
                when,
            };
            Ok(Box::new(wrapped) as Box<dyn Executable>)
        })
    }
}

pub trait Descriptor: Sized {
    fn command_type() -> &'static str;
    fn command_attributes() -> &'static [AttributeSpec<&'static str>];
    fn expected_outputs() -> &'static [ResultSpec<&'static str>];
    // Defaults
    fn available_attributes() -> Vec<&'static AttributeSpec<&'static str>> {
        let mut attrs = Vec::new();
        attrs.extend(COMMON_ATTRIBUTES.iter());
        attrs.extend(Self::command_attributes().iter());
        attrs
    }
    fn required_attributes() -> Vec<&'static AttributeSpec<&'static str>> {
        Self::command_attributes()
            .iter()
            .filter(|attr| attr.required)
            .collect()
    }
    fn optional_attributes() -> Vec<&'static AttributeSpec<&'static str>> {
        Self::command_attributes()
            .iter()
            .filter(|attr| !attr.required)
            .collect()
    }
}

#[async_trait::async_trait]
pub trait Executable: Send + Sync + 'static {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()>;
}
