use crate::prelude::*;
use std::sync::LazyLock;

static CONDITIONCOMMAND_ATTRIBUTES: LazyLock<Vec<AttributeSpec<&'static str>>> =
    LazyLock::new(|| {
        vec![
            AttributeSpec {
                name: "branches",
                ty: TypeDef::ArrayOf(Box::new(TypeDef::ObjectOf {
                    fields: vec![
                        FieldSpec {
                            name: "if",
                            ty: TypeDef::Scalar(ScalarType::String),
                            required: true,
                            hint: Some("Tera expression to evaluate as condition"),
                        },
                        FieldSpec {
                            name: "then",
                            ty: TypeDef::Scalar(ScalarType::String),
                            required: true,
                            hint: Some("Value if condition is true (supports Tera substitution)"),
                        },
                    ],
                })),
                required: true,
                hint: Some("Array of {if, then} objects evaluated in order"),
                default_value: None,
            },
            AttributeSpec {
                name: "default",
                ty: TypeDef::Scalar(ScalarType::String),
                required: false,
                hint: Some("Default value if no branch matches (supports Tera substitution)"),
                default_value: None,
            },
        ]
    });

const CONDITIONCOMMAND_OUTPUTS: &[ResultSpec<&'static str>] = &[
    ResultSpec {
        name: "result",
        ty: TypeDef::Scalar(ScalarType::String),
        hint: Some("The value from the matched branch or default."),
    },
    ResultSpec {
        name: "matched",
        ty: TypeDef::Scalar(ScalarType::Bool),
        hint: Some("Whether a branch condition matched (false if default was used)."),
    },
    ResultSpec {
        name: "branch_index",
        ty: TypeDef::Scalar(ScalarType::Number),
        hint: Some("Index of the matched branch (0-based), or -1 if default was used."),
    },
];

struct Branch {
    condition: String,
    then_value: String,
}

pub struct ConditionCommand {
    branches: Vec<Branch>,
    default: Option<String>,
}

#[async_trait::async_trait]
impl Executable for ConditionCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        tracing::info!(
            branch_count = self.branches.len(),
            has_default = self.default.is_some(),
            "Executing ConditionCommand"
        );

        // Evaluate branches in order
        for (index, branch) in self.branches.iter().enumerate() {
            // Evaluate the condition
            let condition_template = format!("{{{{ {} }}}}", branch.condition);
            let condition_result = context.substitute(&condition_template).await?;
            let condition_value = parse_scalar(&condition_result);

            if is_truthy(&condition_value) {
                // Condition matched - substitute and return the then_value
                let result = context.substitute(&branch.then_value).await?;

                context
                    .scalar()
                    .insert(
                        &output_prefix.with_segment("result"),
                        ScalarValue::String(result),
                    )
                    .await?;
                context
                    .scalar()
                    .insert(
                        &output_prefix.with_segment("matched"),
                        ScalarValue::Bool(true),
                    )
                    .await?;
                context
                    .scalar()
                    .insert(
                        &output_prefix.with_segment("branch_index"),
                        ScalarValue::Number((index as i64).into()),
                    )
                    .await?;

                return Ok(());
            }
        }

        // No branch matched - use default
        let result = match &self.default {
            Some(default_template) => context.substitute(default_template).await?,
            None => String::new(),
        };

        context
            .scalar()
            .insert(
                &output_prefix.with_segment("result"),
                ScalarValue::String(result),
            )
            .await?;
        context
            .scalar()
            .insert(
                &output_prefix.with_segment("matched"),
                ScalarValue::Bool(false),
            )
            .await?;
        context
            .scalar()
            .insert(
                &output_prefix.with_segment("branch_index"),
                ScalarValue::Number((-1_i64).into()),
            )
            .await?;

        Ok(())
    }
}

impl Descriptor for ConditionCommand {
    fn command_type() -> &'static str {
        "ConditionCommand"
    }
    fn available_attributes() -> &'static [AttributeSpec<&'static str>] {
        &CONDITIONCOMMAND_ATTRIBUTES
    }
    fn expected_outputs() -> &'static [ResultSpec<&'static str>] {
        CONDITIONCOMMAND_OUTPUTS
    }
}

