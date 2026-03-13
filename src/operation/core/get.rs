use crate::imports::*;

#[derive(Debug, Clone, Default)]
pub struct Get;

impl Operation for Get {
    fn metadata() -> OperationMetadata {
        OperationMetadata {
            name: "Get",
            description: "Extracts an element from an array by index or a map by key.",
            inputs: &[
                InputSpec {
                    name: "source",
                    ty: Type::Any,
                    required: true,
                    default: None,
                    description: "An array or map store entry to get data from",
                },
                InputSpec {
                    name: "key",
                    ty: Type::Any,
                    required: true,
                    default: None,
                    description: "Key to look up in the source entry, type depends on the source (e.g. integer index for arrays, text key for maps)",
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
        let source = context.input("source")?;
        let key = context.input("key")?.get_value()?;

        let value = if let Ok(array) = source.as_array() {
            let index = key.as_integer().map_err(|_| {
                op_error!(
                    context,
                    "Key for array source must be an integer index, found: {} of type {}",
                    key,
                    key.get_type()
                )
            })?;
            array
                .get(index as usize)
                .ok_or_else(|| {
                    op_error!(
                        context,
                        "Index {} out of bounds for array of length {}",
                        index,
                        array.len()
                    )
                })?
                .clone()
        } else if let Ok(map) = source.as_map() {
            let key_str = key.as_text().map_err(|_| {
                op_error!(
                    context,
                    "Key for map source must be a text string, found: {} of type {}",
                    key,
                    key.get_type()
                )
            })?;
            map.get(key_str)
                .ok_or_else(|| op_error!(context, "Key '{}' not found in map", key_str))?
                .clone()
        } else {
            return Err(op_error!(context, "Source must be an array or map"));
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

    // Array happy paths

    #[test]
    fn get_array_first() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(10i64)
            .unwrap()
            .push(20i64)
            .unwrap()
            .push(30i64)
            .unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => 0i64,
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "get"), Value::Integer(10));
    }

    #[test]
    fn get_array_middle() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(10i64)
            .unwrap()
            .push(20i64)
            .unwrap()
            .push(30i64)
            .unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => 1i64,
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "get"), Value::Integer(20));
    }

    #[test]
    fn get_array_last() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(10i64)
            .unwrap()
            .push(20i64)
            .unwrap()
            .push(30i64)
            .unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => 2i64,
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "get"), Value::Integer(30));
    }

    #[test]
    fn get_array_text_element() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push("hello")
            .unwrap()
            .push("world")
            .unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => 1i64,
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "get"), Value::Text("world".into()));
    }

    // Map happy paths

    #[test]
    fn get_map_existing_key() {
        let mut pipe = Pipeline::default();
        pipe.map("data")
            .unwrap()
            .insert("host", "localhost")
            .unwrap()
            .insert("port", "8080")
            .unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => "host",
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(
            get_result(&complete, "get"),
            Value::Text("localhost".into())
        );
    }

    #[test]
    fn get_map_different_keys() {
        let mut pipe = Pipeline::default();
        pipe.map("data")
            .unwrap()
            .insert("a", 1i64)
            .unwrap()
            .insert("b", 2i64)
            .unwrap();

        pipe.step::<Get>(
            "get_a",
            params!(
                "source" => Param::reference("data"),
                "key" => "a",
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        assert_eq!(get_result(&complete, "get_a"), Value::Integer(1));
    }

    // Custom output name

    #[test]
    fn get_with_custom_name() {
        let mut pipe = Pipeline::default();
        pipe.array("data").unwrap().push(42i64).unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => 0i64,
                "name" => "my_value",
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = complete
            .variables()
            .get("get.my_value")
            .unwrap()
            .get_value()
            .unwrap();
        assert_eq!(result, &Value::Integer(42));
    }

    // Array error paths

    #[test]
    fn get_array_index_out_of_bounds() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(10i64)
            .unwrap()
            .push(20i64)
            .unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => 5i64,
            ),
        )
        .unwrap();
        assert!(pipe.compile().unwrap().run().wait().is_err());
    }

    #[test]
    fn get_array_negative_index() {
        let mut pipe = Pipeline::default();
        pipe.array("data")
            .unwrap()
            .push(10i64)
            .unwrap()
            .push(20i64)
            .unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => -1i64,
            ),
        )
        .unwrap();
        assert!(pipe.compile().unwrap().run().wait().is_err());
    }

    #[test]
    fn get_array_with_text_key() {
        let mut pipe = Pipeline::default();
        pipe.array("data").unwrap().push(10i64).unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => "not_an_index",
            ),
        )
        .unwrap();
        assert!(pipe.compile().unwrap().run().wait().is_err());
    }

    // Map error paths

    #[test]
    fn get_map_key_not_found() {
        let mut pipe = Pipeline::default();
        pipe.map("data").unwrap().insert("a", 1i64).unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => "missing",
            ),
        )
        .unwrap();
        assert!(pipe.compile().unwrap().run().wait().is_err());
    }

    #[test]
    fn get_map_with_integer_key() {
        let mut pipe = Pipeline::default();
        pipe.map("data").unwrap().insert("a", 1i64).unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => 42i64,
            ),
        )
        .unwrap();
        assert!(pipe.compile().unwrap().run().wait().is_err());
    }

    // Source type errors

    #[test]
    fn get_from_var_errors() {
        let mut pipe = Pipeline::default();
        pipe.var("data", "just a string").unwrap();
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => 0i64,
            ),
        )
        .unwrap();
        assert!(pipe.compile().unwrap().run().wait().is_err());
    }

    // Nested collections

    #[test]
    fn get_nested_array_from_map() {
        let mut pipe = Pipeline::default();
        {
            let mut map = pipe.map("data").unwrap();
            map.insert("name", "test").unwrap();
            map.insert_array("items")
                .unwrap()
                .push(100i64)
                .unwrap()
                .push(200i64)
                .unwrap();
        }
        pipe.step::<Get>(
            "get",
            params!(
                "source" => Param::reference("data"),
                "key" => "items",
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let entry = complete.variables().get("get.result").unwrap();
        let arr = entry.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].get_value().unwrap(), &Value::Integer(100));
        assert_eq!(arr[1].get_value().unwrap(), &Value::Integer(200));
    }
}
