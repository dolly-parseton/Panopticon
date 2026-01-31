//! Example: Tera template inheritance and composition
//!
//! Uses the fixtures/tera/ templates (base.tera, header.tera, page.tera) to
//! demonstrate template glob loading, Tera inheritance, includes, and capture mode.
//!
//! Run with: cargo run --example template_inheritance

use panopticon_core::prelude::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let mut pipeline = Pipeline::new();

    // --- Static namespace: provide data for the Tera templates ---
    pipeline.add_namespace(
        NamespaceBuilder::new("inputs")
            .static_ns()
            .insert("site_name", ScalarValue::String("Panopticon Demo".to_string()))
            .insert("page_title", ScalarValue::String("Getting Started".to_string()))
            .insert(
                "page_content",
                ScalarValue::String("Welcome to the Panopticon pipeline engine.".to_string()),
            )
            .insert(
                "nav_items",
                ScalarValue::Array(vec![
                    ObjectBuilder::new()
                        .insert("label", "Home")
                        .insert("url", "/")
                        .build_scalar(),
                    ObjectBuilder::new()
                        .insert("label", "Docs")
                        .insert("url", "/docs")
                        .build_scalar(),
                    ObjectBuilder::new()
                        .insert("label", "Examples")
                        .insert("url", "/examples")
                        .build_scalar(),
                ]),
            ),
    )?;

    // --- TemplateCommand: load templates via glob and render page.tera ---
    // page.tera extends base.tera and includes header.tera
    let template_attrs = ObjectBuilder::new()
        .insert(
            "template_glob",
            format!("{}/**/*.tera", fixtures_dir().join("tera").display()),
        )
        .insert("render", "page.tera")
        .insert(
            "output",
            format!("{}/page.html", temp_dir.path().display()),
        )
        .insert("capture", true)
        .build_hashmap();

    pipeline
        .add_namespace(NamespaceBuilder::new("render"))?
        .add_command::<TemplateCommand>("page", &template_attrs)?;

    // --- Execute ---
    let completed = pipeline.compile()?.execute().await?;
    let results = completed.results(ResultSettings::default()).await?;

    // --- Print the captured output ---
    let source = StorePath::from_segments(["render", "page"]);
    let cmd_results = results
        .get_by_source(&source)
        .expect("Expected render.page results");

    // "content" is a Data result (only present when capture=true)
    let content = cmd_results
        .data_get(&source.with_segment("content"))
        .and_then(|r| r.as_scalar())
        .expect("Expected content (capture=true)");

    // "size" and "line_count" are Meta results
    let size = cmd_results
        .meta_get(&source.with_segment("size"))
        .expect("Expected size");
    let lines = cmd_results
        .meta_get(&source.with_segment("line_count"))
        .expect("Expected line_count");

    let content_str = content.1.as_str().unwrap_or("(empty)");
    println!("Rendered page.tera ({} bytes, {} lines):\n", size, lines);
    println!("{}", content_str);

    // --- Verify the file was also written to disk ---
    let on_disk = tokio::fs::read_to_string(temp_dir.path().join("page.html")).await?;
    assert_eq!(on_disk, content_str);
    println!("\n(file on disk matches captured content)");

    Ok(())
}
