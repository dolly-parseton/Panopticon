//! Root document and node schema.
//!
//! [`Document`] is the root of a v1 workflow YAML file. Its `steps` field
//! is an ordered list of [`Node`] values; each `Node` is one of four
//! shapes distinguished by an internally-tagged `type:` field.
//!
//! Both `Document` and `Node` apply `#[serde(deny_unknown_fields)]` so
//! typos and stray fields become hard parse errors.

use super::param::ParamSpec;
use super::value::VariableSpec;
use super::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Root document of a v1 workflow.
///
/// See `SCHEMA_V1.md` at the crate root for the complete format reference.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Schema version discriminant. Must be `v1`.
    pub version: Version,

    /// Variable declarations, keyed by variable name. Registered against
    /// the pipeline's store at draft time and visible to every subsequent
    /// step.
    #[serde(default)]
    pub variables: BTreeMap<String, VariableSpec>,

    /// Ordered list of execution nodes. List order is preserved and
    /// becomes the pipeline's execution order.
    #[serde(default)]
    pub steps: Vec<Node>,

    /// Return projections, keyed by return-block name. Each inner map
    /// projects parameter names to `ParamSpec` values that are resolved
    /// after the main execution completes.
    #[serde(default)]
    pub returns: BTreeMap<String, BTreeMap<String, ParamSpec>>,
}

