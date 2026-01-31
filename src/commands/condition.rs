use crate::imports::*;

static CONDITIONCOMMAND_SPEC: CommandSchema = LazyLock::new(|| {
    let (pending, fields) = CommandSpecBuilder::new().array_of_objects(
        "branches",
        true,
        Some("Array of {if, then} objects evaluated in order"),
    );

    let fields = fields.add_template(
        "if",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Tera expression to evaluate as condition"),
        ReferenceKind::RuntimeTeraTemplate,
    );
    let fields = fields.add_template(
        "then",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Value if condition is true (supports Tera substitution)"),
        ReferenceKind::StaticTeraTemplate,
    );

    pending
        .finalise_attribute(fields)
        .attribute(AttributeSpec {
            name: "default",
            ty: TypeDef::Scalar(ScalarType::String),
            required: false,
            hint: Some("Default value if no branch matches (supports Tera substitution)"),
            default_value: None,
            reference_kind: ReferenceKind::StaticTeraTemplate,
        })
        .fixed_result(
            "result",
            TypeDef::Scalar(ScalarType::String),
            Some("The value from the matched branch or default."),
            ResultKind::Data,
        )
        .fixed_result(
            "matched",
            TypeDef::Scalar(ScalarType::Bool),
            Some("Whether a branch condition matched (false if default was used)."),
            ResultKind::Data,
        )
        .fixed_result(
            "branch_index",
            TypeDef::Scalar(ScalarType::Number),
            Some("Index of the matched branch (0-based), or -1 if default was used."),
            ResultKind::Data,
        )
        .build()
});

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

        let out = InsertBatch::new(context, output_prefix);

        // Evaluate branches in order
        for (index, branch) in self.branches.iter().enumerate() {
            // Evaluate the condition
            let condition_template = format!("{{{{ {} }}}}", branch.condition);
            let condition_result = context.substitute(&condition_template).await?;
            let condition_value = parse_scalar(&condition_result);

            if is_truthy(&condition_value) {
                // Condition matched - substitute and return the then_value
                let result = context.substitute(&branch.then_value).await?;

                out.string("result", result).await?;
                out.bool("matched", true).await?;
                out.i64("branch_index", index as i64).await?;

                return Ok(());
            }
        }

        // No branch matched - use default
        let result = match &self.default {
            Some(default_template) => context.substitute(default_template).await?,
            None => String::new(),
        };

        out.string("result", result).await?;
        out.bool("matched", false).await?;
        out.i64("branch_index", -1).await?;

        Ok(())
    }
}

impl Descriptor for ConditionCommand {
    fn command_type() -> &'static str {
        "ConditionCommand"
    }
    fn command_attributes() -> &'static [AttributeSpec<&'static str>] {
        &CONDITIONCOMMAND_SPEC.0
    }
    fn command_results() -> &'static [ResultSpec<&'static str>] {
        &CONDITIONCOMMAND_SPEC.1
    }
}

