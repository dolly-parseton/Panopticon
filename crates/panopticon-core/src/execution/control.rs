#[allow(unused_imports)]
use crate::imports::*;

/// A draft-time reference to the store entry an iteration node should walk.
///
/// Passed to [`Pipeline::iter_array`] or [`Pipeline::iter_map`] to name the
/// array or map the body iterates over. At runtime the reference is
/// resolved against the runtime store; if the resolved entry is missing or
/// has the wrong shape, the iteration fails with
/// [`OperationError::IterSourceNotFound`] or
/// [`OperationError::IterSourceTypeMismatch`].
pub struct IterSource {
    pub(crate) reference: String,
}

impl IterSource {
    /// Builds an iter source pointing at an array entry. Use with
    /// [`Pipeline::iter_array`].
    pub fn array(reference: impl Into<String>) -> Self {
        IterSource {
            reference: reference.into(),
        }
    }
    /// Builds an iter source pointing at a map entry. Use with
    /// [`Pipeline::iter_map`].
    pub fn map(reference: impl Into<String>) -> Self {
        IterSource {
            reference: reference.into(),
        }
    }
    /// Read-only access to the underlying store reference. Used by
    /// external callers (e.g. serialisers) that need to walk a validated
    /// execution graph.
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

/// A draft-time reference to the boolean entry a guard node checks.
///
/// Passed to [`Pipeline::guard`] to name the boolean that gates the guard
/// body. At runtime the reference is resolved against the runtime store;
/// if the resolved entry is not a boolean the guard fails with
/// [`OperationError::GuardTypeMismatch`].
pub struct GuardSource {
    pub(crate) reference: String,
}

impl GuardSource {
    /// Builds a guard source pointing at a boolean entry. Use with
    /// [`Pipeline::guard`].
    pub fn boolean(reference: impl Into<String>) -> Self {
        GuardSource {
            reference: reference.into(),
        }
    }
    /// Read-only access to the underlying store reference. Used by
    /// external callers (e.g. serialisers) that need to walk a validated
    /// execution graph.
    pub fn reference(&self) -> &str {
        &self.reference
    }
}
