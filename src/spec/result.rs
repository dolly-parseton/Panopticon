use crate::imports::*;

/*
    Types:
    * ResultSpec - Enum representing the specification of a result (fixed or derived)
    * ResultKind - Enum representing the kind of result (Data or Meta)
*/

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum ResultSpec<T: Into<String>> {
    Field {
        name: T,
        ty: TypeDef<T>,
        hint: Option<T>,
        kind: ResultKind,
    },
    DerivedFromSingleAttribute {
        attribute: T,                   // The name of the AttributeSpec this result is derived from
        name_field: LiteralFieldRef<T>, // Compile-time proof that the source field is literal
        ty: Option<TypeDef<T>>,         // None = inferred from runtime value
        kind: ResultKind,
    },
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum ResultKind {
    Data,
    Meta,
}

impl From<ResultSpec<&'static str>> for ResultSpec<String> {
    fn from(spec: ResultSpec<&'static str>) -> Self {
        match spec {
            ResultSpec::Field {
                name,
                ty,
                hint,
                kind,
            } => ResultSpec::Field {
                name: name.into(),
                ty: ty.into(),
                hint: hint.map(|h| h.into()),
                kind,
            },
            ResultSpec::DerivedFromSingleAttribute {
                attribute,
                name_field,
                ty,
                kind,
            } => ResultSpec::DerivedFromSingleAttribute {
                attribute: attribute.into(),
                name_field: name_field.into(),
                ty: ty.map(|t| t.into()),
                kind,
            },
        }
    }
}
