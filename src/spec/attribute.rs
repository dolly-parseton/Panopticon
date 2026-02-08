use crate::imports::*;

/*
    Types:
    * Attributes - A map of attribute names to ScalarValue
    * AttributeSpec - Struct representing the specification of an attribute

    Macros:
    * attrs! - Creates an Attributes HashMap from key-value pairs
*/
pub type Attributes = std::collections::HashMap<String, ScalarValue>;

#[macro_export]
macro_rules! attrs {
    () => {
        std::collections::HashMap::<String, $crate::prelude::ScalarValue>::new()
    };
    ($($key:expr => $value:expr),+ $(,)?) => {{
        let mut map = std::collections::HashMap::<String, $crate::prelude::ScalarValue>::new();
        $(
            map.insert($key.into(), $value.into());
        )+
        map
    }};
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct AttributeSpec<T: Into<String>> {
    pub(crate) name: T,
    pub(crate) ty: TypeDef<T>,
    pub(crate) required: bool,
    pub(crate) hint: Option<T>,
    pub(crate) default_value: Option<ScalarValue>,
    pub(crate) reference_kind: super::ReferenceKind, // Only valid/evaluated when TypeDef is Scalar or Tabular
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
