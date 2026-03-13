use crate::imports::*;
use std::cmp::Ordering;

#[derive(Debug, Clone, Default)]
pub struct Compare;

fn apply_ordering(ord: Ordering, op: &str) -> bool {
    match op {
        "eq" => ord == Ordering::Equal,
        "neq" => ord != Ordering::Equal,
        "gt" => ord == Ordering::Greater,
        "gte" => ord != Ordering::Less,
        "lt" => ord == Ordering::Less,
        "lte" => ord != Ordering::Greater,
        _ => unreachable!(),
    }
}

fn compare_floats(l: f64, r: f64, op: &str, context: &Context) -> Result<bool, OperationError> {
    match op {
        "eq" => Ok(l == r),
        "neq" => Ok(l != r),
        _ => match l.partial_cmp(&r) {
            Some(ord) => Ok(apply_ordering(ord, op)),
            None => Err(op_error!(
                context,
                "Cannot order floats: NaN is not orderable"
            )),
        },
    }
}

impl Operation for Compare {
    fn metadata() -> OperationMetadata {
        OperationMetadata {
            name: "Compare",
            description: "Compares two values and returns a boolean. \
                Operators: eq (equal), neq (not equal), gt (greater than), \
                gte (greater or equal), lt (less than), lte (less or equal). \
                Null: eq/neq only. Boolean: eq/neq only. Text: eq/neq only (case-sensitive). \
                Integer and Float: all operators; cross-numeric promotes integer to float.",
            inputs: &[
                InputSpec {
                    name: "left",
                    ty: Type::Any,
                    required: true,
                    default: None,
                    description: "Left-hand side of the comparison",
                },
                InputSpec {
                    name: "op",
                    ty: Type::Text,
                    required: true,
                    default: None,
                    description:
                        "Comparison operator: eq, neq, gt, gte, lt, lte (case-insensitive)",
                },
                InputSpec {
                    name: "right",
                    ty: Type::Any,
                    required: true,
                    default: None,
                    description: "Right-hand side of the comparison",
                },
                InputSpec {
                    name: "name",
                    ty: Type::Text,
                    required: false,
                    default: None,
                    description: "Optional output variable name (defaults to 'result')",
                },
            ],
            outputs: &[OutputSpec {
                name: NameSpec::DerivedWithDefault {
                    input_name: "name",
                    default: "result",
                },
                ty: Type::Boolean,
                description: "The boolean result of the comparison",
                scope: OutputScope::Operation,
            }],
            requires_extensions: &[],
        }
    }

