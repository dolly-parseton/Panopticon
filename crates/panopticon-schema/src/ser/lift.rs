//! Walk a `Pipeline<Ready>` and build a [`Document`] via the public core API.
//!
//! This module is the inverse of [`crate::de::lower`]. The walker consults the
//! same [`OperationRegistrationMap`] that `lower` uses, but in the reverse
//! direction — resolving `TypeId`s back to operation-name strings for the
//! emitted YAML.
//!
//! Most callers should use [`crate::Serialiser::to_yaml`] rather than
//! invoking [`lift`] directly. The direct entry point is public for
//! advanced use cases such as building a [`Document`] in memory and
//! post-processing it before serialisation (e.g. a GUI editor).
//!
//! Hooks and extensions attached to the ready pipeline are ignored by the
//! walker — they are not representable in v1 YAML and remain a host-program
//! concern.

use std::collections::BTreeMap;

use panopticon_core::extend::{
    ExecutionNode, Param, Parameters, Pipeline, Ready, Store, StoreEntry, Value,
};

use crate::error::SchemaError;
use crate::registry::OperationRegistrationMap;
use crate::schema::document::{Document, Node};
use crate::schema::param::ParamSpec;
use crate::schema::value::{LiteralValue, VariableSpec};
use crate::schema::version::Version;

/// Lift a `Pipeline<Ready>` into a schema [`Document`].
///
/// Walks variables, execution order, and returns in that order, consulting
/// `ops` to resolve each step node's `TypeId` back to its registered
/// operation name. Missing reverse-map entries surface as
/// [`SchemaError::UnregisteredOperation`]; store shapes the v1 schema cannot
/// represent surface as [`SchemaError::UnsupportedStoreShape`].
pub fn lift(
    ready: &Pipeline<Ready>,
    ops: &OperationRegistrationMap,
) -> Result<Document, SchemaError> {
    let variables = lift_variables(ready.variables())?;
    let steps = lift_nodes(ready.execution_order(), ready.parameters(), None, ops)?;
    let returns = lift_returns(ready.returns())?;

    Ok(Document {
        version: Version::V1,
        variables,
        steps,
        returns,
    })
}

fn lift_variables(
    store: &Store<StoreEntry>,
) -> Result<BTreeMap<String, VariableSpec>, SchemaError> {
    let mut out = BTreeMap::new();
    for (name, entry) in store.iter() {
        out.insert(name.clone(), store_entry_to_variable(name, entry)?);
    }
    Ok(out)
}

fn store_entry_to_variable(
    path: &str,
    entry: &StoreEntry,
) -> Result<VariableSpec, SchemaError> {
    match entry {
        StoreEntry::Var { value, .. } => Ok(match value {
            Value::Text(s) => VariableSpec::Text(s.clone()),
            Value::Integer(i) => VariableSpec::Integer(*i),
            Value::Float(f) => VariableSpec::Float(*f),
            Value::Boolean(b) => VariableSpec::Boolean(*b),
            Value::Null => VariableSpec::Null,
            _ => {
                return Err(SchemaError::UnsupportedStoreShape {
                    path: path.to_string(),
                    reason: "non-exhaustive `Value` variant encountered".into(),
                });
            }
        }),
        StoreEntry::Array(items) => {
            let mut lits = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                lits.push(nested_literal(&format!("{path}[{i}]"), item)?);
            }
            Ok(VariableSpec::Array(lits))
        }
        StoreEntry::Map(entries) => {
            let mut map = BTreeMap::new();
            for (k, v) in entries {
                map.insert(k.clone(), nested_literal(&format!("{path}.{k}"), v)?);
            }
            Ok(VariableSpec::Map(map))
        }
    }
}

