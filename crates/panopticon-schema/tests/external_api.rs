//! Integration test that exercises the public API surface only.
//!
//! Lives under `tests/` rather than inside the crate so it can *only*
//! reach public items. If a public entry point goes missing or a
//! `pub(crate)` slips into the path, this test stops compiling.

use panopticon_core::extend::{
    Coerce, Compare, Dedupe, Get, Len, Param, Parameters, Pipeline, SetVar,
};
use panopticon_schema::{
    register_ops, Deserialiser, OperationRegistrationMap, SchemaError, Serialiser,
};

const FULL_WORKFLOW: &str = r#"
version: v1

variables:
  prefix:
    type: text
    value: "hello, "
  target:
    type: text
    value: world
  enabled:
    type: boolean
    value: true
  items:
    type: array
    value:
      - type: integer
        value: 1
      - type: integer
        value: 2
      - type: integer
        value: 2
      - type: integer
        value: 3
  spare:
    type: none

steps:
  - name: build_greeting
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
          - type: ref
            value: prefix
          - type: ref
            value: target

  - name: dedupe_items
    type: step
    op: Dedupe
    params:
      array:
        type: ref
        value: items

  - name: count_unique
    type: step
    op: Len
    params:
      source:
        type: ref
        value: dedupe_items.result

  - name: gated
    type: guard
    source: enabled
    body:
      - name: produce
        type: step
        op: SetVar
        params:
          name:
            type: literal
            value:
              type: text
              value: gated_output
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
    unique_count:
      type: ref
      value: count_unique.result
    gated_output:
      type: ref
      value: gated_output
"#;

fn core_ops() -> panopticon_schema::OperationRegistrationMap {
    register_ops!(
        "SetVar" => SetVar,
        "Get" => Get,
        "Len" => Len,
        "Compare" => Compare,
        "Coerce" => Coerce,
        "Dedupe" => Dedupe,
    )
}

#[test]
fn public_api_round_trip() {
    let deserialiser = Deserialiser::new(core_ops());
    let draft = deserialiser
        .from_yaml(FULL_WORKFLOW)
        .expect("from_yaml should succeed");

    // The draft must be compilable, runnable, and produce a Complete pipeline.
    let _complete = draft.compile().unwrap().run().wait().unwrap();
}

#[test]
fn deserialiser_default_has_empty_ops() {
    let d = Deserialiser::default();
    assert!(d.ops().is_empty());
}

#[test]
fn deserialiser_clone_preserves_ops() {
    let d = Deserialiser::new(core_ops());
    let cloned = d.clone();
    assert_eq!(cloned.ops().len(), d.ops().len());
    assert!(cloned.ops().contains("SetVar"));
}

#[test]
fn unknown_operation_surfaces_as_schema_error() {
    let yaml = r#"
version: v1
steps:
  - name: oops
    type: step
    op: NopeOp
    params: {}
"#;
    let d = Deserialiser::new(core_ops());
    match d.from_yaml(yaml) {
        Err(SchemaError::UnknownOperation(name)) => assert_eq!(name, "NopeOp"),
        Err(other) => panic!("expected UnknownOperation, got {other:?}"),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn serialiser_round_trips_a_programmatic_draft() {
    let mut pipe = Pipeline::new();
    pipe.var("target", "world").unwrap();
    pipe.step::<SetVar>(
        "build_greeting",
        panopticon_core::extend::params!(
            "name" => "greeting",
            "value" => Param::reference("target"),
        ),
    )
    .unwrap();

    let ready = pipe.compile().unwrap();
    let ser = Serialiser::new(core_ops());

    // `to_yaml` must produce a YAML string that the Deserialiser can
    // round-trip, including recognising the op via the forward map.
    let yaml = ser.to_yaml(&ready).expect("to_yaml");
    let de = Deserialiser::new(core_ops());
    let _redrafted = de.from_yaml(&yaml).expect("re-parse the emitted YAML");
}

#[test]
fn serialiser_rejects_draft_with_unregistered_op() {
    // Simulate a draft whose op set was registered via the forward-only
    // `insert` escape hatch — no reverse TypeId entry, so `to_document`
    // must refuse.
    let mut pipe = Pipeline::new();
    pipe.var("x", "y").unwrap();
    pipe.step::<SetVar>(
        "s",
        panopticon_core::extend::params!(
            "name" => "out",
            "value" => Param::reference("x"),
        ),
    )
    .unwrap();
    let ready = pipe.compile().unwrap();

    let source = core_ops();
    let thunk = source.get("SetVar").unwrap();
    let mut ops_no_reverse = OperationRegistrationMap::new();
    ops_no_reverse.insert("SetVar", thunk);
    let ser = Serialiser::new(ops_no_reverse);

    match ser.to_document(&ready) {
        Err(SchemaError::UnregisteredOperation { .. }) => {}
        Err(other) => panic!("expected UnregisteredOperation, got {other:?}"),
        Ok(_) => panic!("expected error"),
    }
}
