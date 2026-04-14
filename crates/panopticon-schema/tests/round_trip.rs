//! Round-trip integration tests.
//!
//! For every fixture: parse the YAML, lift it back through the serialiser,
//! and assert structural equality between the two `Document`s. Textual
//! equality is not guaranteed (map iteration order and formatting may
//! differ), but structural equality proves that lowering and lifting are
//! inverses for the v1 schema.

use std::fs;
use std::path::PathBuf;

use panopticon_core::extend::{Coerce, Compare, Dedupe, Get, Len, SetVar};
use panopticon_schema::types::Document;
use panopticon_schema::{register_ops, Deserialiser, OperationRegistrationMap, Serialiser};

fn core_ops() -> OperationRegistrationMap {
    register_ops!(
        "SetVar" => SetVar,
        "Get" => Get,
        "Len" => Len,
        "Compare" => Compare,
        "Coerce" => Coerce,
        "Dedupe" => Dedupe,
    )
}

fn workflow_path(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("examples").join("workflows").join(name)
}

fn round_trip_check(yaml: &str) {
    let ops = core_ops();
    let deserialiser = Deserialiser::new(ops.clone());
    let serialiser = Serialiser::new(ops);

    // 1. Parse YAML into a Document directly — our baseline.
    let doc1: Document = serde_yaml::from_str(yaml).expect("YAML must parse into Document");

    // 2. Run the whole loader path and lift the ready pipeline back.
    let draft = deserialiser.from_yaml(yaml).expect("from_yaml");
    let ready = draft.compile().expect("compile");
    let doc2 = serialiser.to_document(&ready).expect("to_document");

    // 3. The two documents must be structurally identical.
    assert_eq!(doc1, doc2, "round-trip should be structurally idempotent");

    // 4. As a second check, emit YAML from the lifted document, parse it
    //    back, and verify that parse still equals the baseline.
    let emitted = serialiser.to_yaml(&ready).expect("to_yaml");
    let doc3: Document = serde_yaml::from_str(&emitted).expect("emitted YAML must re-parse");
    assert_eq!(doc1, doc3, "YAML emission must round-trip structurally");
}

#[test]
fn hello_yaml_round_trip() {
    let yaml = fs::read_to_string(workflow_path("hello_yaml.yaml"))
        .expect("hello_yaml.yaml must be readable");
    round_trip_check(&yaml);
}

#[test]
fn tag_triage_round_trip() {
    let yaml = fs::read_to_string(workflow_path("tag_triage.yaml"))
        .expect("tag_triage.yaml must be readable");
    round_trip_check(&yaml);
}

#[test]
fn synthetic_exercises_every_node_type_and_param_form() {
    let yaml = r#"
version: v1

variables:
  scalar_text:
    type: text
    value: hello
  scalar_int:
    type: integer
    value: 42
  scalar_float:
    type: float
    value: 3.5
  scalar_bool:
    type: boolean
    value: true
  spare:
    type: none
  list:
    type: array
    value:
      - type: integer
        value: 1
      - type: integer
        value: 2
  mapping:
    type: map
    value:
      host:
        type: text
        value: localhost
      flag:
        type: boolean
        value: false

steps:
  - name: set_greeting
    type: step
    op: SetVar
    params:
      name:
        type: literal
        value:
          type: text
          value: greeting
      value:
        type: template
        value:
          - type: literal
            value:
              type: text
              value: "hi, "
          - type: ref
            value: scalar_text

  - name: walk
    type: iter_array
    source: list
    body:
      - name: capture
        type: step
        op: SetVar
        params:
          name:
            type: literal
            value:
              type: text
              value: current
          value:
            type: ref
            value: walk.__item

  - name: iterate_map
    type: iter_map
    source: mapping
    body:
      - name: inner
        type: step
        op: SetVar
        params:
          name:
            type: literal
            value:
              type: text
              value: current_value
          value:
            type: ref
            value: iterate_map.__value

  - name: gate
    type: guard
    source: scalar_bool
    body:
      - name: produce
        type: step
        op: SetVar
        params:
          name:
            type: literal
            value:
              type: text
              value: gated
          value:
            type: literal
            value:
              type: text
              value: visible

returns:
  summary:
    greeting:
      type: ref
      value: greeting
    spare:
      type: ref
      value: spare
"#;

    round_trip_check(yaml);
}