/// Convert a nested `StoreEntry` inside an array or map variable into a
/// `LiteralValue`. V1 only allows scalar literals inside collections, so
/// nested arrays and maps are rejected.
fn nested_literal(path: &str, entry: &StoreEntry) -> Result<LiteralValue, SchemaError> {
    match entry {
        StoreEntry::Var { value, .. } => match value {
            Value::Text(s) => Ok(LiteralValue::Text(s.clone())),
            Value::Integer(i) => Ok(LiteralValue::Integer(*i)),
            Value::Float(f) => Ok(LiteralValue::Float(*f)),
            Value::Boolean(b) => Ok(LiteralValue::Boolean(*b)),
            Value::Null => Ok(LiteralValue::Null),
            _ => Err(SchemaError::UnsupportedStoreShape {
                path: path.to_string(),
                reason: "non-exhaustive `Value` variant encountered".into(),
            }),
        },
        StoreEntry::Array(_) => Err(SchemaError::UnsupportedStoreShape {
            path: path.to_string(),
            reason: "nested array inside a collection variable is not representable in v1".into(),
        }),
        StoreEntry::Map(_) => Err(SchemaError::UnsupportedStoreShape {
            path: path.to_string(),
            reason: "nested map inside a collection variable is not representable in v1".into(),
        }),
    }
}

fn lift_nodes(
    nodes: &[ExecutionNode],
    params: &Store<Parameters>,
    parent_prefix: Option<&str>,
    ops: &OperationRegistrationMap,
) -> Result<Vec<Node>, SchemaError> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        out.push(lift_node(node, params, parent_prefix, ops)?);
    }
    Ok(out)
}

fn lift_node(
    node: &ExecutionNode,
    params: &Store<Parameters>,
    parent_prefix: Option<&str>,
    ops: &OperationRegistrationMap,
) -> Result<Node, SchemaError> {
    match node {
        ExecutionNode::Step {
            name: full_name,
            type_id,
        } => {
            let op_name = ops
                .name_for_type_id(*type_id)
                .ok_or(SchemaError::UnregisteredOperation { type_id: *type_id })?
                .to_string();

            let parameters = params.get(full_name.as_str())?;
            let lifted_params = lift_params(parameters)?;

            Ok(Node::Step {
                name: local_name_within(full_name, parent_prefix).to_string(),
                op: op_name,
                params: lifted_params,
            })
        }
        ExecutionNode::IterArray {
            name: full_name,
            source,
            body,
            ..
        } => {
            let body_nodes = lift_nodes(body, params, Some(full_name), ops)?;
            Ok(Node::IterArray {
                name: local_name_within(full_name, parent_prefix).to_string(),
                source: source.reference().to_string(),
                body: body_nodes,
            })
        }
        ExecutionNode::IterMap {
            name: full_name,
            source,
            body,
            ..
        } => {
            let body_nodes = lift_nodes(body, params, Some(full_name), ops)?;
            Ok(Node::IterMap {
                name: local_name_within(full_name, parent_prefix).to_string(),
                source: source.reference().to_string(),
                body: body_nodes,
            })
        }
        ExecutionNode::Guard {
            name: full_name,
            source,
            body,
        } => {
            let body_nodes = lift_nodes(body, params, Some(full_name), ops)?;
            Ok(Node::Guard {
                name: local_name_within(full_name, parent_prefix).to_string(),
                source: source.reference().to_string(),
                body: body_nodes,
            })
        }
    }
}

/// Unflatten a fully-qualified execution-node name back to the local form
/// used in the YAML document. Core stores body steps as `parent.child`;
/// the document keeps the unprefixed `child`.
fn local_name_within<'a>(full: &'a str, parent_prefix: Option<&str>) -> &'a str {
    match parent_prefix {
        Some(prefix) => full
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_prefix('.'))
            .unwrap_or(full),
        None => full,
    }
}

fn lift_returns(
    store: &Store<Parameters>,
) -> Result<BTreeMap<String, BTreeMap<String, ParamSpec>>, SchemaError> {
    let mut out = BTreeMap::new();
    for (name, params) in store.iter() {
        out.insert(name.clone(), lift_params(params)?);
    }
    Ok(out)
}

fn lift_params(params: &Parameters) -> Result<BTreeMap<String, ParamSpec>, SchemaError> {
    let mut out = BTreeMap::new();
    for (k, v) in params.iter() {
        out.insert(k.clone(), lift_param(v)?);
    }
    Ok(out)
}

