use crate::imports::*;

static CONDITIONCOMMAND_SPEC: CommandSchema = LazyLock::new(|| {
    let (pending, fields) = CommandSpecBuilder::new().array_of_objects(
        "branches",
        true,
        Some("Array of {name, if, then} objects evaluated in order"),
    );

    let (fields, name_ref) = fields.add_literal(
        "name",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Unique identifier for this branch"),
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
        .attribute(
            AttributeSpecBuilder::new("default", TypeDef::Scalar(ScalarType::String))
                .hint("Default value if no branch matches (supports Tera substitution)")
                .reference(ReferenceKind::StaticTeraTemplate)
                .build(),
        )
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
        .derived_result("branches", name_ref, None, ResultKind::Data)
        .build()
});

struct Branch {
    name: String,
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
        let out = InsertBatch::new(context, output_prefix);
        let mut first_match: Option<(usize, String)> = None;

        // Evaluate all branches and write per-branch result objects
        for (index, branch) in self.branches.iter().enumerate() {
            let condition_template = format!("{{{{ {} }}}}", branch.condition);
            let condition_result = context.substitute(&condition_template).await?;
            let condition_value = parse_scalar(&condition_result);
            let matched = is_truthy(&condition_value);

            let value = if matched {
                let result = context.substitute(&branch.then_value).await?;
                if first_match.is_none() {
                    first_match = Some((index, result.clone()));
                }
                result
            } else {
                String::new()
            };

            let branch_obj = ObjectBuilder::new()
                .insert("matched", matched)
                .insert("value", value)
                .build_scalar();
            out.scalar(&branch.name, branch_obj).await?;
        }

        // Write summary fixed results
        match first_match {
            Some((index, result)) => {
                out.string("result", result).await?;
                out.bool("matched", true).await?;
                out.i64("branch_index", index as i64).await?;
            }
            None => {
                let result = match &self.default {
                    Some(default_template) => context.substitute(default_template).await?,
                    None => String::new(),
                };
                out.string("result", result).await?;
                out.bool("matched", false).await?;
                out.i64("branch_index", -1).await?;
            }
        }

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

            let name = branch_obj
                .get_required_string("name")
                .context(format!("branches[{}]", i))?;
            let condition = branch_obj
                .get_required_string("if")
                .context(format!("branches[{}]", i))?;
            let then_value = branch_obj
                .get_required_string("then")
                .context(format!("branches[{}]", i))?;

            branches.push(Branch {
                name,
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
    // Going to redo these.
}
