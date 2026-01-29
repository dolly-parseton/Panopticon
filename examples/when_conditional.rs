//! Example: Using the `when` conditional attribute on commands
//!
//! This example demonstrates how the `when` attribute can be used to conditionally
//! execute commands based on values in the execution store at runtime.
//!
//! Run with: cargo run --example when_conditional

use panopticon_core::prelude::*;
use tokio::net::unix::pipe;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init()
        .ok();

    // Example 1: Command runs when condition is true
    println!("=== Example 1: Condition is TRUE ===");
    run_with_feature_flag(true).await?;

    println!();

    // Example 2: Command is skipped when condition is false
    println!("=== Example 2: Condition is FALSE ===");
    run_with_feature_flag(false).await?;

    Ok(())
}

async fn run_with_feature_flag(feature_enabled: bool) -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // Add a static namespace for inputs - this simulates runtime configuration
    pipeline.add_namespace(
        NamespaceBuilder::new("inputs")
            .static_ns()
            .insert("feature_enabled", ScalarValue::Bool(feature_enabled))
            .insert("user_name", ScalarValue::String("Alice".to_string()))
            .build()?,
    )?;

    // Build ConditionCommand attributes with the `when` attribute
    // The command will only execute if `inputs.feature_enabled` is truthy
    let attrs = ObjectBuilder::new()
        .insert("when", "inputs.feature_enabled")
        .insert(
            "branches",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("if", "true")
                    .insert("then", "Hello, {{ inputs.user_name }}! Feature is active.")
                    .build_scalar(),
            ]),
        )
        .insert("default", "Fallback message")
        .build_hashmap();

    // Add execution namespace and command
    pipeline
        .add_namespace(NamespaceBuilder::new("example").build()?)
        .unwrap()
        .add_command::<ConditionCommand>("greeting", &attrs)?;

    // Execute all commands
    let context: ExecutionContext = pipeline.execute().await?;

    // Check if any output was produced
    let prefix = StorePath::from_segments(["example", "greeting"]);
    let result = context.scalar().get(&prefix.with_segment("result")).await?;

    match result {
        Some(value) => {
            println!("Command executed! Result: {}", value);
        }
        None => {
            println!("Command was skipped (when condition was false)");
        }
    }

    Ok(())
}
