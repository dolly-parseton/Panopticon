use crate::imports::*;

/// Built-in operation that reports the length of a text, array, or map
/// entry.
///
/// **Inputs:**
/// - `source` (Any, required) — a text, array, or map store entry.
/// - `name` (Text, optional) — the output variable name (default
///   `result`).
///
/// **Outputs:**
/// - An operation-scoped `Integer` output. For text the count is in
///   Unicode code points; for arrays and maps it is the entry count.
#[derive(Debug, Clone, Default)]
pub struct Len;

impl Operation for Len {
    fn metadata() -> OperationMetadata {
        OperationMetadata {
            name: "Len",
            description: "Gets the length of a text, array, or map store entry",
            inputs: &[
                InputSpec {
                    name: "source",
                    ty: Type::Any,
                    required: true,
                    default: None,
                    description: "A text, array, or map store entry to get the length of",
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
                description: "The length of the source",
                scope: OutputScope::Operation,
            }],
            requires_extensions: &[],
        }
    }

    fn execute(context: &mut Context) -> Result<(), OperationError> {
        let source = context.input("source")?;

        let value = if let Ok(array) = source.as_array() {
            Value::Integer(array.len() as i64)
        } else if let Ok(map) = source.as_map() {
            Value::Integer(map.len() as i64)
        } else if let Ok(val) = source.get_value() {
            let text = val.as_text().map_err(|_| {
                op_error!(context, "Source must be an array, map, or text value")
            })?;
            Value::Integer(text.chars().count() as i64)
        } else {
            return Err(op_error!(context, "Source must be an array, map, or text value"));
        };

        context.set_derived_output("name", value)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_result(complete: &Pipeline<Complete>, step_name: &str) -> Value {
        let key = format!("{}.result", step_name);
        complete
            .variables()
            .get(&key)
            .unwrap()
            .get_value()
            .unwrap()
            .clone()
    }

    // Array lengths

    #[test]
    fn len_array_empty() {
        let mut pipe = Pipeline::default();
        pipe.array("data").unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "len"), Value::Integer(0));
    }

    #[test]
    fn len_array_non_empty() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(10i64)
            .unwrap()
            .push(20i64)
            .unwrap()
            .push(30i64)
            .unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "len"), Value::Integer(3));
    }

    // Map lengths

    #[test]
    fn len_map_empty() {
        let mut pipe = Pipeline::default();
        pipe.map("data").unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "len"), Value::Integer(0));
    }

    #[test]
    fn len_map_non_empty() {
        let mut pipe = Pipeline::default();
        pipe.map("data")
            .unwrap()
            .insert("a", 1i64)
            .unwrap()
            .insert("b", 2i64)
            .unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "len"), Value::Integer(2));
    }

    // Text lengths

    #[test]
    fn len_text_empty() {
        let mut pipe = Pipeline::default();
        pipe.var("data", "").unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "len"), Value::Integer(0));
    }

    #[test]
    fn len_text_ascii() {
        let mut pipe = Pipeline::default();
        pipe.var("data", "hello").unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "len"), Value::Integer(5));
    }

    #[test]
    fn len_text_unicode() {
        let mut pipe = Pipeline::default();
        pipe.var("data", "héllo 🌍").unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        // chars().count() = 7: h é l l o ' ' 🌍
        assert_eq!(get_result(&complete, "len"), Value::Integer(7));
    }

    // Custom output name

    #[test]
    fn len_with_custom_name() {
        let mut pipe = Pipeline::default();
        pipe.var("data", "test").unwrap();
        pipe.step::<Len>(
            "len",
            params!(
                "source" => Param::reference("data"),
                "name" => "length",
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = complete
            .variables()
            .get("len.length")
            .unwrap()
            .get_value()
            .unwrap();
        assert_eq!(result, &Value::Integer(4));
    }

    // Error paths

    #[test]
    fn len_integer_errors() {
        let mut pipe = Pipeline::default();
        pipe.var("data", 42i64).unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        assert!(pipe.compile().unwrap().run().wait().is_err());
    }

    #[test]
    fn len_boolean_errors() {
        let mut pipe = Pipeline::default();
        pipe.var("data", true).unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        assert!(pipe.compile().unwrap().run().wait().is_err());
    }

    #[test]
    fn len_float_errors() {
        let mut pipe = Pipeline::default();
        pipe.var("data", 3.14f64).unwrap();
        pipe.step::<Len>(
            "len",
            params!("source" => Param::reference("data")),
        )
        .unwrap();
        assert!(pipe.compile().unwrap().run().wait().is_err());
    }
}
