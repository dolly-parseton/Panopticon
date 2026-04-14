use crate::imports::*;

/// Built-in operation that converts a scalar value between primitive types.
///
/// **Inputs:**
/// - `value` (Any, required) — a scalar value (`Boolean`, `Integer`,
///   `Float`, or `Text`).
/// - `type` (Text, required) — the target type name (`"int"`, `"float"`,
///   `"text"`, or `"bool"`), case-insensitive.
/// - `name` (Text, optional) — the output variable name (default
///   `result`).
///
/// **Outputs:**
/// - An operation-scoped output holding the coerced value. Out-of-range
///   coercions (e.g. integer to bool with values other than 0 or 1)
///   fail with a custom [`OperationError`].
#[derive(Debug, Clone, Default)]
pub struct Coerce;

impl Operation for Coerce {
    fn metadata() -> OperationMetadata {
        OperationMetadata {
            name: "Coerce",
            description: "Coerces a value to a different type",
            inputs: &[
                InputSpec {
                    name: "value",
                    ty: Type::Any,
                    required: true,
                    default: None,
                    description: "Scalar value to coerce, must be of type Int, Float, Text, or Bool",
                },
                InputSpec {
                    name: "type",
                    ty: Type::Text,
                    required: true,
                    default: None,
                    description: "Type to coerce to ('Int', 'Float', 'Text', 'Bool'), case-insensitive",
                },
                InputSpec {
                    name: "name",
                    ty: Type::Text,
                    required: false,
                    default: None,
                    description: "Optional name for the output variable (defaults to 'result')",
                },
            ],
            outputs: &[OutputSpec {
                name: NameSpec::DerivedWithDefault {
                    input_name: "name",
                    default: "result",
                },
                ty: Type::Any,
                description: "The coerced value",
                scope: OutputScope::Operation,
            }],
            requires_extensions: &[],
        }
    }

