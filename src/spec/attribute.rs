use crate::imports::*;

/*
    Types:
    * Attributes - A map of attribute names to ScalarValue
    * AttributeSpec - Struct representing the specification of an attribute
*/
pub type Attributes = std::collections::HashMap<String, ScalarValue>;

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct AttributeSpec<T: Into<String>> {
    pub name: T,
    pub ty: TypeDef<T>,
    pub required: bool,
    pub hint: Option<T>,
    pub default_value: Option<ScalarValue>,
    pub reference_kind: super::ReferenceKind, // Only valid/evaluated when TypeDef is Scalar or Tabular
}

impl From<AttributeSpec<&'static str>> for AttributeSpec<String> {
    fn from(attr: AttributeSpec<&'static str>) -> Self {
        AttributeSpec {
            name: attr.name.into(),
            ty: attr.ty.into(),
            required: attr.required,
            hint: attr.hint.map(|h| h.into()),
            default_value: attr.default_value,
            reference_kind: attr.reference_kind,
        }
    }
}
