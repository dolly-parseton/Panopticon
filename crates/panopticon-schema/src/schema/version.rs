//! Schema version discriminant used for version-aware dispatch in
//! [`crate::Deserialiser::from_yaml`].
//!
//! Adding a future schema revision (v2, v3, ...) means extending [`Version`]
//! with a new variant and adding a new arm to the `match` inside
//! `from_yaml` that parses into a new `DocumentV2` / `DocumentV3` struct.
//! Existing v1 documents continue to parse unchanged.

use serde::{Deserialize, Serialize};

/// Schema version. YAML documents declare their version at the root as
/// `version: v1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Version {
    /// Schema v1 — the only currently supported revision. See the
    /// `SCHEMA_V1.md` file in the crate root for the complete format
    /// reference.
    V1,
}

impl Version {
    /// The most recent schema version the crate supports.
    pub const CURRENT: Version = Version::V1;
}

/// Helper used by [`crate::Deserialiser::from_yaml`] to peek the `version`
/// field of a document before committing to a concrete schema struct.
#[derive(Debug, Deserialize)]
pub(crate) struct VersionOnly {
    pub version: Version,
}