/// A single execution node in the `steps:` list.
///
/// Internally tagged on `type:` so each node reads as:
///
/// ```yaml
/// - name: set_greeting
///   type: step
///   op: SetVar
///   params: { ... }
/// ```
///
/// The `name` field on every variant is the *execution node* identifier,
/// used for error messages, dependency tracking, and parameter-store keys.
/// It is distinct from any parameter that happens to be called `name` —
/// e.g. the `SetVar` operation takes a `name` *parameter* to derive its
/// output variable name, and that parameter lives inside `params`, not
/// here.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Node {
    /// Invoke an operation with a map of input parameters. Lowered to
    /// `Pipeline::step::<O>()` via an [`crate::OperationRegistrationMap`]
    /// lookup on the `op` field.
    Step {
        /// Execution node identifier. Must be unique within its scope.
        name: String,
        /// Operation name to resolve against the registration map.
        op: String,
        /// Input parameters passed to the operation.
        #[serde(default)]
        params: BTreeMap<String, ParamSpec>,
    },
    /// Iterate over an array variable, running `body` once per element.
    /// Each iteration runs in a cloned store; see `SCHEMA_V1.md` for
    /// scoping semantics and reserved `{name}.__index` / `{name}.__item`
    /// bindings.
    IterArray {
        /// Execution node identifier.
        name: String,
        /// Name of the array variable to iterate over.
        source: String,
        /// Nested list of execution nodes executed per iteration.
        #[serde(default)]
        body: Vec<Node>,
    },
    /// Iterate over a map variable, running `body` once per entry. Each
    /// iteration runs in a cloned store; reserved bindings are
    /// `{name}.__key` and `{name}.__value`.
    IterMap {
        /// Execution node identifier.
        name: String,
        /// Name of the map variable to iterate over.
        source: String,
        /// Nested list of execution nodes executed per iteration.
        #[serde(default)]
        body: Vec<Node>,
    },
    /// Conditionally execute `body` based on a boolean variable. The
    /// body shares parent scope — outputs produced inside the guard are
    /// visible to steps after the guard and to `returns` blocks.
    Guard {
        /// Execution node identifier.
        name: String,
        /// Name of the boolean variable controlling the gate.
        source: String,
        /// Nested list of execution nodes executed when `source` is true.
        #[serde(default)]
        body: Vec<Node>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::value::LiteralValue;

    #[test]
    fn empty_document_parses() {
        let y = "version: v1";
        let doc: Document = serde_yaml::from_str(y).unwrap();
        assert_eq!(doc.version, Version::V1);
        assert!(doc.variables.is_empty());
        assert!(doc.steps.is_empty());
        assert!(doc.returns.is_empty());
    }

    #[test]
    fn document_with_variables_parses() {
        let y = "
version: v1
variables:
  greeting:
    type: text
    value: hello
  count:
    type: integer
    value: 3
  empty:
    type: none
";
        let doc: Document = serde_yaml::from_str(y).unwrap();
        assert_eq!(
            doc.variables.get("greeting"),
            Some(&VariableSpec::Text("hello".into()))
        );
        assert_eq!(
            doc.variables.get("count"),
            Some(&VariableSpec::Integer(3))
        );
        assert_eq!(doc.variables.get("empty"), Some(&VariableSpec::Null));
    }

    #[test]
    fn step_node_parses() {
        let y = "
version: v1
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
        type: ref
        value: target
";
        let doc: Document = serde_yaml::from_str(y).unwrap();
        assert_eq!(doc.steps.len(), 1);
        match &doc.steps[0] {
            Node::Step { name, op, params } => {
                assert_eq!(name, "set_greeting");
                assert_eq!(op, "SetVar");
                assert_eq!(
                    params.get("name"),
                    Some(&ParamSpec::Literal(LiteralValue::Text("greeting".into())))
                );
                assert_eq!(params.get("value"), Some(&ParamSpec::Ref("target".into())));
            }
            other => panic!("expected Step, got {other:?}"),
        }
    }

    #[test]
    fn iter_array_node_parses_with_nested_body() {
        let y = "
version: v1
steps:
  - name: loop
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
            value: loop.__item
";
        let doc: Document = serde_yaml::from_str(y).unwrap();
        match &doc.steps[0] {
            Node::IterArray { name, source, body } => {
                assert_eq!(name, "loop");
                assert_eq!(source, "items");
                assert_eq!(body.len(), 1);
                match &body[0] {
                    Node::Step { op, params, .. } => {
                        assert_eq!(op, "SetVar");
                        assert_eq!(
                            params.get("value"),
                            Some(&ParamSpec::Ref("loop.__item".into()))
                        );
                    }
                    other => panic!("expected nested Step, got {other:?}"),
                }
            }
            other => panic!("expected IterArray, got {other:?}"),
        }
    }

    #[test]
    fn guard_node_parses() {
        let y = "
version: v1
steps:
  - name: check
    type: guard
    source: flag
    body: []
";
        let doc: Document = serde_yaml::from_str(y).unwrap();
        match &doc.steps[0] {
            Node::Guard { name, source, body } => {
                assert_eq!(name, "check");
                assert_eq!(source, "flag");
                assert!(body.is_empty());
            }
            other => panic!("expected Guard, got {other:?}"),
        }
    }

    #[test]
    fn returns_parses() {
        let y = "
version: v1
returns:
  summary:
    greeting:
      type: ref
      value: greeting
    count:
      type: ref
      value: count
";
        let doc: Document = serde_yaml::from_str(y).unwrap();
        let summary = doc.returns.get("summary").expect("summary block");
        assert_eq!(summary.get("greeting"), Some(&ParamSpec::Ref("greeting".into())));
        assert_eq!(summary.get("count"), Some(&ParamSpec::Ref("count".into())));
    }

    #[test]
    fn unknown_top_level_field_rejected() {
        let y = "
version: v1
surprise: true
";
        let err = serde_yaml::from_str::<Document>(y).unwrap_err();
        assert!(err.to_string().contains("unknown field") || err.to_string().contains("surprise"));
    }

    #[test]
    fn unknown_step_field_rejected() {
        let y = "
version: v1
steps:
  - name: bad
    type: step
    op: SetVar
    opcode: oops
    params: {}
";
        let err = serde_yaml::from_str::<Document>(y).unwrap_err();
        assert!(err.to_string().contains("unknown field") || err.to_string().contains("opcode"));
    }

    #[test]
    fn unknown_node_type_rejected() {
        let y = "
version: v1
steps:
  - name: bad
    type: teleport
";
        let err = serde_yaml::from_str::<Document>(y).unwrap_err();
        assert!(err.to_string().contains("unknown variant") || err.to_string().contains("teleport"));
    }
}
