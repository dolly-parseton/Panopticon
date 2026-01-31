use crate::imports::*;

pub mod attribute;
pub mod builder;
pub mod command;
pub mod result;

/*
    Types:
    * TypeDef - Enum representing the type definition of a value (Scalar, Tabular, ArrayOf, ObjectOf)
    * FieldSpec - Struct representing the specification of a field in an object type
    * ReferenceKind - Enum indicating if a field supports references and how to evaluate them.
    * LiteralFieldRef - Opaque handle proving a field has ReferenceKind::Unsupported (compile-time safety)
    * ObjectFields - Builder for ObjectOf fields that enforces LiteralFieldRef safety
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

/// Extracts the inner FieldSpec vec from an ArrayOf(ObjectOf { fields }) TypeDef.
/// Returns None if the TypeDef is not ArrayOf(ObjectOf).
pub fn extract_object_fields<T: Into<String>>(ty: &TypeDef<T>) -> Option<&Vec<FieldSpec<T>>> {
    match ty {
        TypeDef::ArrayOf(inner) => match inner.as_ref() {
            TypeDef::ObjectOf { fields } => Some(fields),
            _ => None,
        },
        _ => None,
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

// ─── LiteralFieldRef ───
//
// Opaque proof that a field has ReferenceKind::Unsupported (literal value).
// Fields are PRIVATE — can only be constructed inside this module via ObjectFields::add_literal().
// This is the compile-time enforcement mechanism: ResultSpec::DerivedFromSingleAttribute
// requires a LiteralFieldRef, so derived result names can only come from literal fields.

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct LiteralFieldRef<T: Into<String>> {
    name: T,
}

impl<T: Into<String> + Clone> LiteralFieldRef<T> {
    pub fn name(&self) -> &T {
        &self.name
    }
}

impl From<LiteralFieldRef<&'static str>> for LiteralFieldRef<String> {
    fn from(r: LiteralFieldRef<&'static str>) -> Self {
        LiteralFieldRef {
            name: r.name.into(),
        }
    }
}

// ─── ObjectFields builder ───
//
// The only way to obtain a LiteralFieldRef is through this builder.
// add_literal() returns a LiteralFieldRef; add_template() does not.
// This prevents template fields from being used as derived result names.

pub struct ObjectFields<T: Into<String>> {
    fields: Vec<FieldSpec<T>>,
}

impl<T: Into<String>> Default for ObjectFields<T> {
    fn default() -> Self {
        Self { fields: vec![] }
    }
}

impl<T: Into<String> + Clone> ObjectFields<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a literal field (ReferenceKind::Unsupported).
    /// Returns Self AND a LiteralFieldRef — proof this field is safe for derived result names.
    pub fn add_literal(
        mut self,
        name: T,
        ty: TypeDef<T>,
        required: bool,
        hint: Option<T>,
    ) -> (Self, LiteralFieldRef<T>) {
        let handle = LiteralFieldRef { name: name.clone() };
        self.fields.push(FieldSpec {
            name,
            ty,
            required,
            hint,
            reference_kind: ReferenceKind::Unsupported,
        });
        (self, handle)
    }

    /// Add a template/reference field. No LiteralFieldRef returned —
    /// cannot be used as a derived result name source.
    pub fn add_template(
        mut self,
        name: T,
        ty: TypeDef<T>,
        required: bool,
        hint: Option<T>,
        kind: ReferenceKind,
    ) -> Self {
        self.fields.push(FieldSpec {
            name,
            ty,
            required,
            hint,
            reference_kind: kind,
        });
        self
    }

    pub fn build(self) -> Vec<FieldSpec<T>> {
        self.fields
    }
}
