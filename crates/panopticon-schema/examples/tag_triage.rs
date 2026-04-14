//! Load the tag_triage workflow, run it, and print the final state.
//!
//! Exercises Dedupe, Len, Compare, Get, Coerce, SetVar, and a guard.

use std::fs;
use std::path::PathBuf;

use panopticon_core::extend::{Coerce, Compare, Dedupe, Get, Len, SetVar};
use panopticon_schema::{register_ops, Deserialiser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let yaml_path = manifest_dir.join("examples/workflows/tag_triage.yaml");
    let yaml = fs::read_to_string(&yaml_path)?;

    let ops = register_ops!(
        "SetVar" => SetVar,
        "Get" => Get,
        "Len" => Len,
        "Compare" => Compare,
        "Coerce" => Coerce,
        "Dedupe" => Dedupe,
    );

    let deserialiser = Deserialiser::new(ops);
    let draft = deserialiser.from_yaml(&yaml)?;
    let complete = draft.compile()?.run().wait()?;

    println!("Loaded from: {}", yaml_path.display());
    println!();
    complete.debug();

    Ok(())
}
