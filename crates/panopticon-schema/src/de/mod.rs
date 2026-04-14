//! Deserialisation: YAML → `Pipeline<Draft>`.
//!
//! This module hosts the public [`Deserialiser`] facade and the internal
//! [`lower`] walker that implements it. `Deserialiser` is a thin wrapper
//! over a two-pass parse: peek the root `version:` field, dispatch into
//! the matching schema struct, then hand the parsed [`crate::schema::Document`]
//! to [`lower::lower`] for lowering into core's fluent `Pipeline<Draft>`
//! API.
//!
//! In the common case a caller constructs one [`Deserialiser`] with a
//! set of registered operations and then calls [`Deserialiser::from_yaml`]
//! for each document it wants to load.

pub mod lower;

use panopticon_core::extend::{Draft, Pipeline};

use crate::error::SchemaError;
use crate::registry::OperationRegistrationMap;
use crate::schema::document::Document;
use crate::schema::version::{Version, VersionOnly};

/// Lowers strict-schema YAML documents into
/// `panopticon_core::extend::Pipeline<Draft>` instances, resolving
/// operation references through an [`OperationRegistrationMap`] built by
/// the [`crate::register_ops!`] macro.
///
/// Hooks and extensions are out of scope — the produced draft contains
/// only variables, parameters, execution order, and the operation
/// registry. The caller is expected to inject hooks and extensions on the
/// resulting draft before calling `Pipeline::compile()`.
#[derive(Default, Clone)]
pub struct Deserialiser {
    ops: OperationRegistrationMap,
}

impl Deserialiser {
    /// Construct a deserialiser that resolves operation names through
    /// `ops`. The map is typically built by the `crate::register_ops!` macro.
    pub fn new(ops: OperationRegistrationMap) -> Self {
        Self { ops }
    }

    /// Read-only access to the underlying operation registration map.
    /// Useful for tests and for introspecting which ops a deserialiser
    /// can resolve.
    pub fn ops(&self) -> &OperationRegistrationMap {
        &self.ops
    }

    /// Deserialise a strict-schema YAML document into a fresh
    /// `Pipeline<Draft>`.
    ///
    /// Parses in two passes:
    /// 1. Peek the root `version:` field to decide which schema
    ///    revision to parse into.
    /// 2. Parse the full document for that version and hand it to
    ///    [`Deserialiser::from_document`].
    ///
    /// Only v1 is supported today. Adding v2 means extending [`Version`]
    /// and adding a new arm to the match in this function.
    ///
    /// # Errors
    ///
    /// - [`SchemaError::Yaml`] — the YAML failed to parse, violated the
    ///   schema shape, referenced an unknown schema version, or declared
    ///   an unknown field/variant.
    /// - [`SchemaError::UnknownOperation`] — a step node referenced an
    ///   operation name that is not present in the registration map.
    /// - [`SchemaError::Draft`] / [`SchemaError::Store`] — the lowering
    ///   walker's call to core was rejected (e.g. duplicate step name,
    ///   duplicate variable).
    pub fn from_yaml(&self, input: &str) -> Result<Pipeline<Draft>, SchemaError> {
        let VersionOnly { version } = serde_yaml::from_str::<VersionOnly>(input)?;

        match version {
            Version::V1 => {
                let doc: Document = serde_yaml::from_str(input)?;
                self.from_document(doc)
            }
        }
    }

