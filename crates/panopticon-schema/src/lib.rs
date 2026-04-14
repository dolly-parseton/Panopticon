//! Strict-schema YAML deserialisation for [`panopticon-core`] pipelines.
//!
//! This crate converts YAML documents conforming to a strict, versioned
//! schema into `panopticon_core::extend::Pipeline<Draft>` values. The
//! resulting draft pipelines are identical to ones built by hand through
//! core's fluent API — they go through the same `compile()` validation and
//! the same runtime.
//!
//! # Separation of concerns
//!
//! The schema describes **pipeline structure only**: variables, steps,
//! iteration, guards, and return projections. It does *not* describe hooks
//! or extensions. Hooks are closures and extensions are trait objects;
//! neither is serialisable, and both are the host program's responsibility:
//!
//! 1. Parse YAML → `Pipeline<Draft>` via [`Deserialiser::from_yaml`].
//! 2. Register any required hooks on the draft via `pipe.hook(...)`.
//! 3. Register any required extensions on the draft via `pipe.extension(...)`.
//! 4. Compile and run: `pipe.compile()?.run().wait()?`.
//!
//! # Operation name resolution
//!
//! YAML step nodes reference operations by string name (e.g. `op: SetVar`).
//! At load time each name is resolved to a concrete Rust operation type via
//! an [`OperationRegistrationMap`] built by the [`register_ops!`] macro.
//! The macro expands to a table of non-capturing closures that call core's
//! generic `Pipeline::step::<O>()`, so operation types are statically
//! resolved — there is no runtime trait-object dispatch.
//!
//! # Minimal example
//!
//! ```no_run
//! use panopticon_core::extend::SetVar;
//! use panopticon_schema::{register_ops, Deserialiser};
//!
//! let yaml = r#"
//! version: v1
//! variables:
//!   target:
//!     type: text
//!     value: world
//! steps:
//!   - name: build_greeting
//!     type: step
//!     op: SetVar
//!     params:
//!       name:
//!         type: literal
//!         value:
//!           type: text
//!           value: greeting
//!       value:
//!         type: template
//!         value:
//!           - type: literal
//!             value:
//!               type: text
//!               value: "hello, "
//!           - type: ref
//!             value: target
//! returns:
//!   out:
//!     greeting:
//!       type: ref
//!       value: greeting
//! "#;
//!
//! let ops = register_ops!(
//!     "SetVar" => SetVar,
//! );
//!
//! let draft = Deserialiser::new(ops).from_yaml(yaml)?;
//! let complete = draft.compile()?.run().wait()?;
//! complete.debug();
//! # Ok::<_, Box<dyn std::error::Error>>(())
//! ```
//!
//! # Round-tripping a ready pipeline back to YAML
//!
//! [`Serialiser`] is the symmetric counterpart to [`Deserialiser`]. Both
//! share an [`OperationRegistrationMap`], and both honour the same
//! schema-level invariants:
//!
//! ```no_run
//! # use panopticon_core::extend::{Param, Parameters, Pipeline, SetVar};
//! # use panopticon_schema::{register_ops, Serialiser};
//! let mut pipe = Pipeline::new();
//! pipe.var("target", "world")?;
//! pipe.step::<SetVar>(
//!     "build_greeting",
//!     panopticon_core::extend::params!(
//!         "name" => "greeting",
//!         "value" => Param::reference("target"),
//!     ),
//! )?;
//!
//! let ready = pipe.compile()?;
//! let serialiser = Serialiser::new(register_ops!("SetVar" => SetVar));
//! let yaml = serialiser.to_yaml(&ready)?;
//! println!("{yaml}");
//! # Ok::<_, Box<dyn std::error::Error>>(())
//! ```
//!
//! The round-trip `from_yaml → compile → to_yaml` preserves document
//! structure (key ordering and whitespace may differ). Ops must have
//! been registered with
//! [`OperationRegistrationMap::register`] or the `register_ops!` macro
//! for the reverse `TypeId → name` lookup to succeed — ops added via the
//! forward-only [`OperationRegistrationMap::insert`] escape hatch cannot
//! be serialised.
//!
//! # Strictness
//!
//! The schema rejects unknown fields, unknown enum variants, and wrong
//! shapes at parse time. See `SCHEMA_V1.md` in the crate root for the
//! complete format reference, including scoping rules and known
//! limitations.
//!
//! # Entry points
//!
//! Five named items at the crate root plus the [`types`] submodule cover
//! the whole public surface. Basic users only touch the facades
//! ([`Deserialiser`], [`Serialiser`]) and their error type; power users
//! reach for the schema data model via `panopticon_schema::types::*`.
//!
//! - [`Deserialiser::from_yaml`] — YAML string → draft.
//! - [`Deserialiser::from_document`] — in-memory [`types::Document`] →
//!   draft, skipping YAML parsing.
//! - [`Serialiser::to_yaml`] — ready pipeline → YAML string.
//! - [`Serialiser::to_document`] — ready pipeline → in-memory
//!   [`types::Document`] without YAML emission.
//! - [`register_ops!`] — build an [`OperationRegistrationMap`] from a
//!   list of `"name" => OpType` pairs.
//!
//! [`panopticon-core`]: https://docs.rs/panopticon-core

mod de;
mod error;
mod registry;
mod schema;
mod ser;

/// YAML / [`types::Document`] → `Pipeline<Draft>`.
pub use de::Deserialiser;
pub use error::SchemaError;
pub use registry::{OperationRegistrationMap, StepThunk};
/// `Pipeline<Ready>` → YAML / [`types::Document`].
pub use ser::Serialiser;

/// Schema data model types.
///
/// Construct, inspect, or transform workflow documents in memory. Most
/// users reach these types indirectly via
/// [`crate::Serialiser::to_document`] or
/// [`crate::Deserialiser::from_document`] rather than naming them
/// explicitly.
pub mod types {
    pub use crate::schema::{
        Document, LiteralValue, Node, ParamSpec, VariableSpec, Version,
    };
}
