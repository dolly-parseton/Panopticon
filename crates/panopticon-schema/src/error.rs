//! Error type surfaced by every fallible entry point in the crate.
//!
//! [`SchemaError`] wraps every error class the crate can produce:
//!
//! - YAML parse / shape failures come via `serde_yaml::Error`, which includes
//!   line and column information for syntactic errors and unknown-field /
//!   unknown-variant rejections.
//! - Workflow authorship errors (referencing an op that was not registered)
//!   come via [`SchemaError::UnknownOperation`].
//! - Anything that the lowering walker hands off to `panopticon-core` —
//!   duplicate variable names, duplicate step names, forward references —
//!   surfaces through `DraftError` or `StoreError` transparently.

use std::any::TypeId;

use panopticon_core::extend::{DraftError, StoreError};
use thiserror::Error;

/// Error returned by every fallible entry point in the crate:
/// [`crate::Deserialiser::from_yaml`],
/// [`crate::Deserialiser::from_document`],
/// [`crate::Serialiser::to_yaml`], and [`crate::Serialiser::to_document`].
#[derive(Debug, Error)]
pub enum SchemaError {
    /// YAML parsing failed, or the document violated the schema's
    /// structural rules (unknown field, unknown enum variant, wrong
    /// shape, unknown schema `version` value, etc.). Also used when
    /// serialisation produces YAML that `serde_yaml` refuses to emit.
    #[error("failed to parse YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Lowering error: a step node referenced an operation name that is
    /// not present in the
    /// [`OperationRegistrationMap`](crate::OperationRegistrationMap)
    /// supplied to the [`crate::Deserialiser`].
    #[error("unknown operation: {0}")]
    UnknownOperation(String),

    /// Lifting error: a `Pipeline<Ready>` contained a step whose
    /// operation `TypeId` has no reverse entry in the
    /// [`OperationRegistrationMap`](crate::OperationRegistrationMap)
    /// supplied to the [`crate::Serialiser`]. This happens when the
    /// draft was built programmatically using an op type that was not
    /// registered via
    /// [`OperationRegistrationMap::register`](crate::OperationRegistrationMap::register)
    /// or the `register_ops!` macro.
    #[error("operation is not registered for serialisation (type_id: {type_id:?})")]
    UnregisteredOperation {
        /// The `TypeId` that failed reverse lookup.
        type_id: TypeId,
    },

    /// Lifting error: a `StoreEntry` in the ready pipeline has a shape
    /// the v1 schema cannot represent. In v1 this means a nested
    /// collection inside an array or map variable (e.g. array-of-array,
    /// map-of-map). Pipelines built purely from YAML cannot produce this
    /// shape because the schema forbids it; the error is reserved for
    /// drafts constructed programmatically.
    #[error("unsupported store shape at {path}: {reason}")]
    UnsupportedStoreShape {
        /// The store key (or key path) that carried the offending entry.
        path: String,
        /// Human-readable explanation of why the shape cannot be lifted.
        reason: String,
    },

    /// The lowering walker handed a valid-looking call to
    /// `panopticon-core` and core rejected it — e.g. a duplicate step
    /// name, a duplicate variable, or a forward reference that cannot
    /// be resolved.
    #[error(transparent)]
    Draft(#[from] DraftError),

    /// Store-level error from `panopticon-core` — e.g. pushing into an
    /// array handle failed because the entry already existed.
    #[error(transparent)]
    Store(#[from] StoreError),
}
