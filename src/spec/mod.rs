use crate::imports::*;

pub mod attribute;
pub mod command;
pub mod result;

/*
    Types:
    * TypeDef - Enum representing the type definition of a value (Scalar, Tabular, ArrayOf, ObjectOf)
    * FieldSpec - Struct representing the specification of a field in an object type
    * ReferenceKind - Enum indicating if a field supports references and how to evaluate them.
*/
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum TypeDef<T: Into<String>> {
    Scalar(ScalarType),
    Tabular,
    ArrayOf(Box<TypeDef<T>>),
    ObjectOf { fields: Vec<FieldSpec<T>> },
}

impl From<TypeDef<&'static str>> for TypeDef<String> {
    fn from(td: TypeDef<&'static str>) -> Self {
        match td {
            TypeDef::Scalar(s) => TypeDef::Scalar(s),
            TypeDef::Tabular => TypeDef::Tabular,
            TypeDef::ArrayOf(inner) => TypeDef::ArrayOf(Box::new((*inner).into())),
            TypeDef::ObjectOf { fields } => TypeDef::ObjectOf {
                fields: fields.into_iter().map(|f| f.into()).collect(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct FieldSpec<T: Into<String>> {
    pub name: T,
    pub ty: TypeDef<T>,
    pub required: bool,
    pub hint: Option<T>,
    pub reference_kind: ReferenceKind, // Only valid/evaluated when TypeDef is Scalar or Tabular
}

impl From<FieldSpec<&'static str>> for FieldSpec<String> {
    fn from(fs: FieldSpec<&'static str>) -> Self {
        FieldSpec {
            name: fs.name.into(),
            ty: fs.ty.into(),
            required: fs.required,
            hint: fs.hint.map(|h| h.into()),
            reference_kind: fs.reference_kind,
        }
    }
}

// Indicates if a field supports references and how, this is used by dependency checks
#[derive(Debug, Clone, PartialEq, Hash, Eq, Default)]
pub enum ReferenceKind {
    StaticTeraTemplate,  // The field can contain tera templates.
    RuntimeTeraTemplate, // The field is treated as a tera template at runtime.
    StorePath,           // The field is a direct store path reference. Tabular or Scalar
    #[default]
    Unsupported,
}