impl FromAttributes for ConditionCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        let branches_array = attrs
            .get_required("branches")?
            .as_array_or_err("branches")?;

        let mut branches = Vec::with_capacity(branches_array.len());
        for (i, branch_value) in branches_array.iter().enumerate() {
            let branch_obj = branch_value.as_object_or_err(&format!("branches[{}]", i))?;

            let condition = branch_obj
                .get_required_string("if")
                .context(format!("branches[{}]", i))?;
            let then_value = branch_obj
                .get_required_string("then")
                .context(format!("branches[{}]", i))?;

            branches.push(Branch {
                condition,
                then_value,
            });
        }

        let default = attrs.get_optional_string("default");

        Ok(ConditionCommand { branches, default })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_tracing;

    fn make_branch(condition: &str, then_value: &str) -> Branch {
        Branch {
            condition: condition.to_string(),
            then_value: then_value.to_string(),
        }
    }

    /// Helper to build condition command attributes
    fn condition_attrs(branches: Vec<Branch>, default: Option<&str>) -> Attributes {
        let branch_values: Vec<ScalarValue> = branches
            .into_iter()
            .map(|b| {
                ObjectBuilder::new()
                    .insert("if", b.condition)
                    .insert("then", b.then_value)
                    .build_scalar()
            })
            .collect();

        let mut builder =
            ObjectBuilder::new().insert("branches", ScalarValue::Array(branch_values));
        if let Some(d) = default {
            builder = builder.insert("default", d);
        }
        builder.build_hashmap()
    }

    async fn get_result(
        context: &ExecutionContext,
        namespace: &str,
        command: &str,
    ) -> (String, bool, i64) {
        let prefix = StorePath::from_segments([namespace, command]);
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
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("status", ScalarValue::String("active".to_string())),
            )
            .unwrap();

        let attrs = condition_attrs(
            vec![
                make_branch("inputs.status == 'active'", "User is active"),
                make_branch("inputs.status == 'pending'", "User is pending"),
            ],
            Some("Unknown"),
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<ConditionCommand>("check", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();
        let (result, matched, index) = get_result(&context, "exec", "check").await;

        assert_eq!(result, "User is active");
        assert!(matched);
        assert_eq!(index, 0);
    }

    #[tokio::test]
    async fn test_second_branch_matches() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("status", ScalarValue::String("pending".to_string())),
            )
            .unwrap();

        let attrs = condition_attrs(
            vec![
                make_branch("inputs.status == 'active'", "User is active"),
                make_branch("inputs.status == 'pending'", "User is pending"),
            ],
            Some("Unknown"),
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<ConditionCommand>("check", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();
        let (result, matched, index) = get_result(&context, "exec", "check").await;

        assert_eq!(result, "User is pending");
        assert!(matched);
        assert_eq!(index, 1);
    }

    #[tokio::test]
    async fn test_default_when_no_match() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("status", ScalarValue::String("unknown".to_string())),
            )
            .unwrap();

        let attrs = condition_attrs(
            vec![
                make_branch("inputs.status == 'active'", "User is active"),
                make_branch("inputs.status == 'pending'", "User is pending"),
            ],
            Some("Unknown status"),
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<ConditionCommand>("check", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();
        let (result, matched, index) = get_result(&context, "exec", "check").await;

        assert_eq!(result, "Unknown status");
        assert!(!matched);
        assert_eq!(index, -1);
    }

    #[tokio::test]
    async fn test_no_default_no_match_returns_empty() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("status", ScalarValue::String("unknown".to_string())),
            )
            .unwrap();

        let attrs = condition_attrs(
            vec![make_branch("inputs.status == 'active'", "User is active")],
            None,
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<ConditionCommand>("check", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();
        let (result, matched, index) = get_result(&context, "exec", "check").await;

        assert_eq!(result, "");
        assert!(!matched);
        assert_eq!(index, -1);
    }

    #[tokio::test]
    async fn test_tera_substitution_in_then_value() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("name", ScalarValue::String("Alice".to_string()))
                    .insert("count", ScalarValue::Number(42.into())),
            )
            .unwrap();

        let attrs = condition_attrs(
            vec![make_branch(
                "true",
                "Hello {{ inputs.name }}, you have {{ inputs.count }} items",
            )],
            None,
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<ConditionCommand>("check", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();
        let (result, matched, _) = get_result(&context, "exec", "check").await;

        assert_eq!(result, "Hello Alice, you have 42 items");
        assert!(matched);
    }

    #[tokio::test]
    async fn test_tera_substitution_in_default() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("status", ScalarValue::String("weird".to_string())),
            )
            .unwrap();

        let attrs = condition_attrs(
            vec![make_branch("inputs.status == 'active'", "Active")],
            Some("Unexpected status: {{ inputs.status }}"),
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<ConditionCommand>("check", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();
        let (result, matched, _) = get_result(&context, "exec", "check").await;

        assert_eq!(result, "Unexpected status: weird");
        assert!(!matched);
    }

    #[tokio::test]
    async fn test_numeric_comparison() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("score", ScalarValue::Number(85.into())),
            )
            .unwrap();

        let attrs = condition_attrs(
            vec![
                make_branch("inputs.score >= 90", "A"),
                make_branch("inputs.score >= 80", "B"),
                make_branch("inputs.score >= 70", "C"),
            ],
            Some("F"),
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<ConditionCommand>("check", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();
        let (result, matched, index) = get_result(&context, "exec", "check").await;

        assert_eq!(result, "B");
        assert!(matched);
        assert_eq!(index, 1);
    }

    #[tokio::test]
    async fn test_empty_branches_uses_default() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        let attrs = condition_attrs(vec![], Some("No branches defined"));

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<ConditionCommand>("check", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();
        let (result, matched, index) = get_result(&context, "exec", "check").await;

        assert_eq!(result, "No branches defined");
        assert!(!matched);
        assert_eq!(index, -1);
    }

    #[tokio::test]
    async fn test_from_attributes() {
        init_tracing();

        // Build branch objects using ObjectBuilder
        let branch1 = ObjectBuilder::new()
            .insert("if", "x > 5")
            .insert("then", "big")
            .build_scalar();

        let branch2 = ObjectBuilder::new()
            .insert("if", "x <= 5")
            .insert("then", "small")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("branches", ScalarValue::Array(vec![branch1, branch2]))
            .insert("default", "unknown")
            .build_hashmap();

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

        let attrs = condition_attrs(
            vec![make_branch("inputs.value > 10", "big number")],
            Some("small number"),
        );

        // Test that factory() returns a valid builder
        let factory = ConditionCommand::factory();
        let _executable = factory(&attrs).expect("Factory should succeed with valid attributes");

        // Now use Pipeline::execute() pattern to actually run
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("value", ScalarValue::Number(25.into())),
            )
            .unwrap();

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<ConditionCommand>("factory_test", &attrs)
            .unwrap();

        let completed = pipeline.compile().unwrap().execute().await.unwrap();
        let context = completed.context();
        let (result, matched, _) = get_result(&context, "exec", "factory_test").await;

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

        // Branch missing required 'then' field
        let branch = ObjectBuilder::new()
            .insert("if", "true")
            // Missing 'then' field
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("branches", ScalarValue::Array(vec![branch]))
            .build_hashmap();

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