fn lift_param(p: &Param) -> Result<ParamSpec, SchemaError> {
    Ok(match p {
        Param::Literal(v) => ParamSpec::Literal(value_to_literal(v)?),
        Param::Ref(path) => ParamSpec::Ref(path.clone()),
        Param::Template(parts) => ParamSpec::Template(
            parts
                .iter()
                .map(lift_param)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Param::Array(items) => ParamSpec::Array(
            items
                .iter()
                .map(lift_param)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Param::Map(entries) => {
            let mut map = BTreeMap::new();
            for (k, v) in entries {
                map.insert(k.clone(), lift_param(v)?);
            }
            ParamSpec::Map(map)
        }
    })
}

fn value_to_literal(v: &Value) -> Result<LiteralValue, SchemaError> {
    Ok(match v {
        Value::Text(s) => LiteralValue::Text(s.clone()),
        Value::Integer(i) => LiteralValue::Integer(*i),
        Value::Float(f) => LiteralValue::Float(*f),
        Value::Boolean(b) => LiteralValue::Boolean(*b),
        Value::Null => LiteralValue::Null,
        _ => {
            return Err(SchemaError::UnsupportedStoreShape {
                path: String::new(),
                reason: "non-exhaustive `Value` variant encountered in parameter literal".into(),
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register_ops;
    use panopticon_core::extend::SetVar;

    fn core_ops() -> OperationRegistrationMap {
        register_ops!(
            "SetVar" => SetVar,
        )
    }

    #[test]
    fn lift_variables_only() {
        let mut pipe = Pipeline::<_>::new();
        pipe.var("greeting", "hello").unwrap();
        pipe.var("count", 3i64).unwrap();
        pipe.var("ratio", 0.5f64).unwrap();
        pipe.var("enabled", true).unwrap();
        pipe.var("spare", Value::Null).unwrap();

        let ready = pipe.compile().unwrap();
        let doc = lift(&ready, &core_ops()).expect("lift ok");

        assert_eq!(doc.version, Version::V1);
        assert_eq!(
            doc.variables.get("greeting"),
            Some(&VariableSpec::Text("hello".into()))
        );
        assert_eq!(doc.variables.get("count"), Some(&VariableSpec::Integer(3)));
        assert_eq!(doc.variables.get("spare"), Some(&VariableSpec::Null));
        assert!(doc.steps.is_empty());
        assert!(doc.returns.is_empty());
    }

    #[test]
    fn lift_array_and_map_variables() {
        let mut pipe = Pipeline::<_>::new();
        pipe.array("items")
            .unwrap()
            .push(Value::Integer(1))
            .unwrap()
            .push(Value::Integer(2))
            .unwrap();
        pipe.map("config")
            .unwrap()
            .insert("host", "localhost")
            .unwrap();

        let ready = pipe.compile().unwrap();
        let doc = lift(&ready, &core_ops()).expect("lift ok");

        match doc.variables.get("items") {
            Some(VariableSpec::Array(items)) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], LiteralValue::Integer(1));
            }
            other => panic!("expected array, got {other:?}"),
        }

        match doc.variables.get("config") {
            Some(VariableSpec::Map(entries)) => {
                assert_eq!(
                    entries.get("host"),
                    Some(&LiteralValue::Text("localhost".into()))
                );
            }
            other => panic!("expected map, got {other:?}"),
        }
    }

    #[test]
    fn lift_single_step_node() {
        let mut pipe = Pipeline::<_>::new();
        pipe.var("source", "hello").unwrap();
        pipe.step::<SetVar>(
            "set_greeting",
            panopticon_core::extend::params!(
                "name" => "greeting",
                "value" => Param::reference("source"),
            ),
        )
        .unwrap();

        let ready = pipe.compile().unwrap();
        let doc = lift(&ready, &core_ops()).expect("lift ok");

        assert_eq!(doc.steps.len(), 1);
        match &doc.steps[0] {
            Node::Step { name, op, params } => {
                assert_eq!(name, "set_greeting");
                assert_eq!(op, "SetVar");
                assert_eq!(
                    params.get("value"),
                    Some(&ParamSpec::Ref("source".into()))
                );
            }
            other => panic!("expected Step, got {other:?}"),
        }
    }

    #[test]
    fn lift_iter_array_unflattens_body_names() {
        let mut pipe = Pipeline::<_>::new();
        pipe.array("items")
            .unwrap()
            .push(Value::Integer(1))
            .unwrap();

        pipe.iter_array(
            "walk",
            panopticon_core::extend::IterSource::array("items"),
            |_, item, body| {
                body.step::<SetVar>(
                    "capture",
                    panopticon_core::extend::params!(
                        "name" => "current",
                        "value" => Param::reference(item),
                    ),
                )?;
                Ok(())
            },
        )
        .unwrap();

        let ready = pipe.compile().unwrap();
        let doc = lift(&ready, &core_ops()).expect("lift ok");

        match &doc.steps[0] {
            Node::IterArray { name, source, body } => {
                assert_eq!(name, "walk");
                assert_eq!(source, "items");
                assert_eq!(body.len(), 1);
                match &body[0] {
                    // The inner step's core name is `walk.capture`, but in
                    // the lifted document it shows as the un-prefixed
                    // `capture` — the whole point of the unflattening pass.
                    Node::Step { name, op, .. } => {
                        assert_eq!(name, "capture");
                        assert_eq!(op, "SetVar");
                    }
                    other => panic!("expected inner Step, got {other:?}"),
                }
            }
            other => panic!("expected IterArray, got {other:?}"),
        }
    }

    #[test]
    fn lift_guard_unflattens_body_names() {
        let mut pipe = Pipeline::<_>::new();
        pipe.var("flag", true).unwrap();

        pipe.guard(
            "gate",
            panopticon_core::extend::GuardSource::boolean("flag"),
            |body| {
                body.step::<SetVar>(
                    "produce",
                    panopticon_core::extend::params!(
                        "name" => "gated",
                        "value" => "yes",
                    ),
                )?;
                Ok(())
            },
        )
        .unwrap();

        let ready = pipe.compile().unwrap();
        let doc = lift(&ready, &core_ops()).expect("lift ok");

        match &doc.steps[0] {
            Node::Guard { name, source, body } => {
                assert_eq!(name, "gate");
                assert_eq!(source, "flag");
                match &body[0] {
                    Node::Step { name, .. } => assert_eq!(name, "produce"),
                    other => panic!("expected inner Step, got {other:?}"),
                }
            }
            other => panic!("expected Guard, got {other:?}"),
        }
    }

    #[test]
    fn lift_returns_block() {
        let mut pipe = Pipeline::<_>::new();
        pipe.var("greeting", "hi").unwrap();
        pipe.returns(
            "summary",
            panopticon_core::extend::params!(
                "greeting" => Param::reference("greeting"),
            ),
        )
        .unwrap();

        let ready = pipe.compile().unwrap();
        let doc = lift(&ready, &core_ops()).expect("lift ok");

        let summary = doc.returns.get("summary").expect("summary block present");
        assert_eq!(
            summary.get("greeting"),
            Some(&ParamSpec::Ref("greeting".into()))
        );
    }

    #[test]
    fn unregistered_op_errors() {
        let mut pipe = Pipeline::<_>::new();
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

        // Build an op map that has a `SetVar` thunk for lowering but no
        // reverse TypeId entry — simulating a draft constructed from YAML
        // through a legitimate deserialiser, but serialised with an ops
        // table where `SetVar` was inserted via the forward-only `insert`
        // escape hatch.
        let source = core_ops();
        let thunk = source.get("SetVar").unwrap();
        let mut ops_no_reverse = OperationRegistrationMap::new();
        ops_no_reverse.insert("SetVar", thunk);

        match lift(&ready, &ops_no_reverse) {
            Err(SchemaError::UnregisteredOperation { .. }) => {}
            Err(other) => panic!("expected UnregisteredOperation, got {other:?}"),
            Ok(_) => panic!("expected error"),
        }
    }
}
