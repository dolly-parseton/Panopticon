//! Load a YAML workflow from disk, lower it into a `Pipeline<Draft>`, compile,
//! run, and dump the resulting variables and returns.

use std::fs;
use std::path::PathBuf;

use panopticon_core::extend::SetVar;
use panopticon_schema::{Deserialiser, register_ops};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Locate the example YAML relative to the crate manifest.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let yaml_path = manifest_dir.join("examples/workflows/hello_yaml.yaml");
    let yaml = fs::read_to_string(&yaml_path)?;

    // Register the operations the document will reference.
    let ops = register_ops!(
        "SetVar" => SetVar,
    );

    let deserialiser = Deserialiser::new(ops);

    // YAML → Pipeline<Draft> → Ready → Running → Complete
    let draft = deserialiser.from_yaml(&yaml)?;
    let complete = draft.compile()?.run().wait()?;

    println!("Loaded from: {}", yaml_path.display());
    println!();
    complete.debug();

    Ok(())
}
