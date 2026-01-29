use crate::imports::*;

/*
    Types:
    * Results - A map of result names to ResultValue
    * ResultValue - Enum representing either a ScalarValue or TabularValue
    * ResultSpec - Struct representing the specification of a result
*/
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct ResultSpec<T: Into<String>> {
    pub name: T,
    pub ty: TypeDef<T>,
    pub hint: Option<T>,
}

impl From<ResultSpec<&'static str>> for ResultSpec<String> {
    fn from(attr: ResultSpec<&'static str>) -> Self {
        ResultSpec {
            name: attr.name.into(),
            ty: attr.ty.into(),
            hint: attr.hint.map(|h| h.into()),
        }
    }
}
