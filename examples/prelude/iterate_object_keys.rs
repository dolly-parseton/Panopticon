//! Example: Iterating over object keys with ScalarObjectKeys
//!
//! Demonstrates an iterative namespace that loops over the keys of a JSON object.
//! Each iteration receives the key name as the `item` variable, which can be used
//! in Tera templates and condition expressions.
//!
//! Run with: cargo run --example iterate_object_keys

use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    // --- Static namespace: an object whose keys we will iterate ---
    pipeline.add_namespace(
        NamespaceBuilder::new("config").static_ns().insert(
            "regions",
            ObjectBuilder::new()
                .insert("us-east", "Virginia")
                .insert("us-west", "Oregon")
                .insert("eu-west", "Ireland")
                .build_scalar(),
        ),
    )?;

    // --- Iterative namespace: loop over each region key ---
    // region = key name (e.g. "us-east"), idx = 0, 1, 2 ...
    let condition_attrs = ObjectBuilder::new()
        .insert(
            "branches",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "is_us")
                    .insert("if", "region is starting_with(\"us-\")")
                    .insert("then", "Region {{ region }} is in the US")
                    .build_scalar(),
                ObjectBuilder::new()
                    .insert("name", "is_eu")
                    .insert("if", "region is starting_with(\"eu-\")")
                    .insert("then", "Region {{ region }} is in the EU")
                    .build_scalar(),
            ]),
        )
        .insert("default", "Region {{ region }} is in an unknown area")
        .build_hashmap();

    let mut handle = pipeline.add_namespace(
        NamespaceBuilder::new("classify")
            .iterative()
            .store_path(StorePath::from_segments(["config", "regions"]))
            .scalar_object_keys(None, false)
            .iter_var("region")
            .index_var("idx"),
    )?;
    handle.add_command::<ConditionCommand>("region", &condition_attrs)?;

    // --- Execute ---
    let completed = pipeline.compile()?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // --- Print results per iteration ---
    println!("=== Iterating over region keys ===\n");

    let mut idx = 0;
    loop {
        let source = StorePath::from_segments(["classify", "region"]).with_index(idx);
        let Some(cmd_results) = results.get_by_source(&source) else {
            break;
        };

        let result = cmd_results
            .data_get(&source.with_segment("result"))
            .and_then(|r| r.as_scalar())
            .expect("Expected result");
        let matched = cmd_results
            .data_get(&source.with_segment("matched"))
            .and_then(|r| r.as_scalar())
            .expect("Expected matched");
        println!("  [{}] {} (matched: {})", idx, result.1, matched.1);

        idx += 1;
    }

    println!("\nProcessed {} region(s)", idx);

    Ok(())
}
