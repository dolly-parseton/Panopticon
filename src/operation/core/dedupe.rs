use crate::imports::*;

#[derive(Debug, Clone, Default)]
pub struct Dedupe;

impl Operation for Dedupe {
    fn metadata() -> OperationMetadata {
        OperationMetadata {
            name: "Dedupe",
            description: "Removes duplicate elements from an array.",
            inputs: &[
                InputSpec {
                    name: "array",
                    ty: Type::Array,
                    required: true,
                    default: None,
                    description: "An array to remove duplicate elements from",
                },
                InputSpec {
                    name: "name",
                    ty: Type::Text,
                    required: false,
                    default: None,
                    description: "Name for the output variable (defaults to 'result')",
                },
            ],
            outputs: &[OutputSpec {
                name: NameSpec::DerivedWithDefault {
                    input_name: "name",
                    default: "result",
                },
                ty: Type::Any,
                description: "The value that was retrieved",
                scope: OutputScope::Operation,
            }],
            requires_extensions: &[],
        }
    }

    fn execute(context: &mut Context) -> Result<(), OperationError> {
        let array = context.input("array")?.as_array()?;
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();

        for item in array {
            if seen.insert(item.clone()) {
                deduped.push(item.clone());
            }
        }

        context.set_derived_output("name", deduped)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_result_array(complete: &Pipeline<Complete>, step_name: &str) -> Vec<Value> {
        let key = format!("{}.result", step_name);
        complete
            .variables()
            .get(&key)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.get_value().unwrap().clone())
            .collect()
    }

    // Empty array

    #[test]
    fn dedupe_empty_array() {
        let mut pipe = Pipeline::default();
        pipe.array("data").unwrap();
        pipe.step::<Dedupe>(
            "dedupe",
            params!("array" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = get_result_array(&complete, "dedupe");
        assert!(result.is_empty());
    }

    // No duplicates

    #[test]
    fn dedupe_no_duplicates() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(1i64)
            .unwrap()
            .push(2i64)
            .unwrap()
            .push(3i64)
            .unwrap();
        pipe.step::<Dedupe>(
            "dedupe",
            params!("array" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = get_result_array(&complete, "dedupe");
        assert_eq!(result, vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)]);
    }

    // With duplicates

    #[test]
    fn dedupe_integers_with_duplicates() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(1i64)
            .unwrap()
            .push(2i64)
            .unwrap()
            .push(1i64)
            .unwrap()
            .push(3i64)
            .unwrap()
            .push(2i64)
            .unwrap();
        pipe.step::<Dedupe>(
            "dedupe",
            params!("array" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = get_result_array(&complete, "dedupe");
        assert_eq!(result, vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)]);
    }

    #[test]
    fn dedupe_text_with_duplicates() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push("apple")
            .unwrap()
            .push("banana")
            .unwrap()
            .push("apple")
            .unwrap()
            .push("cherry")
            .unwrap()
            .push("banana")
            .unwrap();
        pipe.step::<Dedupe>(
            "dedupe",
            params!("array" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = get_result_array(&complete, "dedupe");
        assert_eq!(
            result,
            vec![
                Value::Text("apple".into()),
                Value::Text("banana".into()),
                Value::Text("cherry".into()),
            ]
        );
    }

    #[test]
    fn dedupe_all_same() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(42i64)
            .unwrap()
            .push(42i64)
            .unwrap()
            .push(42i64)
            .unwrap();
        pipe.step::<Dedupe>(
            "dedupe",
            params!("array" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = get_result_array(&complete, "dedupe");
        assert_eq!(result, vec![Value::Integer(42)]);
    }

    // Preserves first occurrence order

    #[test]
    fn dedupe_preserves_order() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(3i64)
            .unwrap()
            .push(1i64)
            .unwrap()
            .push(2i64)
            .unwrap()
            .push(1i64)
            .unwrap()
            .push(3i64)
            .unwrap();
        pipe.step::<Dedupe>(
            "dedupe",
            params!("array" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = get_result_array(&complete, "dedupe");
        assert_eq!(result, vec![Value::Integer(3), Value::Integer(1), Value::Integer(2)]);
    }

    // Booleans

    #[test]
    fn dedupe_booleans() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(true)
            .unwrap()
            .push(false)
            .unwrap()
            .push(true)
            .unwrap()
            .push(false)
            .unwrap();
        pipe.step::<Dedupe>(
            "dedupe",
            params!("array" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = get_result_array(&complete, "dedupe");
        assert_eq!(result, vec![Value::Boolean(true), Value::Boolean(false)]);
    }

    // Custom output name

    #[test]
    fn dedupe_with_custom_name() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(1i64)
            .unwrap()
            .push(1i64)
            .unwrap();
        pipe.step::<Dedupe>(
            "dedupe",
            params!(
                "array" => Param::reference("data"),
                "name" => "unique",
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let entry = complete.variables().get("dedupe.unique").unwrap();
        let arr = entry.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].get_value().unwrap(), &Value::Integer(1));
    }

    // Single element

    #[test]
    fn dedupe_single_element() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(99i64)
            .unwrap();
        pipe.step::<Dedupe>(
            "dedupe",
            params!("array" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = get_result_array(&complete, "dedupe");
        assert_eq!(result, vec![Value::Integer(99)]);
    }
}
