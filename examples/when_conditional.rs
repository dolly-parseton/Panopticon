//! Example: Using the `when` conditional attribute on commands
//!
//! This example demonstrates how the `when` attribute can be used to conditionally
//! execute commands based on values in the execution store at runtime.
//!
//! Run with: cargo run --example when_conditional

use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        )
        .try_init()
        .ok();

    println!("=== Example 1: Condition is TRUE ===");
    run_with_feature_flag(true).await?;

    println!();

    println!("=== Example 2: Condition is FALSE ===");
    run_with_feature_flag(false).await?;

    Ok(())
}

async fn run_with_feature_flag(feature_enabled: bool) -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    pipeline.add_namespace(
        NamespaceBuilder::new("inputs")
            .static_ns()
            .insert("feature_enabled", ScalarValue::Bool(feature_enabled))
            .insert("user_name", ScalarValue::String("Alice".to_string())),
    )?;

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

    pipeline
        .add_namespace(NamespaceBuilder::new("example"))
        .unwrap()
        .add_command::<ConditionCommand>("greeting", &attrs)?;

    // Compile and execute the pipeline
    let completed = pipeline.compile()?.execute().await?;

    // Collect results via ResultStore
    let results = completed.results(ResultSettings::default()).await?;
    let source = StorePath::from_segments(["example", "greeting"]);

    match results.get_by_source(&source) {
        Some(cmd_results) => {
            // COMMON_RESULTS provides status meta on every command
            let status_path = source.with_segment("status");
            let status = cmd_results.meta_get(&status_path);
            println!("  status = {:?}", status);

            let result_path = source.with_segment("result");
            if let Some(result_value) = cmd_results.data_get(&result_path) {
                let (_ty, value) = result_value.as_scalar().unwrap();
                println!("  result = {}", value);
            } else {
                println!("  Command was skipped (when condition was false)");
            }
        }
        None => {
            println!("No results found for example.greeting");
        }
    }

    Ok(())
}
