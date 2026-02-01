use crate::imports::*;

use super::{DEFAULT_NAME_POLICY, LiteralFieldRef, ObjectFields, extract_object_fields};

// Uses some generics magic to compile time safety for certain spec edge cases.

/*
    Types:
    * CommandSpecBuilder - Builder for constructing (attributes, results) pairs with validation
    * PendingAttribute - Intermediate state while an ArrayOf(ObjectOf) attribute's fields are being built
    * AttributeSpecBuilder - Builder for constructing AttributeSpec
*/

pub struct CommandSpecBuilder<T: Into<String>> {
    attributes: Vec<AttributeSpec<T>>,
    results: Vec<ResultSpec<T>>,
}

impl<T: Into<String>> Default for CommandSpecBuilder<T> {
    fn default() -> Self {
        Self {
            attributes: vec![],
            results: vec![],
        }
    }
}

impl<T: Into<String> + Clone + PartialEq + std::fmt::Debug> CommandSpecBuilder<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attribute(mut self, spec: AttributeSpec<T>) -> Self {
        self.attributes.push(spec);
        self
    }

    pub fn array_of_objects(
        self,
        name: T,
        required: bool,
        hint: Option<T>,
    ) -> (PendingAttribute<T>, ObjectFields<T>) {
        (
            PendingAttribute {
                inner: self,
                name,
                required,
                hint,
            },
            ObjectFields::new(),
        )
    }

    pub fn fixed_result(
        mut self,
        name: T,
        ty: TypeDef<T>,
        hint: Option<T>,
        kind: ResultKind,
    ) -> Self {
        self.results.push(ResultSpec::Field {
            name,
            ty,
            hint,
            kind,
        });
        self
    }

    pub fn derived_result(
        mut self,
        attribute: T,
        name_field: LiteralFieldRef<T>,
        ty: Option<TypeDef<T>>,
        kind: ResultKind,
    ) -> Self {
        self.results.push(ResultSpec::DerivedFromSingleAttribute {
            attribute,
            name_field,
            ty,
            kind,
        });
        self
    }

    pub fn build(self) -> (Vec<AttributeSpec<T>>, Vec<ResultSpec<T>>) {
        for result in &self.results {
            if let ResultSpec::DerivedFromSingleAttribute {
                attribute,
                name_field,
                ..
            } = result
            {
                let attr = self
                    .attributes
                    .iter()
                    .find(|a| &a.name == attribute)
                    .unwrap_or_else(|| {
                        panic!(
                            "Derived result references unknown attribute '{:?}'",
                            attribute
                        )
                    });

                let fields = extract_object_fields(&attr.ty).unwrap_or_else(|| {
                    panic!(
                        "Derived result attribute '{:?}' must be ArrayOf(ObjectOf)",
                        attribute
                    )
                });

                let field_name = name_field.name();
                assert!(
                    fields.iter().any(|f| &f.name == field_name),
                    "Derived result name_field '{:?}' not found in attribute '{:?}' fields",
                    field_name,
                    attribute,
                );
            }
        }

        let policy = &*DEFAULT_NAME_POLICY;

        for attr in &self.attributes {
            policy.validate(attr.name.clone(), "attribute");
        }

        for result in &self.results {
            match result {
                ResultSpec::Field { name, .. } => {
                    policy.validate(name.clone(), "result");
                }
                ResultSpec::DerivedFromSingleAttribute { .. } => {
                    // Derived result names come from runtime data, not spec-defined names
                }
            }
        }

        (self.attributes, self.results)
    }
}

pub struct PendingAttribute<T: Into<String>> {
    inner: CommandSpecBuilder<T>,
    name: T,
    required: bool,
    hint: Option<T>,
}

impl<T: Into<String> + Clone + PartialEq + std::fmt::Debug> PendingAttribute<T> {
    pub fn finalise_attribute(self, fields: ObjectFields<T>) -> CommandSpecBuilder<T> {
        let mut builder = self.inner;
        builder.attributes.push(AttributeSpec {
            name: self.name,
            ty: TypeDef::ArrayOf(Box::new(TypeDef::ObjectOf {
                fields: fields.build(),
            })),
            required: self.required,
            hint: self.hint,
            default_value: None,
            reference_kind: ReferenceKind::Unsupported,
        });
        builder
    }
}

pub struct AttributeSpecBuilder<T: Into<String>> {
    name: T,
    ty: TypeDef<T>,
    required: bool,
    hint: Option<T>,
    default_value: Option<ScalarValue>,
    reference_kind: ReferenceKind,
}

impl<T: Into<String>> AttributeSpecBuilder<T> {
    pub fn new(name: T, ty: TypeDef<T>) -> Self {
        Self {
            name,
            ty,
            required: false,
            hint: None,
            default_value: None,
            reference_kind: ReferenceKind::Unsupported,
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn hint(mut self, hint: T) -> Self {
        self.hint = Some(hint);
        self
    }

    pub fn default_value(mut self, value: ScalarValue) -> Self {
        self.default_value = Some(value);
        self
    }

    pub fn reference(mut self, kind: ReferenceKind) -> Self {
        self.reference_kind = kind;
        self
    }

    pub fn build(self) -> AttributeSpec<T> {
        AttributeSpec {
            name: self.name,
            ty: self.ty,
            required: self.required,
            hint: self.hint,
            default_value: self.default_value,
            reference_kind: self.reference_kind,
        }
    }
}