    /// Lower a programmatically-built [`Document`] into a fresh
    /// `Pipeline<Draft>`, skipping YAML parsing entirely.
    ///
    /// This is the in-memory counterpart to
    /// [`Deserialiser::from_yaml`]. Useful for GUI editors, static
    /// analysers, and tests that want to build or mutate schema
    /// documents in memory without round-tripping through YAML text.
    ///
    /// # Errors
    ///
    /// - [`SchemaError::UnknownOperation`] — a step node referenced an
    ///   operation name that is not present in the registration map.
    /// - [`SchemaError::Draft`] / [`SchemaError::Store`] — the lowering
    ///   walker's call to core was rejected (e.g. duplicate step name,
    ///   duplicate variable, forward reference).
    ///
    /// [`Document`]: crate::types::Document
    pub fn from_document(&self, doc: Document) -> Result<Pipeline<Draft>, SchemaError> {
        lower::lower(doc, &self.ops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register_ops;
    use crate::schema::document::Node;
    use crate::schema::param::ParamSpec;
    use crate::schema::value::{LiteralValue, VariableSpec};
    use panopticon_core::extend::SetVar;
    use std::collections::BTreeMap;

    fn make_deserialiser() -> Deserialiser {
        Deserialiser::new(register_ops!(
            "SetVar" => SetVar,
        ))
    }

    #[test]
    fn from_document_runs_end_to_end() {
        // Build a Document directly, no YAML involved.
        let mut variables = BTreeMap::new();
        variables.insert("target".into(), VariableSpec::Text("world".into()));

        let mut params = BTreeMap::new();
        params.insert(
            "name".into(),
            ParamSpec::Literal(LiteralValue::Text("greeting".into())),
        );
        params.insert("value".into(), ParamSpec::Ref("target".into()));

        let steps = vec![Node::Step {
            name: "build_greeting".into(),
            op: "SetVar".into(),
            params,
        }];

        let mut ret_block = BTreeMap::new();
        ret_block.insert("greeting".into(), ParamSpec::Ref("greeting".into()));
        let mut returns = BTreeMap::new();
        returns.insert("out".into(), ret_block);

        let doc = Document {
            version: Version::V1,
            variables,
            steps,
            returns,
        };

        let pipe = make_deserialiser()
            .from_document(doc)
            .expect("from_document ok");
        pipe.compile()
            .expect("compile ok")
            .run()
            .wait()
            .expect("run ok");
    }

    #[test]
    fn from_yaml_end_to_end() {
        let yaml = r#"
version: v1

variables:
  prefix:
    type: text
    value: "hello, "
  target:
    type: text
    value: world
  flag:
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
        value: 3

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

  - name: walk
    type: iter_array
    source: items
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

  - name: gate
    type: guard
    source: flag
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
    gated:
      type: ref
      value: gated
"#;

        let d = make_deserialiser();
        let pipe = d.from_yaml(yaml).expect("from_yaml ok");
        pipe.compile()
            .expect("compile ok")
            .run()
            .wait()
            .expect("run ok");
    }

    #[test]
    fn from_yaml_supports_null_literal_variable() {
        let yaml = r#"
version: v1
variables:
  maybe:
    type: none
"#;
        let d = make_deserialiser();
        let pipe = d.from_yaml(yaml).expect("from_yaml ok");
        pipe.compile().expect("compile ok");
    }

    #[test]
    fn from_yaml_accepts_null_alias_null_value() {
        let yaml = r#"
version: v1
variables:
  maybe:
    type: null_value
"#;
        let d = make_deserialiser();
        let pipe = d.from_yaml(yaml).expect("from_yaml ok");
        pipe.compile().expect("compile ok");
    }

    #[test]
    fn from_yaml_accepts_null_alias_quoted_null() {
        let yaml = r#"
version: v1
variables:
  maybe:
    type: "null"
"#;
        let d = make_deserialiser();
        let pipe = d.from_yaml(yaml).expect("from_yaml ok");
        pipe.compile().expect("compile ok");
    }

    #[test]
    fn from_yaml_rejects_unknown_version() {
        let yaml = "version: v99";
        let d = make_deserialiser();
        match d.from_yaml(yaml) {
            Err(SchemaError::Yaml(_)) => {}
            Err(other) => panic!("expected Yaml error for unknown version, got {other:?}"),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn from_yaml_rejects_unknown_operation() {
        let yaml = r#"
version: v1
steps:
  - name: oops
    type: step
    op: NopeOp
    params: {}
"#;
        let d = make_deserialiser();
        match d.from_yaml(yaml) {
            Err(SchemaError::UnknownOperation(name)) => assert_eq!(name, "NopeOp"),
            Err(other) => panic!("expected UnknownOperation, got {other:?}"),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn from_yaml_rejects_unknown_top_level_field() {
        let yaml = r#"
version: v1
surprise: true
"#;
        let d = make_deserialiser();
        match d.from_yaml(yaml) {
            Err(SchemaError::Yaml(_)) => {}
            Err(other) => panic!("expected Yaml (unknown field) error, got {other:?}"),
            Ok(_) => panic!("expected error"),
        }
    }
}
