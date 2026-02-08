use crate::imports::*;

use super::validation::validate_attributes;

/*
    Types:
    * CommandFactory - Factory function type for creating command instances from attributes
    * ExecutableWrapper - Wrapper around Executable trait objects to handle common attributes like 'when
*/
pub type CommandFactory = Box<dyn Fn(&Attributes) -> Result<Box<dyn Executable>>>;

struct ExecutableWrapper {
    inner: Box<dyn Executable>,
    when: Option<String>,
}

// Wrapper to handle 'when' conditional before executing the inner command
// ^ Again might extend later with more common functionality
#[async_trait::async_trait]
impl Executable for ExecutableWrapper {
    #[tracing::instrument(skip(self, context, output_prefix), err, fields(
        command_output = %output_prefix.to_dotted(),
    ))]
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        // Capture Instant::now() before execution starts
        let start_time = std::time::Instant::now();

        let batch = InsertBatch::new(context, output_prefix);

        // Evaluate when condition
        if let Some(condition) = &self.when {
            let template = format!("{{{{ {} }}}}", condition);
            let result = context.substitute(&template).await?;
            if !is_truthy(&parse_scalar(&result)) {
                tracing::debug!("Skipping command - 'when' condition is false");
                batch
                    .string("status", EXECUTION_STATUS_SKIPPED.to_string())
                    .await?;
                batch
                    .u64("duration_ms", start_time.elapsed().as_millis() as u64)
                    .await?;
                return Ok(());
            }
        }

        // Check for cancellation
        if context.extensions().is_canceled().await {
            tracing::debug!("Skipping command - cancelled");
            batch
                .string("status", EXECUTION_STATUS_CANCELLED.to_string())
                .await?;
            batch
                .u64("duration_ms", start_time.elapsed().as_millis() as u64)
                .await?;
            return Ok(());
        }

        let result = self.inner.execute(context, output_prefix).await;
        let duration = start_time.elapsed().as_millis() as u64;
        batch.u64("duration_ms", duration).await?;
        // Set status based on execution result
        match &result {
            Ok(_) => {
                batch
                    .string("status", EXECUTION_STATUS_SUCCESS.to_string())
                    .await?;
            }
            Err(_) => {
                batch
                    .string("status", EXECUTION_STATUS_ERROR.to_string())
                    .await?;
            }
        }
        tracing::debug!(
            command_output = %output_prefix.to_dotted(),
            duration_ms = duration,
            "Command execution complete"
        );
        // Return original execution result
        result
    }
}

/*
    Consts
    * COMMON_ATTRIBUTES - Common attributes shared by all commands
    * COMMON_RESULTS - Common results shared by all commands
    * STATUS constants - Standardized execution status strings

*/
pub const COMMON_ATTRIBUTES: &[AttributeSpec<&'static str>] = &[AttributeSpec {
    name: "when",
    ty: TypeDef::Scalar(ScalarType::String),
    required: false,
    hint: Some("Evaluates a tera conditional to determine if the command should run"),
    default_value: None,
    reference_kind: ReferenceKind::RuntimeTeraTemplate,
}];

pub const EXECUTION_STATUS_SUCCESS: &str = "success";
pub const EXECUTION_STATUS_SKIPPED: &str = "skipped";
pub const EXECUTION_STATUS_ERROR: &str = "error";
pub const EXECUTION_STATUS_CANCELLED: &str = "cancelled";

pub const COMMON_RESULTS: &[ResultSpec<&'static str>] = &[
    ResultSpec::Field {
        name: "duration_ms",
        ty: TypeDef::Scalar(ScalarType::Number),
        kind: ResultKind::Meta,
        hint: Some("Execution duration of the command in milliseconds"),
    },
    ResultSpec::Field {
        name: "status",
        ty: TypeDef::Scalar(ScalarType::String),
        kind: ResultKind::Meta,
        hint: Some(
            "Execution status of the command: 'success', 'skipped', 'error', or 'cancelled'",
        ),
    },
];

/*
    Traits:
    * Command - Marker trait for commands implementing FromAttributes, Descriptor, and Executable
    * FromAttributes - Trait for constructing command instances from attributes
    * Descriptor - Trait for providing command metadata like type, attributes, and outputs
    * Executable - Async trait for executing commands within an execution context
*/

pub trait Command: FromAttributes + Descriptor + Executable {}
// Blanket implementation for any type that implements the required traits
impl<T: FromAttributes + Descriptor + Executable> Command for T {}

// Todo - Add typed builder derive macro that accepts the Command struct and maybe spec.
pub trait FromAttributes: Sized + Descriptor {
    fn from_attributes(attrs: &Attributes) -> Result<Self>;

    fn extract_dependencies(attrs: &Attributes) -> Result<std::collections::HashSet<StorePath>> {
        use crate::dependencies::helpers;
        // Todo change to use available attributes? Current approach wont consider common attributes
        helpers::extract_dependencies_from_spec(attrs, Self::command_attributes())
    }

    fn factory() -> CommandFactory
    where
        Self: Executable + Descriptor + 'static,
    {
        Box::new(|attrs| {
            validate_attributes(attrs, Self::available_attributes())?;

            let when = attrs.get("when").and_then(|v| v.as_str()).map(String::from);

            let instance = Self::from_attributes(attrs)?;
            let wrapped = ExecutableWrapper {
                inner: Box::new(instance),
                when,
            };
            Ok(Box::new(wrapped) as Box<dyn Executable>)
        })
    }
}

pub trait Descriptor: Sized {
    fn command_type() -> &'static str;
    fn command_attributes() -> &'static [AttributeSpec<&'static str>];
    fn command_results() -> &'static [ResultSpec<&'static str>];
    // Defaults - Attributes
    fn available_attributes() -> Vec<&'static AttributeSpec<&'static str>> {
        let mut attrs = Vec::new();
        attrs.extend(COMMON_ATTRIBUTES.iter());
        attrs.extend(Self::command_attributes().iter());
        attrs
    }
    fn required_attributes() -> Vec<&'static AttributeSpec<&'static str>> {
        Self::command_attributes()
            .iter()
            .filter(|attr| attr.required)
            .collect()
    }
    fn optional_attributes() -> Vec<&'static AttributeSpec<&'static str>> {
        Self::command_attributes()
            .iter()
            .filter(|attr| !attr.required)
            .collect()
    }
    // Defaults - Results
    fn available_results() -> Vec<&'static ResultSpec<&'static str>> {
        let mut results = Vec::new();
        results.extend(COMMON_RESULTS.iter());
        results.extend(Self::command_results().iter());
        results
    }
}

#[async_trait::async_trait]
pub trait Executable: Send + Sync + 'static {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()>;
}