    fn execute(context: &mut Context) -> Result<(), OperationError> {
        let left = context.input("left")?.get_value()?.clone();
        let op = context
            .input("op")?
            .get_value()?
            .as_text()?
            .to_ascii_lowercase();
        let right = context.input("right")?.get_value()?.clone();

        // Validate operator
        if !matches!(op.as_str(), "eq" | "neq" | "gt" | "gte" | "lt" | "lte") {
            return Err(op_error!(context, "Unknown comparison operator '{op}'"));
        }

        let is_ordering = matches!(op.as_str(), "gt" | "gte" | "lt" | "lte");

        let result = match (&left, &right) {
            // Null comparisons — eq/neq only
            (Value::Null, _) | (_, Value::Null) => {
                if is_ordering {
                    return Err(op_error!(
                        context,
                        "Cannot use ordering operator '{op}' with Null"
                    ));
                }
                let both_null = matches!((&left, &right), (Value::Null, Value::Null));
                match op.as_str() {
                    "eq" => both_null,
                    "neq" => !both_null,
                    _ => unreachable!(),
                }
            }

            // Boolean — eq/neq only
            (Value::Boolean(l), Value::Boolean(r)) => {
                if is_ordering {
                    return Err(op_error!(
                        context,
                        "Cannot use ordering operator '{op}' with Boolean"
                    ));
                }
                match op.as_str() {
                    "eq" => l == r,
                    "neq" => l != r,
                    _ => unreachable!(),
                }
            }

            // Text — eq/neq only (case-sensitive)
            (Value::Text(l), Value::Text(r)) => {
                if is_ordering {
                    return Err(op_error!(
                        context,
                        "Cannot use ordering operator '{op}' with Text"
                    ));
                }
                match op.as_str() {
                    "eq" => l == r,
                    "neq" => l != r,
                    _ => unreachable!(),
                }
            }

            // Integer — all operators
            (Value::Integer(l), Value::Integer(r)) => apply_ordering(l.cmp(r), &op),

            // Float — all operators
            (Value::Float(l), Value::Float(r)) => compare_floats(*l, *r, &op, context)?,

            // Cross-numeric: promote integer to float
            (Value::Integer(l), Value::Float(r)) => {
                compare_floats(*l as f64, *r, &op, context)?
            }
            (Value::Float(l), Value::Integer(r)) => {
                compare_floats(*l, *r as f64, &op, context)?
            }

            // Incompatible types
            _ => {
                return Err(op_error!(
                    context,
                    "Cannot compare {} with {}: incompatible types",
                    left.get_type(),
                    right.get_type()
                ));
            }
        };

        context.set_derived_output("name", <bool as Into<Value>>::into(result))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_compare(
        left: Param,
        op: &str,
        right: Param,
    ) -> Result<Pipeline<Complete>, OperationError> {
        let mut pipe = Pipeline::default();
        pipe.var("left", Value::Null).unwrap(); // placeholder for ref-based params
        pipe.step::<Compare>(
            "cmp",
            params!(
                "left" => left,
                "op" => op,
                "right" => right,
            ),
        )
        .unwrap();
        pipe.compile().unwrap().run().wait()
    }

    fn get_result(complete: &Pipeline<Complete>) -> bool {
        complete
            .variables()
            .get("cmp.result")
            .unwrap()
            .get_value()
            .unwrap()
            .as_boolean()
            .unwrap()
    }

    // ---- Happy path: Integer ----

    #[test]
    fn int_eq_true() {
        let c = run_compare(42i64.into(), "eq", 42i64.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn int_eq_false() {
        let c = run_compare(42i64.into(), "eq", 99i64.into()).unwrap();
        assert!(!get_result(&c));
    }

    #[test]
    fn int_neq() {
        let c = run_compare(1i64.into(), "neq", 2i64.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn int_gt() {
        let c = run_compare(10i64.into(), "gt", 5i64.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn int_gte_equal() {
        let c = run_compare(7i64.into(), "gte", 7i64.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn int_lt() {
        let c = run_compare(3i64.into(), "lt", 9i64.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn int_lte() {
        let c = run_compare(5i64.into(), "lte", 5i64.into()).unwrap();
        assert!(get_result(&c));
    }

    // ---- Happy path: Float ----

    #[test]
    fn float_gt() {
        let c = run_compare(3.14f64.into(), "gt", 2.71f64.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn float_lt() {
        let c = run_compare(1.0f64.into(), "lt", 2.0f64.into()).unwrap();
        assert!(get_result(&c));
    }

    // ---- Happy path: Cross-numeric ----

    #[test]
    fn cross_numeric_int_gt_float() {
        let c = run_compare(10i64.into(), "gt", 9.5f64.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn cross_numeric_float_lt_int() {
        let c = run_compare(1.5f64.into(), "lt", 2i64.into()).unwrap();
        assert!(get_result(&c));
    }

    // ---- Happy path: Boolean ----

    #[test]
    fn bool_eq_same() {
        let c = run_compare(true.into(), "eq", true.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn bool_neq() {
        let c = run_compare(true.into(), "neq", false.into()).unwrap();
        assert!(get_result(&c));
    }

    // ---- Happy path: Text ----

    #[test]
    fn text_eq_same() {
        let c = run_compare("hello".into(), "eq", "hello".into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn text_eq_different() {
        let c = run_compare("hello".into(), "eq", "world".into()).unwrap();
        assert!(!get_result(&c));
    }

    #[test]
    fn text_neq() {
        let c = run_compare("hello".into(), "neq", "world".into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn text_case_sensitive() {
        let c = run_compare("Hello".into(), "eq", "hello".into()).unwrap();
        assert!(!get_result(&c));
    }

    // ---- Happy path: Null ----

    #[test]
    fn null_eq_null() {
        let c = run_compare(
            Param::Literal(Value::Null),
            "eq",
            Param::Literal(Value::Null),
        )
        .unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn null_neq_int() {
        let c = run_compare(Param::Literal(Value::Null), "neq", 42i64.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn null_eq_int() {
        let c = run_compare(Param::Literal(Value::Null), "eq", 42i64.into()).unwrap();
        assert!(!get_result(&c));
    }

    #[test]
    fn int_eq_null() {
        let c = run_compare(42i64.into(), "eq", Param::Literal(Value::Null)).unwrap();
        assert!(!get_result(&c));
    }

    // ---- Happy path: Operator case insensitivity ----

    #[test]
    fn op_case_insensitive() {
        let c = run_compare(1i64.into(), "EQ", 1i64.into()).unwrap();
        assert!(get_result(&c));
    }

    // ---- Happy path: Custom output name ----

    #[test]
    fn custom_output_name() {
        let mut pipe = Pipeline::default();
        pipe.step::<Compare>(
            "cmp",
            params!(
                "left" => 42i64,
                "op" => "eq",
                "right" => 42i64,
                "name" => "is_equal",
            ),
        )
        .unwrap();
        let complete = pipe.compile().unwrap().run().wait().unwrap();
        let result = complete
            .variables()
            .get("cmp.is_equal")
            .unwrap()
            .get_value()
            .unwrap()
            .as_boolean()
            .unwrap();
        assert!(result);
    }

    // ---- Happy path: Float edge cases (non-error) ----

    #[test]
    fn nan_eq_nan() {
        let c = run_compare(f64::NAN.into(), "eq", f64::NAN.into()).unwrap();
        assert!(!get_result(&c));
    }

    #[test]
    fn nan_neq_nan() {
        let c = run_compare(f64::NAN.into(), "neq", f64::NAN.into()).unwrap();
        assert!(get_result(&c));
    }

    #[test]
    fn inf_gt_large() {
        let c = run_compare(f64::INFINITY.into(), "gt", 999999.0f64.into()).unwrap();
        assert!(get_result(&c));
    }

    // ---- Error path: Unknown operator ----

    #[test]
    fn unknown_operator() {
        assert!(run_compare(1i64.into(), "nope", 2i64.into()).is_err());
    }

    // ---- Error path: Ordering on non-orderable types ----

    #[test]
    fn null_gt_errors() {
        assert!(run_compare(
            Param::Literal(Value::Null),
            "gt",
            Param::Literal(Value::Null)
        )
        .is_err());
    }

    #[test]
    fn bool_gt_errors() {
        assert!(run_compare(true.into(), "gt", false.into()).is_err());
    }

    #[test]
    fn text_gt_errors() {
        assert!(run_compare("a".into(), "gt", "b".into()).is_err());
    }

    // ---- Error path: Incompatible types ----

    #[test]
    fn incompatible_int_vs_text() {
        assert!(run_compare(42i64.into(), "eq", "hello".into()).is_err());
    }

    #[test]
    fn incompatible_bool_vs_int() {
        assert!(run_compare(true.into(), "eq", 1i64.into()).is_err());
    }

    #[test]
    fn incompatible_text_vs_float() {
        assert!(run_compare("hello".into(), "eq", 3.14f64.into()).is_err());
    }

    // ---- Error path: NaN ordering ----

    #[test]
    fn nan_gt_errors() {
        assert!(run_compare(f64::NAN.into(), "gt", 1.0f64.into()).is_err());
    }
}
