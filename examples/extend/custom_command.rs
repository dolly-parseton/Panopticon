//! Example: Implementing a custom Command
//!
//! Demonstrates how to implement the Command trait (Descriptor + FromAttributes +
//! Executable) to create a custom command that integrates with panopticon pipelines.
//!
//! The example command reverses a string and writes results using InsertBatch.
//!
//! Run with: cargo run --example custom_command

use panopticon_core::extend::*;
use panopticon_core::prelude::*;

// ─── Step 1: Define the CommandSchema ───────────────────────────────────────
//
// A CommandSchema is a LazyLock<(Vec<AttributeSpec>, Vec<ResultSpec>)>.
// Use CommandSpecBuilder to construct it. Validation (NamePolicy checks,
// derived result integrity) runs once at first access.

static REVERSE_SPEC: CommandSchema = LazyLock::new(|| {
    CommandSpecBuilder::new()
        .attribute(
            AttributeSpecBuilder::new("input", TypeDef::Scalar(ScalarType::String))
                .required()
                .hint("String to reverse (supports Tera template substitution)")
                .reference(ReferenceKind::StaticTeraTemplate)
                .build(),
        )
        .fixed_result(
            "reversed",
            TypeDef::Scalar(ScalarType::String),
            Some("The reversed string"),
            ResultKind::Data,
        )
        .fixed_result(
            "length",
            TypeDef::Scalar(ScalarType::Number),
            Some("Character count of the input"),
            ResultKind::Meta,
        )
        .build()
});

// ─── Step 2: Define the command struct ──────────────────────────────────────
//
// Fields hold the parsed attribute values extracted in FromAttributes.

pub struct ReverseCommand {
    input: String,
}

// ─── Step 3: Implement Descriptor ───────────────────────────────────────────
//
// Delegates to the static spec. The Command trait blanket impl requires this.

impl Descriptor for ReverseCommand {
    fn command_type() -> &'static str {
        "ReverseCommand"
    }
    fn command_attributes() -> &'static [AttributeSpec<&'static str>] {
        &REVERSE_SPEC.0
    }
    fn command_results() -> &'static [ResultSpec<&'static str>] {
        &REVERSE_SPEC.1
    }
}

// ─── Step 4: Implement FromAttributes ───────────────────────────────────────
//
// Extracts attribute values from the Attributes map. The ScalarMapExt trait
// (implemented on HashMap<String, ScalarValue>) provides typed getters like
// get_required_string.

impl FromAttributes for ReverseCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        let input = attrs.get_required_string("input")?;
        Ok(ReverseCommand { input })
    }
}

// ─── Step 5: Implement Executable ───────────────────────────────────────────
//
// The async execute method receives:
//   - context: ExecutionContext with scalar/tabular stores and template substitution
//   - output_prefix: StorePath where this command should write its results
//
// Use InsertBatch for convenient typed inserts under the output prefix.
// Use context.substitute() to resolve Tera templates in attribute values.

#[async_trait]
impl Executable for ReverseCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        // Resolve any Tera templates in the input (e.g. "{{ inputs.greeting }}")
        let resolved = context.substitute(&self.input).await?;

        let reversed: String = resolved.chars().rev().collect();
        let length = resolved.chars().count() as u64;

        // InsertBatch writes values under output_prefix.<segment>
        let out = InsertBatch::new(context, output_prefix);
        out.string("reversed", reversed).await?;
        out.u64("length", length).await?;

        Ok(())
    }
}

// ─── Pipeline demo ──────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Seed a value into the context via a static namespace
    let mut pipeline = Pipeline::new();

    pipeline
        .add_namespace(
            NamespaceBuilder::new("inputs")
                .static_ns()
                .insert("greeting", ScalarValue::String("Hello, world!".to_string())),
        )
        .await?;

    // Use the custom command with a Tera template referencing the static value
    let attrs = ObjectBuilder::new()
        .insert("input", "{{ inputs.greeting }}")
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("demo"))
        .await?
        .add_command::<ReverseCommand>("reverse", &attrs)
        .await?;

    // Execute the pipeline
    let completed = pipeline.compile().await?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // Retrieve and print results
    let source = StorePath::from_segments(["demo", "reverse"]);
    let cmd_results = results
        .get_by_source(&source)
        .expect("Expected demo.reverse results");

    let reversed = cmd_results
        .data_get(&source.with_segment("reversed"))
        .and_then(|r| r.as_scalar())
        .expect("Expected reversed result");
    println!("Original: Hello, world!");
    println!("Reversed: {}", reversed.1);

    let length = cmd_results
        .meta_get(&source.with_segment("length"))
        .expect("Expected length metadata");
    println!("Length:   {}", length);

    let status = cmd_results
        .meta_get(&source.with_segment("status"))
        .expect("Expected status metadata");
    println!("Status:   {}", status);

    Ok(())
}
