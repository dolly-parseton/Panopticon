//! Schema data model.
//!
//! The types in this subtree describe the shape of a v1 workflow YAML
//! document. They implement both `Deserialize` and `Serialize` and are
//! consumed by `crate::de::lower` (when loading YAML into a
//! `Pipeline<Draft>`) and produced by `crate::ser::lift` (when lifting a
//! `Pipeline<Ready>` back into a document).
//!
//! The data model is deliberately a closed set — no trait objects, no
//! borrowed data, no hidden construction — so it can be walked by both
//! directions without surprise.

pub mod document;
pub mod param;
pub mod value;
pub mod version;

pub use document::{Document, Node};
pub use param::ParamSpec;
pub use value::{LiteralValue, VariableSpec};
pub use version::Version;
