use crate::imports::*;

/*
    Functions:
    (PUBLIC)
    * extract_dependencies_from_spec - Extracts StorePath dependencies from attributes based on attribute specifications
    (PRIVATE)
    * extract_from_value - Helper function to extract dependencies from a ScalarValue based on its TypeDef
    * extract_from_scalar - Helper function to extract dependencies from a ScalarValue based on ReferenceKind
*/
pub fn extract_dependencies_from_spec(
    attrs: &Attributes,
    specs: &[AttributeSpec<&'static str>],
) -> HashSet<StorePath> {
    let mut deps = HashSet::new();

    for spec in specs {
        if let Some(value) = attrs.get(spec.name) {
            extract_from_value(value, &spec.ty, spec.reference_kind.clone(), &mut deps);
        }
    }

    deps
}

fn extract_from_value(
    value: &ScalarValue,
    ty: &TypeDef<&'static str>,
    reference_kind: ReferenceKind,
    deps: &mut HashSet<StorePath>,
) {
    match ty {
        TypeDef::Scalar(_) | TypeDef::Tabular => {
            extract_from_scalar(value, &reference_kind, deps);
        }
        TypeDef::ArrayOf(inner_ty) => {
            if let Some(arr) = value.as_array() {
                for item in arr {
                    extract_from_value(item, inner_ty, reference_kind.clone(), deps);
                }
            }
        }
        TypeDef::ObjectOf { fields } => {
            if let Some(obj) = value.as_object() {
                for field_spec in fields {
                    if let Some(field_value) = obj.get(field_spec.name) {
                        extract_from_value(
                            field_value,
                            &field_spec.ty,
                            field_spec.reference_kind.clone(),
                            deps,
                        );
                    }
                }
            }
        }
    }
}

fn extract_from_scalar(
    value: &ScalarValue,
    reference_kind: &ReferenceKind,
    deps: &mut HashSet<StorePath>,
) {
    use crate::dependencies::parser;
    match reference_kind {
        ReferenceKind::StaticTeraTemplate => {
            if let Some(s) = value.as_str() {
                parser::parse_template_dependencies(s, deps);
            }
        }
        ReferenceKind::RuntimeTeraTemplate => {
            if let Some(s) = value.as_str() {
                let template = format!("{{{{ {} }}}}", s);
                parser::parse_template_dependencies(&template, deps);
            }
        }
        ReferenceKind::StorePath => {
            if let Some(s) = value.as_str() {
                deps.insert(StorePath::from_dotted(s));
            }
        }
        ReferenceKind::Unsupported => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_specs() -> Vec<AttributeSpec<&'static str>> {
        vec![
            AttributeSpec {
                name: "query",
                ty: TypeDef::Scalar(ScalarType::String),
                required: true,
                hint: None,
                default_value: None,
                reference_kind: ReferenceKind::StaticTeraTemplate,
            },
            AttributeSpec {
                name: "condition",
                ty: TypeDef::Scalar(ScalarType::String),
                required: false,
                hint: None,
                default_value: None,
                reference_kind: ReferenceKind::RuntimeTeraTemplate,
            },
            AttributeSpec {
                name: "source",
                ty: TypeDef::Scalar(ScalarType::String),
                required: false,
                hint: None,
                default_value: None,
                reference_kind: ReferenceKind::StorePath,
            },
            AttributeSpec {
                name: "label",
                ty: TypeDef::Scalar(ScalarType::String),
                required: false,
                hint: None,
                default_value: None,
                reference_kind: ReferenceKind::Unsupported,
            },
        ]
    }

    #[test]
    fn test_static_tera_template() {
        let specs = make_specs();
        let attrs = ObjectBuilder::new()
            .insert("query", "SELECT * FROM {{ inputs.table }}")
            .build_hashmap();

        let deps = extract_dependencies_from_spec(&attrs, &specs);

        assert_eq!(deps.len(), 1);
        assert!(deps.contains(&StorePath::from_dotted("inputs.table")));
    }

    #[test]
    fn test_runtime_tera_template() {
        let specs = make_specs();
        let attrs = ObjectBuilder::new()
            .insert("condition", "inputs.count > 10")
            .build_hashmap();

        let deps = extract_dependencies_from_spec(&attrs, &specs);

        assert_eq!(deps.len(), 1);
        assert!(deps.contains(&StorePath::from_dotted("inputs.count")));
    }

    #[test]
    fn test_store_path_reference() {
        let specs = make_specs();
        let attrs = ObjectBuilder::new()
            .insert("source", "loader.users.data")
            .build_hashmap();

        let deps = extract_dependencies_from_spec(&attrs, &specs);

        assert_eq!(deps.len(), 1);
        assert!(deps.contains(&StorePath::from_dotted("loader.users.data")));
    }

    #[test]
    fn test_unsupported_extracts_nothing() {
        let specs = make_specs();
        let attrs = ObjectBuilder::new()
            .insert("label", "{{ this.should.not.extract }}")
            .build_hashmap();

        let deps = extract_dependencies_from_spec(&attrs, &specs);

        assert!(deps.is_empty());
    }

    #[test]
    fn test_multiple_attributes() {
        let specs = make_specs();
        let attrs = ObjectBuilder::new()
            .insert("query", "SELECT {{ inputs.col }} FROM {{ inputs.table }}")
            .insert("condition", "inputs.enabled == true")
            .insert("source", "loader.data")
            .insert("label", "ignored")
            .build_hashmap();

        let deps = extract_dependencies_from_spec(&attrs, &specs);

        assert_eq!(deps.len(), 4);
        assert!(deps.contains(&StorePath::from_dotted("inputs.col")));
        assert!(deps.contains(&StorePath::from_dotted("inputs.table")));
        assert!(deps.contains(&StorePath::from_dotted("inputs.enabled")));
        assert!(deps.contains(&StorePath::from_dotted("loader.data")));
    }

    #[test]
    fn test_nested_object_fields() {
        let specs = vec![AttributeSpec {
            name: "branches",
            ty: TypeDef::ArrayOf(Box::new(TypeDef::ObjectOf {
                fields: vec![
                    FieldSpec {
                        name: "if",
                        ty: TypeDef::Scalar(ScalarType::String),
                        required: true,
                        hint: None,
                        reference_kind: ReferenceKind::RuntimeTeraTemplate,
                    },
                    FieldSpec {
                        name: "then",
                        ty: TypeDef::Scalar(ScalarType::String),
                        required: true,
                        hint: None,
                        reference_kind: ReferenceKind::StaticTeraTemplate,
                    },
                ],
            })),
            required: true,
            hint: None,
            default_value: None,
            reference_kind: ReferenceKind::Unsupported,
        }];

        let branch1 = ObjectBuilder::new()
            .insert("if", "inputs.status == 'active'")
            .insert("then", "User {{ inputs.name }} is active")
            .build_scalar();

        let branch2 = ObjectBuilder::new()
            .insert("if", "inputs.status == 'pending'")
            .insert("then", "Pending: {{ inputs.id }}")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("branches", ScalarValue::Array(vec![branch1, branch2]))
            .build_hashmap();

        let deps = extract_dependencies_from_spec(&attrs, &specs);

        assert!(deps.contains(&StorePath::from_dotted("inputs.status")));
        assert!(deps.contains(&StorePath::from_dotted("inputs.name")));
        assert!(deps.contains(&StorePath::from_dotted("inputs.id")));
    }
}
