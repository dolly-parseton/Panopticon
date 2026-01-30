use crate::imports::*;

/*
    Types:
    * ResultSpec - Struct representing the specification of a result
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
        attribute: T,  // The name of the AttributSpec this result is derived from
        name_field: T, // The field in FieldSpec that provides the name of this result
        ty: TypeDef<T>,
        kind: ResultKind,
    },
    // DervivedFromGroupAttribute {
    //     attribute: T,               // The name of the AttributSpec this result is derived from
    //     name_field: T,              // The field in FieldSpec that provides the name of this result
    //     fields: Vec<ResultSpec<T>>, // For derived nested results.
    //     kind: ResultKind,
    // },
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum ResultKind {
    Data,
    Meta,
}

impl From<ResultSpec<&'static str>> for ResultSpec<String> {
    fn from(attr: ResultSpec<&'static str>) -> Self {
        match attr {
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
                ty: ty.into(),
                kind,
            },
        }
    }
}
