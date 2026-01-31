//! Example: Using the `when` attribute to conditionally skip commands
//!
//! Every command supports a `when` attribute — a Tera expression evaluated at
//! runtime. If it resolves to a falsy value the command is skipped entirely and
//! its results are absent from the ResultStore.
//!
//! This example runs the same pipeline twice with different feature flags to
//! show both the executed and skipped paths.
//!
//! Run with: cargo run --example when_conditional

use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Feature flag: TRUE ===");
    run_with_feature_flag(true).await?;

    println!("\n=== Feature flag: FALSE ===");
    run_with_feature_flag(false).await?;

    Ok(())
}

async fn run_with_feature_flag(enabled: bool) -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // --- Static namespace: feature flag + user data ---
    pipeline.add_namespace(
        NamespaceBuilder::new("inputs")
            .static_ns()
            .insert("feature_enabled", ScalarValue::Bool(enabled))
            .insert("user_name", ScalarValue::String("Alice".to_string())),
    )?;

    // --- Command with `when` guard ---
    // The command is only executed when inputs.feature_enabled is truthy.
    let attrs = ObjectBuilder::new()
        .insert("when", "inputs.feature_enabled")
        .insert(
            "branches",
            ScalarValue::Array(vec![ObjectBuilder::new()
                .insert("name", "greeting")
                .insert("if", "true")
                .insert("then", "Hello, {{ inputs.user_name }}! Feature is active.")
                .build_scalar()]),
        )
        .insert("default", "Fallback message")
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("example"))?
        .add_command::<ConditionCommand>("greeting", &attrs)?;

    // --- Execute ---
    let completed = pipeline.compile()?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // --- Inspect results ---
    let source = StorePath::from_segments(["example", "greeting"]);
    let cmd_results = results
        .get_by_source(&source)
        .expect("Expected example.greeting results");

    let status = cmd_results
        .meta_get(&source.with_segment("status"))
        .expect("Expected status");
    println!("  status = {}", status);

    // When the `when` condition is false the command is skipped:
    // status is "skipped" and data results are absent.
    if let Some(result) = cmd_results
        .data_get(&source.with_segment("result"))
        .and_then(|r| r.as_scalar())
    {
        println!("  result = {}", result.1);
    } else {
        println!("  (no data — command was skipped)");
    }

    Ok(())
}
