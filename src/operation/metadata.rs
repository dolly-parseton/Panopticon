use crate::imports::*;

#[derive(Debug, Clone)]
pub struct OperationMetadata {
    pub name: &'static str,
    pub description: &'static str,
    pub inputs: &'static [InputSpec],
    pub outputs: &'static [OutputSpec],
    pub requires_extensions: &'static [ExtensionSpec],
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputSpec {
    pub name: &'static str,
    pub ty: Type,
    pub required: bool,
    pub default: Option<Value>,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputSpec {
    pub name: NameSpec,
    pub ty: Type,
    pub description: &'static str,
    pub scope: OutputScope,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NameSpec {
    Static(&'static str),
    DerivedFrom(&'static str),
    DerivedWithDefault {
        input_name: &'static str,
        default: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputScope {
    Global,
    Operation,
}

#[derive(Debug, Clone)]
pub struct ExtensionSpec {
    pub name: NameSpec,
    pub description: &'static str,
    pub type_id: fn() -> TypeId,
}
