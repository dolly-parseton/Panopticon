//! Serialisation: `Pipeline<Ready>` → YAML.
//!
//! This module hosts the public [`Serialiser`] facade and the internal
//! [`lift`] walker that implements it. [`Serialiser`] is the symmetric
//! counterpart of [`crate::Deserialiser`] and shares the same
//! [`OperationRegistrationMap`] abstraction. Both types can be
//! constructed from the same ops table — usually a single table built
//! once at startup and cloned into each side.

pub mod lift;

use panopticon_core::extend::{Pipeline, Ready};

use crate::error::SchemaError;
use crate::registry::OperationRegistrationMap;
use crate::schema::document::Document;

/// Lifts `panopticon_core::extend::Pipeline<Ready>` instances back into
/// strict-schema YAML documents, resolving operation `TypeId`s through an
/// [`OperationRegistrationMap`] built by the `register_ops!` macro.
///
/// Ops registered via the forward-only
/// [`OperationRegistrationMap::insert`](crate::OperationRegistrationMap::insert)
/// escape hatch are **not** serialisable — they have no reverse
/// `TypeId → name` entry. Use
/// [`OperationRegistrationMap::register`](crate::OperationRegistrationMap::register)
/// or the `register_ops!` macro for full round-trip support.
///
/// Hooks and extensions on the ready pipeline are ignored: v1 YAML has no
/// way to express them, and they remain a host-program concern.
#[derive(Default, Clone)]
pub struct Serialiser {
    ops: OperationRegistrationMap,
}

impl Serialiser {
    /// Construct a serialiser that resolves operation `TypeId`s through
    /// `ops`. The map is typically built by the `register_ops!` macro.
    pub fn new(ops: OperationRegistrationMap) -> Self {
        Self { ops }
    }

    /// Read-only access to the underlying operation registration map.
    pub fn ops(&self) -> &OperationRegistrationMap {
        &self.ops
    }

    /// Lift a ready pipeline into a structured [`Document`] without
    /// emitting YAML text.
    ///
    /// Useful for programmatic round-trip tests (structural equality
    /// avoids YAML formatting variance) and for callers that want to
    /// inspect or post-process the document before writing it out.
    ///
    /// # Errors
    ///
    /// - [`SchemaError::UnregisteredOperation`] — a step node in the
    ///   ready pipeline references an op whose `TypeId` is not in the
    ///   registration map's reverse lookup.
    /// - [`SchemaError::UnsupportedStoreShape`] — a store entry has a
    ///   shape v1 cannot express (e.g. a nested collection inside an
    ///   array or map variable).
    pub fn to_document(&self, ready: &Pipeline<Ready>) -> Result<Document, SchemaError> {
        lift::lift(ready, &self.ops)
    }

    /// Lift a ready pipeline and serialise it to a YAML string.
    ///
    /// # Errors
    ///
    /// Propagates every error case from [`Serialiser::to_document`],
    /// plus [`SchemaError::Yaml`] if `serde_yaml` refuses to emit the
    /// document for any reason (in practice this should not happen for
    /// any document produced by [`Serialiser::to_document`]).
    pub fn to_yaml(&self, ready: &Pipeline<Ready>) -> Result<String, SchemaError> {
        let doc = self.to_document(ready)?;
        Ok(serde_yaml::to_string(&doc)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register_ops;
    use panopticon_core::extend::{Param, Parameters, Pipeline, SetVar};

    #[test]
    fn serialiser_to_yaml_produces_valid_yaml() {
        let mut pipe = Pipeline::<_>::new();
        pipe.var("target", "world").unwrap();
        pipe.step::<SetVar>(
            "build_greeting",
            panopticon_core::extend::params!(
                "name" => "greeting",
                "value" => Param::reference("target"),
            ),
        )
        .unwrap();
        pipe.returns(
            "out",
            panopticon_core::extend::params!(
                "greeting" => Param::reference("greeting"),
            ),
        )
        .unwrap();

        let ready = pipe.compile().unwrap();
        let ser = Serialiser::new(register_ops!("SetVar" => SetVar));

        let yaml = ser.to_yaml(&ready).unwrap();

        // The emitted YAML must parse back via the standard deserialiser.
        let parsed: Document = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.version, crate::schema::version::Version::V1);
        assert_eq!(parsed.steps.len(), 1);
        assert!(parsed.variables.contains_key("target"));
    }

    #[test]
    fn default_serialiser_has_empty_ops() {
        let s = Serialiser::default();
        assert!(s.ops().is_empty());
    }
}
