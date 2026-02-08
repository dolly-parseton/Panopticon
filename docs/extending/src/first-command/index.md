# Your First Custom Command

This tutorial walks you through building a complete custom command from scratch. By the end, you will understand how Panopticon's command system works and be able to create your own commands for any use case.

## What We're Building

We'll create a `ReverseCommand` that takes a string input and produces two outputs:

1. **reversed** - The input string with its characters reversed
2. **length** - The character count of the original input

This simple example covers all the essential concepts:

- Defining a command schema with attributes and results
- Implementing the three required traits (`Descriptor`, `FromAttributes`, `Executable`)
- Using Tera template substitution for dynamic inputs
- Writing results using `InsertBatch`
- Integrating the command into a pipeline

## Prerequisites

Before starting, ensure you have:

- Basic familiarity with Rust (structs, traits, async/await)
- A Rust project with `panopticon-core` as a dependency
- Understanding of what commands and pipelines are (see the Guide)

## Command Architecture Overview

Every Panopticon command consists of three parts:

```text
┌─────────────────────────────────────────────────────────────────┐
│                        Custom Command                           │
├─────────────────────────────────────────────────────────────────┤
│  1. Schema (CommandSchema)                                      │
│     - Defines what attributes the command accepts               │
│     - Defines what results the command produces                 │
│     - Validated at initialization time                          │
├─────────────────────────────────────────────────────────────────┤
│  2. Struct + Traits                                             │
│     - Descriptor: Links struct to schema                        │
│     - FromAttributes: Parses attributes into struct fields      │
│     - Executable: Performs the actual work                      │
├─────────────────────────────────────────────────────────────────┤
│  3. Integration                                                 │
│     - Added to namespaces via add_command::<T>()                │
│     - Receives ExecutionContext at runtime                      │
│     - Writes results to the store                               │
└─────────────────────────────────────────────────────────────────┘
```

## Tutorial Structure

This tutorial is divided into three sections:

1. **[Defining the Schema](./defining-schema.md)** - Create the command specification using `CommandSpecBuilder` and `AttributeSpecBuilder`

2. **[Implementing Traits](./implementing-traits.md)** - Implement `Descriptor`, `FromAttributes`, and `Executable` traits

3. **[Testing Your Command](./testing-command.md)** - Integrate the command into a pipeline and verify it works

## The Complete Example

Here's a preview of what we'll build. Don't worry if it looks complex - we'll explain every line:

```rust
use panopticon_core::extend::*;
use panopticon_core::prelude::*;

// Step 1: Define the schema
static REVERSE_SPEC: CommandSchema = LazyLock::new(|| {
    CommandSpecBuilder::new()
        .attribute(
            AttributeSpecBuilder::new("input", TypeDef::Scalar(ScalarType::String))
                .required()
                .hint("String to reverse")
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
            Some("Character count"),
            ResultKind::Meta,
        )
        .build()
});

// Step 2: Define the struct
pub struct ReverseCommand {
    input: String,
}

// Step 3: Implement Descriptor
impl Descriptor for ReverseCommand {
    fn command_type() -> &'static str { "ReverseCommand" }
    fn command_attributes() -> &'static [AttributeSpec<&'static str>] { &REVERSE_SPEC.0 }
    fn command_results() -> &'static [ResultSpec<&'static str>] { &REVERSE_SPEC.1 }
}

// Step 4: Implement FromAttributes
impl FromAttributes for ReverseCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        let input = attrs.get_required_string("input")?;
        Ok(ReverseCommand { input })
    }
}

// Step 5: Implement Executable
#[async_trait]
impl Executable for ReverseCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        let resolved = context.substitute(&self.input).await?;
        let reversed: String = resolved.chars().rev().collect();
        let length = resolved.chars().count() as u64;

        let out = InsertBatch::new(context, output_prefix);
        out.string("reversed", reversed).await?;
        out.u64("length", length).await?;

        Ok(())
    }
}
```

Let's break this down step by step, starting with [defining the schema](./defining-schema.md).
