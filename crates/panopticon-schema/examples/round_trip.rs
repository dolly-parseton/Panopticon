//! Load a YAML workflow, compile it, serialise it back to YAML, and
//! demonstrate structural round-trip equivalence.
//!
//! Flow:
//!   1. Read `tag_triage.yaml` from disk
//!   2. `from_yaml` → `Pipeline<Draft>` → `compile()` → `Pipeline<Ready>`
//!   3. `to_yaml` from the ready pipeline
//!   4. Parse the emitted YAML back into a `Document`
//!   5. Compare the baseline and the round-tripped `Document` structurally
//!
//! The original and emitted YAML may differ textually (map key ordering,
//! quoting, whitespace) but must match *structurally* — both parse into
//! the same `Document` value under `PartialEq`.

use std::fs;
use std::path::PathBuf;

use panopticon_core::extend::{Coerce, Compare, Dedupe, Get, Len, SetVar};
use panopticon_schema::types::Document;
use panopticon_schema::{register_ops, Deserialiser, Serialiser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let yaml_path = manifest_dir.join("examples/workflows/tag_triage.yaml");
    let original_yaml = fs::read_to_string(&yaml_path)?;

    // Build one op table and clone it into both the deserialiser and
    // serialiser. Any op registered here becomes round-trippable.
    let ops = register_ops!(
        "SetVar" => SetVar,
        "Get" => Get,
        "Len" => Len,
        "Compare" => Compare,
        "Coerce" => Coerce,
        "Dedupe" => Dedupe,
    );
    let deserialiser = Deserialiser::new(ops.clone());
    let serialiser = Serialiser::new(ops);

    // Phase 1: YAML → Draft → Ready
    let draft = deserialiser.from_yaml(&original_yaml)?;
    let ready = draft.compile()?;

    // Phase 2: Ready → emitted YAML
    let emitted_yaml = serialiser.to_yaml(&ready)?;

    // Phase 3: Structural comparison
    //
    // Parse both documents directly into `Document` values. If the
    // round-trip preserved structure, these must compare equal under
    // `PartialEq` even though the underlying YAML text may differ.
    let original_doc: Document = serde_yaml::from_str(&original_yaml)?;
    let emitted_doc: Document = serde_yaml::from_str(&emitted_yaml)?;

    println!("=== Source: {} ===", yaml_path.display());
    println!();
    println!("--- Original YAML ({} bytes) ---", original_yaml.len());
    println!("{original_yaml}");
    println!("--- Emitted YAML  ({} bytes) ---", emitted_yaml.len());
    println!("{emitted_yaml}");
    println!("--- Structural comparison ---");
    if original_doc == emitted_doc {
        println!("OK — the original and emitted documents are structurally equal.");
        println!(
            "     Variables: {}, steps: {}, return blocks: {}.",
            original_doc.variables.len(),
            original_doc.steps.len(),
            original_doc.returns.len(),
        );
    } else {
        println!("MISMATCH — the round-trip did not preserve structure.");
        println!();
        println!("Original document: {:#?}", original_doc);
        println!();
        println!("Emitted document:  {:#?}", emitted_doc);
        std::process::exit(1);
    }

    // Phase 4 (bonus): re-drafting the emitted YAML must also succeed,
    // proving the output is not just structurally valid but also
    // runnable through the full loader path again.
    let redrafted = deserialiser.from_yaml(&emitted_yaml)?;
    let _ = redrafted.compile()?;
    println!("OK — the emitted YAML compiles cleanly when re-parsed.");

    Ok(())
}
