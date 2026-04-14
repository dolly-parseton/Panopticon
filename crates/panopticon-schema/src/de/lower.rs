//! Walk a [`Document`] and build a `Pipeline<Draft>` via the public core API.
//!
//! This module is the bridge between the serde schema types and
//! `panopticon-core`. It is deliberately a flat set of functions rather
//! than a struct — each function takes what it needs and has no hidden
//! state.
//!
//! Most callers should use [`crate::Deserialiser::from_yaml`] rather than
//! calling [`lower`] directly. The direct entry point is public for
//! advanced use cases such as building a [`Document`] programmatically
//! (e.g. from a GUI builder) and skipping the YAML parse step.
//!
//! Hooks and extensions are not handled here. The resulting draft is
//! bare; the caller must register any required hooks/extensions on it
//! before calling `compile()`.

use std::collections::{BTreeMap, HashMap};

use panopticon_core::extend::{
    Draft, GuardSource, IterSource, Param, Parameters, Pipeline, Value,
};

use crate::error::SchemaError;
use crate::registry::OperationRegistrationMap;
use crate::schema::document::{Document, Node};
use crate::schema::param::ParamSpec;
use crate::schema::value::{LiteralValue, VariableSpec};

/// Lower a parsed [`Document`] into a fresh `Pipeline<Draft>`.
///
/// Operation references are resolved through `ops` at walk time. Unknown
/// operation names produce [`SchemaError::UnknownOperation`]; everything
/// else — ref resolution, type correctness, duplicate names, forward
/// references — is deferred to `Pipeline<Draft>::compile()`.
///
/// This function walks the document in natural order: variables first,
/// then steps (recursively for iteration and guard bodies), then return
/// projections. The resulting draft carries no hooks or extensions.
pub fn lower(
    doc: Document,
    ops: &OperationRegistrationMap,
) -> Result<Pipeline<Draft>, SchemaError> {
    let mut pipe = Pipeline::new();
    lower_variables(&mut pipe, doc.variables)?;
    for node in doc.steps {
        lower_node(&mut pipe, node, ops)?;
    }
    lower_returns(&mut pipe, doc.returns)?;
    Ok(pipe)
}

fn lower_variables(
    pipe: &mut Pipeline<Draft>,
    vars: BTreeMap<String, VariableSpec>,
) -> Result<(), SchemaError> {
    for (name, spec) in vars {
        match spec {
            VariableSpec::Text(s) => {
                pipe.var(name, s)?;
            }
            VariableSpec::Integer(i) => {
                pipe.var(name, i)?;
            }
            VariableSpec::Float(f) => {
                pipe.var(name, f)?;
            }
            VariableSpec::Boolean(b) => {
                pipe.var(name, b)?;
            }
            VariableSpec::Null => {
                pipe.var(name, Value::Null)?;
            }
            VariableSpec::Array(items) => {
                let mut handle = pipe.array(name)?;
                for item in items {
                    handle.push(literal_to_value(item))?;
                }
            }
            VariableSpec::Map(entries) => {
                let mut handle = pipe.map(name)?;
                for (k, v) in entries {
                    handle.insert(k, literal_to_value(v))?;
                }
            }
        }
    }
    Ok(())
}

fn lower_node(
    pipe: &mut Pipeline<Draft>,
    node: Node,
    ops: &OperationRegistrationMap,
) -> Result<(), SchemaError> {
    match node {
        Node::Step { name, op, params } => {
            let thunk = ops
                .get(&op)
                .ok_or_else(|| SchemaError::UnknownOperation(op.clone()))?;
            let parameters = lower_params(params)?;
            thunk(pipe, name, parameters)?;
        }
        Node::IterArray {
            name,
            source,
            body,
        } => {
            let mut child = Pipeline::new();
            for n in body {
                lower_node(&mut child, n, ops)?;
            }
            pipe.iter_array_from_child(name, IterSource::array(source), child)?;
        }
        Node::IterMap {
            name,
            source,
            body,
        } => {
            let mut child = Pipeline::new();
            for n in body {
                lower_node(&mut child, n, ops)?;
            }
            pipe.iter_map_from_child(name, IterSource::map(source), child)?;
        }
        Node::Guard {
            name,
            source,
            body,
        } => {
            let mut child = Pipeline::new();
            for n in body {
                lower_node(&mut child, n, ops)?;
            }
            pipe.guard_from_child(name, GuardSource::boolean(source), child)?;
        }
    }
    Ok(())
}

fn lower_returns(
    pipe: &mut Pipeline<Draft>,
    returns: BTreeMap<String, BTreeMap<String, ParamSpec>>,
) -> Result<(), SchemaError> {
    for (name, param_map) in returns {
        let parameters = lower_params(param_map)?;
        pipe.returns(name, parameters)?;
    }
    Ok(())
}

fn lower_params(specs: BTreeMap<String, ParamSpec>) -> Result<Parameters, SchemaError> {
    let mut out: HashMap<String, Param> = HashMap::with_capacity(specs.len());
    for (k, v) in specs {
        out.insert(k, lower_param(v)?);
    }
    Ok(Parameters::new(out))
}