    fn execute(context: &mut Context) -> Result<(), OperationError> {
        let value = context.input("value")?.get_value()?;
        let target_type = context
            .input("type")?
            .get_value()?
            .as_text()?
            .to_ascii_lowercase();
        let value_type = value.get_type();
        match (value_type, target_type.as_str()) {
            (Type::Integer, "int")
            | (Type::Float, "float")
            | (Type::Text, "text")
            | (Type::Boolean, "bool") => {
                context.set_derived_output("name", value.clone())?;
            }
            // Bool to other types
            (Type::Boolean, "int") => {
                let coerced = if value.as_boolean()? { 1i64 } else { 0i64 };
                context.set_derived_output("name", <i64 as Into<Value>>::into(coerced))?;
            }
            (Type::Boolean, "float") => {
                let coerced = if value.as_boolean()? { 1.0f64 } else { 0.0f64 };
                context.set_derived_output("name", <f64 as Into<Value>>::into(coerced))?;
            }
            (Type::Boolean, "text") => {
                let coerced = value.as_boolean()?.to_string();
                context.set_derived_output("name", <String as Into<Value>>::into(coerced))?;
            }
            // Int to other types
            (Type::Integer, "float") => {
                let coerced = value.as_integer()? as f64;
                context.set_derived_output("name", <f64 as Into<Value>>::into(coerced))?;
            }
            (Type::Integer, "text") => {
                let coerced = value.as_integer()?.to_string();
                context.set_derived_output("name", <String as Into<Value>>::into(coerced))?;
            }
            (Type::Integer, "bool") => {
                let coerced = match value.as_integer()? {
                    0 => false,
                    1 => true,
                    other => {
                        return Err(op_error!(
                            context,
                            "Cannot coerce integer {other} to bool (only 0 and 1 are valid)"
                        ));
                    }
                };
                context.set_derived_output("name", <bool as Into<Value>>::into(coerced))?;
            }
            // Float to other types
            (Type::Float, "int") => {
                let coerced = value.as_float()? as i64; // Rounding?
                context.set_derived_output("name", <i64 as Into<Value>>::into(coerced))?;
            }
            (Type::Float, "text") => {
                let coerced = value.as_float()?.to_string();
                context.set_derived_output("name", <String as Into<Value>>::into(coerced))?;
            }
            (Type::Float, "bool") => {
                let coerced = match value.as_float()? {
                    0.0f64 => false,
                    1.0f64 => true,
                    other => {
                        return Err(op_error!(
                            context,
                            "Cannot coerce float {other} to bool (only 0.0 and 1.0 are valid)"
                        ));
                    }
                };
                context.set_derived_output("name", <bool as Into<Value>>::into(coerced))?;
            }
            // Text to other types
            (Type::Text, "int") => {
                let text = value.as_text()?;
                let coerced = text
                    .parse::<i64>()
                    .map_err(|_| op_error!(context, "Cannot coerce text '{text}' to int"))?;
                context.set_derived_output("name", <i64 as Into<Value>>::into(coerced))?;
            }
            (Type::Text, "float") => {
                let text = value.as_text()?;
                let coerced = text
                    .parse::<f64>()
                    .map_err(|_| op_error!(context, "Cannot coerce text '{text}' to float"))?;
                context.set_derived_output("name", <f64 as Into<Value>>::into(coerced))?;
            }
            (Type::Text, "bool") => {
                let text = value.as_text()?;
                let coerced = match text.to_ascii_lowercase().as_str() {
                    "true" | "1" => true,
                    "false" | "0" => false,
                    _ => {
                        return Err(op_error!(
                            context,
                            "Cannot coerce text '{text}' to bool (expected 'true', 'false', '0', or '1')"
                        ));
                    }
                };
                context.set_derived_output("name", <bool as Into<Value>>::into(coerced))?;
            }
            // Unsupported coercions
            _ => {
                return Err(op_error!(
                    context,
                    "Cannot coerce {} to '{target_type}'",
                    value.get_type()
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_coerce(
        input_value: impl Into<Value>,
        target_type: &str,
    ) -> Result<Pipeline<Complete>, OperationError> {
        let mut pipe = Pipeline::default();
        pipe.var("input", input_value).unwrap();
        pipe.step::<Coerce>(
            "coerce",
            params!(
                "value" => Param::reference("input"),
                "type" => target_type,
            ),
        )
        .unwrap();
        pipe.compile().unwrap().run().wait()
    }

    fn get_result(complete: &Pipeline<Complete>) -> &Value {
        complete
            .variables()
            .get("coerce.result")
            .unwrap()
            .get_value()
            .unwrap()
    }

    // Identity coercions

    #[test]
    fn bool_to_bool() {
        let complete = run_coerce(true, "bool").unwrap();
        assert_eq!(get_result(&complete), &Value::Boolean(true));
    }

    #[test]
    fn int_to_int() {
        let complete = run_coerce(42i64, "int").unwrap();
        assert_eq!(get_result(&complete), &Value::Integer(42));
    }

    #[test]
    fn float_to_float() {
        let complete = run_coerce(3.14f64, "float").unwrap();
        assert_eq!(get_result(&complete), &Value::Float(3.14));
    }

    #[test]
    fn text_to_text() {
        let complete = run_coerce("hello".to_string(), "text").unwrap();
        assert_eq!(get_result(&complete), &Value::Text("hello".into()));
    }

    // Bool to other types

    #[test]
    fn bool_to_int() {
        let complete = run_coerce(true, "int").unwrap();
        assert_eq!(get_result(&complete), &Value::Integer(1));

        let complete = run_coerce(false, "int").unwrap();
        assert_eq!(get_result(&complete), &Value::Integer(0));
    }

    #[test]
    fn bool_to_float() {
        let complete = run_coerce(true, "float").unwrap();
        assert_eq!(get_result(&complete), &Value::Float(1.0));

        let complete = run_coerce(false, "float").unwrap();
        assert_eq!(get_result(&complete), &Value::Float(0.0));
    }

    #[test]
    fn bool_to_text() {
        let complete = run_coerce(true, "text").unwrap();
        assert_eq!(get_result(&complete), &Value::Text("true".into()));

        let complete = run_coerce(false, "text").unwrap();
        assert_eq!(get_result(&complete), &Value::Text("false".into()));
    }

    // Int to other types

    #[test]
    fn int_to_float() {
        let complete = run_coerce(42i64, "float").unwrap();
        assert_eq!(get_result(&complete), &Value::Float(42.0));
    }

    #[test]
    fn int_to_text() {
        let complete = run_coerce(42i64, "text").unwrap();
        assert_eq!(get_result(&complete), &Value::Text("42".into()));
    }

    #[test]
    fn int_to_bool() {
        let complete = run_coerce(0i64, "bool").unwrap();
        assert_eq!(get_result(&complete), &Value::Boolean(false));

        let complete = run_coerce(1i64, "bool").unwrap();
        assert_eq!(get_result(&complete), &Value::Boolean(true));
    }

    #[test]
    fn int_to_bool_rejects_invalid() {
        assert!(run_coerce(2i64, "bool").is_err());
        assert!(run_coerce(-1i64, "bool").is_err());
    }

    // Float to other types

    #[test]
    fn float_to_int() {
        let complete = run_coerce(3.7f64, "int").unwrap();
        assert_eq!(get_result(&complete), &Value::Integer(3));
    }

    #[test]
    fn float_to_text() {
        let complete = run_coerce(2.5f64, "text").unwrap();
        assert_eq!(get_result(&complete), &Value::Text("2.5".into()));
    }

    #[test]
    fn float_to_bool() {
        let complete = run_coerce(0.0f64, "bool").unwrap();
        assert_eq!(get_result(&complete), &Value::Boolean(false));

        let complete = run_coerce(1.0f64, "bool").unwrap();
        assert_eq!(get_result(&complete), &Value::Boolean(true));
    }

    #[test]
    fn float_to_bool_rejects_invalid() {
        assert!(run_coerce(2.5f64, "bool").is_err());
        assert!(run_coerce(-1.0f64, "bool").is_err());
    }

    // Text to other types

    #[test]
    fn text_to_int() {
        let complete = run_coerce("42".to_string(), "int").unwrap();
        assert_eq!(get_result(&complete), &Value::Integer(42));
    }

    #[test]
    fn text_to_int_rejects_non_numeric() {
        assert!(run_coerce("hello".to_string(), "int").is_err());
    }

    #[test]
    fn text_to_float() {
        let complete = run_coerce("3.14".to_string(), "float").unwrap();
        assert_eq!(get_result(&complete), &Value::Float(3.14));
    }

    #[test]
    fn text_to_float_rejects_non_numeric() {
        assert!(run_coerce("hello".to_string(), "float").is_err());
    }

    #[test]
    fn text_to_bool() {
        let complete = run_coerce("true".to_string(), "bool").unwrap();
        assert_eq!(get_result(&complete), &Value::Boolean(true));

        let complete = run_coerce("false".to_string(), "bool").unwrap();
        assert_eq!(get_result(&complete), &Value::Boolean(false));

        let complete = run_coerce("1".to_string(), "bool").unwrap();
        assert_eq!(get_result(&complete), &Value::Boolean(true));

        let complete = run_coerce("0".to_string(), "bool").unwrap();
        assert_eq!(get_result(&complete), &Value::Boolean(false));
    }

    #[test]
    fn text_to_bool_rejects_invalid() {
        assert!(run_coerce("yes".to_string(), "bool").is_err());
        assert!(run_coerce("2".to_string(), "bool").is_err());
    }

    // Unsupported target type

    #[test]
    fn unsupported_target_type_errors() {
        assert!(run_coerce(42i64, "array").is_err());
        assert!(run_coerce(42i64, "map").is_err());
        assert!(run_coerce(42i64, "nonsense").is_err());
    }

    // Case insensitivity

    #[test]
    fn target_type_case_insensitive() {
        let complete = run_coerce(42i64, "INT").unwrap();
        assert_eq!(get_result(&complete), &Value::Integer(42));

        let complete = run_coerce(42i64, "Float").unwrap();
        assert_eq!(get_result(&complete), &Value::Float(42.0));

        let complete = run_coerce(42i64, "TEXT").unwrap();
        assert_eq!(get_result(&complete), &Value::Text("42".into()));
    }

    // Custom output name

    #[test]
    fn custom_output_name() {
        let mut pipe = Pipeline::default();
        pipe.var("input", 42i64).unwrap();
        pipe.step::<Coerce>(
            "coerce",
            params!(
                "value" => Param::reference("input"),
                "type" => "text",
                "name" => "my_output",
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = complete
            .variables()
            .get("coerce.my_output")
            .unwrap()
            .get_value()
            .unwrap();
        assert_eq!(result, &Value::Text("42".into()));
    }
}