impl FromAttributes for ConditionCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        // Parse branches array
        let branches_value = attrs
            .get("branches")
            .context("Missing required attribute 'branches'")?;

        let branches_array = branches_value
            .as_array()
            .context("Attribute 'branches' must be an array")?;

        let mut branches = Vec::with_capacity(branches_array.len());
        for (i, branch_value) in branches_array.iter().enumerate() {
            let branch_obj = branch_value
                .as_object()
                .context(format!("branches[{}] must be an object", i))?;

            let condition = branch_obj
                .get("if")
                .context(format!("branches[{}] missing 'if' field", i))?
                .as_str()
                .context(format!("branches[{}].if must be a string", i))?
                .to_string();

            let then_value = branch_obj
                .get("then")
                .context(format!("branches[{}] missing 'then' field", i))?
                .as_str()
                .context(format!("branches[{}].then must be a string", i))?
                .to_string();

            branches.push(Branch {
                condition,
                then_value,
            });
        }

        // Parse optional default
        let default = attrs
            .get("default")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(ConditionCommand { branches, default })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_branch(condition: &str, then_value: &str) -> Branch {
        Branch {
            condition: condition.to_string(),
            then_value: then_value.to_string(),
        }
    }

    async fn get_result(context: &ExecutionContext, prefix: &StorePath) -> (String, bool, i64) {
        let result = context
            .scalar()
            .get(&prefix.with_segment("result"))
            .await
            .unwrap()
            .unwrap();
        let matched = context
            .scalar()
            .get(&prefix.with_segment("matched"))
            .await
            .unwrap()
            .unwrap();
        let branch_index = context
            .scalar()
            .get(&prefix.with_segment("branch_index"))
            .await
            .unwrap()
            .unwrap();

        (
            result.as_str().unwrap().to_string(),
            matched.as_bool().unwrap(),
            branch_index.as_i64().unwrap(),
        )
    }

    #[tokio::test]
    async fn test_first_branch_matches() {
        init_tracing();
        let mut inputs = HashMap::new();
        inputs.insert(
            "status".to_string(),
            ScalarValue::String("active".to_string()),
        );
        let context = ExecutionContext::new(Some(&inputs));
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = ConditionCommand {
            branches: vec![
                make_branch("status == 'active'", "User is active"),
                make_branch("status == 'pending'", "User is pending"),
            ],
            default: Some("Unknown".to_string()),
        };

        cmd.execute(&context, &prefix).await.unwrap();
        let (result, matched, index) = get_result(&context, &prefix).await;

        assert_eq!(result, "User is active");
        assert!(matched);
        assert_eq!(index, 0);
    }

    #[tokio::test]
    async fn test_second_branch_matches() {
        init_tracing();
        let mut inputs = HashMap::new();
        inputs.insert(
            "status".to_string(),
            ScalarValue::String("pending".to_string()),
        );
        let context = ExecutionContext::new(Some(&inputs));
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = ConditionCommand {
            branches: vec![
                make_branch("status == 'active'", "User is active"),
                make_branch("status == 'pending'", "User is pending"),
            ],
            default: Some("Unknown".to_string()),
        };

        cmd.execute(&context, &prefix).await.unwrap();
        let (result, matched, index) = get_result(&context, &prefix).await;

        assert_eq!(result, "User is pending");
        assert!(matched);
        assert_eq!(index, 1);
    }

    #[tokio::test]
    async fn test_default_when_no_match() {
        init_tracing();
        let mut inputs = HashMap::new();
        inputs.insert(
            "status".to_string(),
            ScalarValue::String("unknown".to_string()),
        );
        let context = ExecutionContext::new(Some(&inputs));
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = ConditionCommand {
            branches: vec![
                make_branch("status == 'active'", "User is active"),
                make_branch("status == 'pending'", "User is pending"),
            ],
            default: Some("Unknown status".to_string()),
        };

        cmd.execute(&context, &prefix).await.unwrap();
        let (result, matched, index) = get_result(&context, &prefix).await;

        assert_eq!(result, "Unknown status");
        assert!(!matched);
        assert_eq!(index, -1);
    }

    #[tokio::test]
    async fn test_no_default_no_match_returns_empty() {
        init_tracing();
        let mut inputs = HashMap::new();
        inputs.insert(
            "status".to_string(),
            ScalarValue::String("unknown".to_string()),
        );
        let context = ExecutionContext::new(Some(&inputs));
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = ConditionCommand {
            branches: vec![make_branch("status == 'active'", "User is active")],
            default: None,
        };

        cmd.execute(&context, &prefix).await.unwrap();
        let (result, matched, index) = get_result(&context, &prefix).await;

        assert_eq!(result, "");
        assert!(!matched);
        assert_eq!(index, -1);
    }

    #[tokio::test]
    async fn test_tera_substitution_in_then_value() {
        init_tracing();
        let mut inputs = HashMap::new();
        inputs.insert("name".to_string(), ScalarValue::String("Alice".to_string()));
        inputs.insert("count".to_string(), ScalarValue::Number(42.into()));
        let context = ExecutionContext::new(Some(&inputs));
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = ConditionCommand {
            branches: vec![make_branch(
                "true",
                "Hello {{ name }}, you have {{ count }} items",
            )],
            default: None,
        };

        cmd.execute(&context, &prefix).await.unwrap();
        let (result, matched, _) = get_result(&context, &prefix).await;

        assert_eq!(result, "Hello Alice, you have 42 items");
        assert!(matched);
    }

    #[tokio::test]
    async fn test_tera_substitution_in_default() {
        init_tracing();
        let mut inputs = HashMap::new();
        inputs.insert(
            "status".to_string(),
            ScalarValue::String("weird".to_string()),
        );
        let context = ExecutionContext::new(Some(&inputs));
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = ConditionCommand {
            branches: vec![make_branch("status == 'active'", "Active")],
            default: Some("Unexpected status: {{ status }}".to_string()),
        };

        cmd.execute(&context, &prefix).await.unwrap();
        let (result, matched, _) = get_result(&context, &prefix).await;

        assert_eq!(result, "Unexpected status: weird");
        assert!(!matched);
    }

    #[tokio::test]
    async fn test_numeric_comparison() {
        let mut inputs = HashMap::new();
        inputs.insert("score".to_string(), ScalarValue::Number(85.into()));
        let context = ExecutionContext::new(Some(&inputs));
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = ConditionCommand {
            branches: vec![
                make_branch("score >= 90", "A"),
                make_branch("score >= 80", "B"),
                make_branch("score >= 70", "C"),
            ],
            default: Some("F".to_string()),
        };

        cmd.execute(&context, &prefix).await.unwrap();
        let (result, matched, index) = get_result(&context, &prefix).await;

        assert_eq!(result, "B");
        assert!(matched);
        assert_eq!(index, 1);
    }

    #[tokio::test]
    async fn test_empty_branches_uses_default() {
        init_tracing();
        let context = ExecutionContext::new(None);
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = ConditionCommand {
            branches: vec![],
            default: Some("No branches defined".to_string()),
        };

        cmd.execute(&context, &prefix).await.unwrap();
        let (result, matched, index) = get_result(&context, &prefix).await;

        assert_eq!(result, "No branches defined");
        assert!(!matched);
        assert_eq!(index, -1);
    }

    #[tokio::test]
    async fn test_from_attributes() {
        init_tracing();
        use tera::Map;

        // Build branch objects manually
        let mut branch1 = Map::new();
        branch1.insert("if".to_string(), ScalarValue::String("x > 5".to_string()));
        branch1.insert("then".to_string(), ScalarValue::String("big".to_string()));

        let mut branch2 = Map::new();
        branch2.insert("if".to_string(), ScalarValue::String("x <= 5".to_string()));
        branch2.insert("then".to_string(), ScalarValue::String("small".to_string()));

        let mut attrs = Attributes::new();
        attrs.insert(
            "branches".to_string(),
            ScalarValue::Array(vec![
                ScalarValue::Object(branch1),
                ScalarValue::Object(branch2),
            ]),
        );
        attrs.insert(
            "default".to_string(),
            ScalarValue::String("unknown".to_string()),
        );

        let cmd = ConditionCommand::from_attributes(&attrs).unwrap();

        assert_eq!(cmd.branches.len(), 2);
        assert_eq!(cmd.branches[0].condition, "x > 5");
        assert_eq!(cmd.branches[0].then_value, "big");
        assert_eq!(cmd.branches[1].condition, "x <= 5");
        assert_eq!(cmd.branches[1].then_value, "small");
        assert_eq!(cmd.default, Some("unknown".to_string()));
    }

    #[tokio::test]
    async fn test_factory_builds_and_executes() {
        init_tracing();
        use tera::Map;

        // Build valid attributes
        let mut branch = Map::new();
        branch.insert(
            "if".to_string(),
            ScalarValue::String("value > 10".to_string()),
        );
        branch.insert(
            "then".to_string(),
            ScalarValue::String("big number".to_string()),
        );

        let mut attrs = Attributes::new();
        attrs.insert(
            "branches".to_string(),
            ScalarValue::Array(vec![ScalarValue::Object(branch)]),
        );
        attrs.insert(
            "default".to_string(),
            ScalarValue::String("small number".to_string()),
        );

        // Use factory to build the command
        let factory = ConditionCommand::factory();
        let executable = factory(&attrs).expect("Factory should succeed with valid attributes");

        // Execute the command
        let mut inputs = HashMap::new();
        inputs.insert("value".to_string(), ScalarValue::Number(25.into()));
        let context = ExecutionContext::new(Some(&inputs));
        let prefix = StorePath::from_segments(["ns", "factory_test"]);

        executable.execute(&context, &prefix).await.unwrap();
        let (result, matched, _) = get_result(&context, &prefix).await;

        assert_eq!(result, "big number");
        assert!(matched);
    }

    #[tokio::test]
    async fn test_factory_rejects_missing_required_attribute() {
        init_tracing();

        // Empty attributes - missing required 'branches'
        let attrs = Attributes::new();

        let factory = ConditionCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing attribute"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required attribute 'branches'"),
                    "Expected missing attribute error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_wrong_type() {
        init_tracing();

        // branches should be an array, not a string
        let mut attrs = Attributes::new();
        attrs.insert(
            "branches".to_string(),
            ScalarValue::String("not an array".to_string()),
        );

        let factory = ConditionCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with wrong type"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("must be an array"),
                    "Expected type error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_invalid_branch_object() {
        init_tracing();
        use tera::Map;

        // Branch missing required 'then' field
        let mut branch = Map::new();
        branch.insert("if".to_string(), ScalarValue::String("true".to_string()));
        // Missing 'then' field

        let mut attrs = Attributes::new();
        attrs.insert(
            "branches".to_string(),
            ScalarValue::Array(vec![ScalarValue::Object(branch)]),
        );

        let factory = ConditionCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing field"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required field 'branches[0].then'"),
                    "Expected missing field error, got: {}",
                    msg
                );
            }
        }
    }
}