fn lower_param(spec: ParamSpec) -> Result<Param, SchemaError> {
    Ok(match spec {
        ParamSpec::Literal(lit) => Param::literal(literal_to_value(lit)),
        ParamSpec::Ref(path) => Param::reference(path),
        ParamSpec::Template(parts) => Param::template(
            parts
                .into_iter()
                .map(lower_param)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        ParamSpec::Array(items) => Param::array(
            items
                .into_iter()
                .map(lower_param)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        ParamSpec::Map(entries) => {
            let mut map: HashMap<String, Param> = HashMap::with_capacity(entries.len());
            for (k, v) in entries {
                map.insert(k, lower_param(v)?);
            }
            Param::map(map)
        }
    })
}

fn literal_to_value(lit: LiteralValue) -> Value {
    match lit {
        LiteralValue::Text(s) => Value::Text(s),
        LiteralValue::Integer(i) => Value::Integer(i),
        LiteralValue::Float(f) => Value::Float(f),
        LiteralValue::Boolean(b) => Value::Boolean(b),
        LiteralValue::Null => Value::Null,
    }
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

    fn text(s: &str) -> ParamSpec {
        ParamSpec::Literal(LiteralValue::Text(s.into()))
    }

    fn ref_(s: &str) -> ParamSpec {
        ParamSpec::Ref(s.into())
    }

    #[test]
    fn variables_only_document_compiles() {
        let mut vars = BTreeMap::new();
        vars.insert("name".to_string(), VariableSpec::Text("world".into()));
        vars.insert("count".to_string(), VariableSpec::Integer(3));
        vars.insert(
            "items".to_string(),
            VariableSpec::Array(vec![
                LiteralValue::Integer(1),
                LiteralValue::Integer(2),
            ]),
        );

        let doc = Document {
            version: crate::schema::version::Version::V1,
            variables: vars,
            steps: vec![],
            returns: BTreeMap::new(),
        };

        let pipe = lower(doc, &core_ops()).expect("lower ok");
        pipe.compile().expect("compile ok");
    }

    #[test]
    fn single_step_runs_end_to_end() {
        let mut vars = BTreeMap::new();
        vars.insert("source".to_string(), VariableSpec::Text("hello".into()));

        let mut params = BTreeMap::new();
        params.insert("name".into(), text("greeting"));
        params.insert("value".into(), ref_("source"));

        let steps = vec![Node::Step {
            name: "set_greeting".into(),
            op: "SetVar".into(),
            params,
        }];

        let mut ret_block = BTreeMap::new();
        ret_block.insert("greeting".into(), ref_("greeting"));
        let mut returns = BTreeMap::new();
        returns.insert("output".into(), ret_block);

        let doc = Document {
            version: crate::schema::version::Version::V1,
            variables: vars,
            steps,
            returns,
        };

        let pipe = lower(doc, &core_ops()).unwrap();
        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn iter_array_body_runs() {
        let mut vars = BTreeMap::new();
        vars.insert(
            "items".to_string(),
            VariableSpec::Array(vec![
                LiteralValue::Integer(10),
                LiteralValue::Integer(20),
            ]),
        );

        let mut inner_params = BTreeMap::new();
        inner_params.insert("name".into(), text("current"));
        inner_params.insert("value".into(), ref_("loop.__item"));

        let body = vec![Node::Step {
            name: "capture".into(),
            op: "SetVar".into(),
            params: inner_params,
        }];

        let steps = vec![Node::IterArray {
            name: "loop".into(),
            source: "items".into(),
            body,
        }];

        let doc = Document {
            version: crate::schema::version::Version::V1,
            variables: vars,
            steps,
            returns: BTreeMap::new(),
        };

        let pipe = lower(doc, &core_ops()).unwrap();
        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn guard_with_downstream_consumer_runs() {
        let mut vars = BTreeMap::new();
        vars.insert("flag".to_string(), VariableSpec::Boolean(true));

        // Guard body produces a variable, then a downstream step consumes it.
        let mut inner_params = BTreeMap::new();
        inner_params.insert("name".into(), text("guarded_value"));
        inner_params.insert("value".into(), text("present"));

        let guard_body = vec![Node::Step {
            name: "produce".into(),
            op: "SetVar".into(),
            params: inner_params,
        }];

        let mut consumer_params = BTreeMap::new();
        consumer_params.insert("name".into(), text("final"));
        consumer_params.insert("value".into(), ref_("guarded_value"));

        let steps = vec![
            Node::Guard {
                name: "check".into(),
                source: "flag".into(),
                body: guard_body,
            },
            Node::Step {
                name: "consume".into(),
                op: "SetVar".into(),
                params: consumer_params,
            },
        ];

        let doc = Document {
            version: crate::schema::version::Version::V1,
            variables: vars,
            steps,
            returns: BTreeMap::new(),
        };

        let pipe = lower(doc, &core_ops()).unwrap();
        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn null_variable_lowers_and_compiles() {
        let mut vars = BTreeMap::new();
        vars.insert("slot".to_string(), VariableSpec::Null);

        let doc = Document {
            version: crate::schema::version::Version::V1,
            variables: vars,
            steps: vec![],
            returns: BTreeMap::new(),
        };

        let pipe = lower(doc, &core_ops()).expect("lower ok");
        pipe.compile().expect("compile ok");
    }

    #[test]
    fn unknown_operation_errors() {
        let mut params = BTreeMap::new();
        params.insert("name".into(), text("x"));
        params.insert("value".into(), text("y"));

        let doc = Document {
            version: crate::schema::version::Version::V1,
            variables: BTreeMap::new(),
            steps: vec![Node::Step {
                name: "bad".into(),
                op: "DoesNotExist".into(),
                params,
            }],
            returns: BTreeMap::new(),
        };

        match lower(doc, &core_ops()) {
            Err(SchemaError::UnknownOperation(name)) => assert_eq!(name, "DoesNotExist"),
            Err(other) => panic!("expected UnknownOperation, got {other:?}"),
            Ok(_) => panic!("expected UnknownOperation error"),
        }
    }
}
