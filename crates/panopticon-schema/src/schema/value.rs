//! Typed scalar literals and variable declarations.
//!
//! Both [`LiteralValue`] and [`VariableSpec`] are adjacently tagged serde
//! enums: each value is a YAML mapping with two fields, `type` (the variant
//! discriminant) and `value` (the variant payload). For example:
//!
//! ```yaml
//! # LiteralValue / VariableSpec text
//! type: text
//! value: "hello"
//!
//! # LiteralValue / VariableSpec integer
//! type: integer
//! value: 42
//! ```
//!
//! Explicit type tags are required because YAML does not distinguish
//! integers from floats on its own, and bare scalars would make it
//! impossible to disambiguate a literal from a reference.
//!
//! ## Null
//!
//! The `Null` variant uses the primary spelling `type: none` to avoid
//! colliding with YAML's null keyword in key positions. Two aliases are
//! accepted:
//!
//! - `type: null_value` — unambiguous, unquoted.
//! - `type: "null"` — quoted to force string interpretation. Bare
//!   unquoted `type: null` fails because YAML resolves it to the null
//!   scalar, not the string `"null"`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Typed literal value used inside parameters and variable declarations.
///
/// Mirrors `panopticon_core::extend::Value`. See the module-level docs
/// for the YAML form and for null-spelling rules.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum LiteralValue {
    /// Text (UTF-8 string). Lowered to `Value::Text`.
    Text(String),
    /// Signed 64-bit integer. Lowered to `Value::Integer`.
    Integer(i64),
    /// 64-bit float. Lowered to `Value::Float`.
    Float(f64),
    /// Boolean. Lowered to `Value::Boolean`.
    Boolean(bool),
    /// Null literal. Lowered to `Value::Null`. Primary YAML spelling is
    /// `type: none`; also accepts `type: null_value` and `type: "null"`.
    #[serde(rename = "none", alias = "null", alias = "null_value")]
    Null,
}

/// Human-readable formatting for diagnostics. Scalars render as their
/// Rust-literal form: text is quoted, integers and floats use their
/// native formatting, booleans are `true`/`false`, and null renders as
/// `null`.
impl fmt::Display for LiteralValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralValue::Text(s) => write!(f, "{s:?}"),
            LiteralValue::Integer(i) => write!(f, "{i}"),
            LiteralValue::Float(v) => write!(f, "{v}"),
            LiteralValue::Boolean(b) => write!(f, "{b}"),
            LiteralValue::Null => write!(f, "null"),
        }
    }
}

/// Variable declaration appearing under the top-level `variables:` mapping.
///
/// Scalar variants are lowered to `StoreEntry::Var`. [`Array`][Self::Array]
/// is lowered to `StoreEntry::Array` via `Pipeline::array()`, and
/// [`Map`][Self::Map] to `StoreEntry::Map` via `Pipeline::map()`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum VariableSpec {
    /// Text variable.
    Text(String),
    /// Integer variable.
    Integer(i64),
    /// Float variable.
    Float(f64),
    /// Boolean variable.
    Boolean(bool),
    /// Array variable whose elements are typed literals. Nested arrays
    /// and maps inside array elements are not representable in v1.
    Array(Vec<LiteralValue>),
    /// Map variable whose values are typed literals. Nested arrays and
    /// maps inside map values are not representable in v1.
    Map(BTreeMap<String, LiteralValue>),
    /// Null variable. See the module-level docs for the accepted
    /// spellings (`none`, `null_value`, quoted `"null"`).
    #[serde(rename = "none", alias = "null", alias = "null_value")]
    Null,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_value_serde_round_trip() {
        let original = LiteralValue::Text("hello".into());
        let yaml = serde_yaml::to_string(&original).unwrap();
        let round_tripped: LiteralValue = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(original, round_tripped);

        let null = LiteralValue::Null;
        let yaml = serde_yaml::to_string(&null).unwrap();
        let round_tripped: LiteralValue = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(null, round_tripped);
    }

    #[test]
    fn literal_value_display() {
        assert_eq!(format!("{}", LiteralValue::Text("hi".into())), r#""hi""#);
        assert_eq!(format!("{}", LiteralValue::Integer(42)), "42");
        assert_eq!(format!("{}", LiteralValue::Float(3.5)), "3.5");
        assert_eq!(format!("{}", LiteralValue::Boolean(true)), "true");
        assert_eq!(format!("{}", LiteralValue::Null), "null");
    }

    #[test]
    fn literal_text_parses() {
        let y = "
type: text
value: hello
";
        let v: LiteralValue = serde_yaml::from_str(y).unwrap();
        assert_eq!(v, LiteralValue::Text("hello".into()));
    }

    #[test]
    fn literal_integer_parses() {
        let v: LiteralValue = serde_yaml::from_str("type: integer\nvalue: 42").unwrap();
        assert_eq!(v, LiteralValue::Integer(42));
    }

    #[test]
    fn literal_float_parses() {
        let v: LiteralValue = serde_yaml::from_str("type: float\nvalue: 3.5").unwrap();
        assert_eq!(v, LiteralValue::Float(3.5));
    }

    #[test]
    fn literal_boolean_parses() {
        let v: LiteralValue = serde_yaml::from_str("type: boolean\nvalue: true").unwrap();
        assert_eq!(v, LiteralValue::Boolean(true));
    }

    #[test]
    fn literal_null_primary_spelling_parses() {
        let v: LiteralValue = serde_yaml::from_str("type: none").unwrap();
        assert_eq!(v, LiteralValue::Null);
    }

    #[test]
    fn literal_null_alias_null_value_parses() {
        let v: LiteralValue = serde_yaml::from_str("type: null_value").unwrap();
        assert_eq!(v, LiteralValue::Null);
    }

    #[test]
    fn literal_null_alias_quoted_null_parses() {
        let v: LiteralValue = serde_yaml::from_str(r#"type: "null""#).unwrap();
        assert_eq!(v, LiteralValue::Null);
    }

    #[test]
    fn literal_null_bare_unquoted_null_parses() {
        // YAML's null scalar in tag position is coerced to the string
        // "null" by serde_yaml during adjacently-tagged enum dispatch,
        // so the `alias = "null"` on the Null variant matches cleanly.
        // Bare `type: null` is therefore a fourth accepted spelling in
        // addition to `none`, `null_value`, and quoted `"null"`.
        let v: LiteralValue = serde_yaml::from_str("type: null").unwrap();
        assert_eq!(v, LiteralValue::Null);
    }

    #[test]
    fn literal_unknown_tag_rejected() {
        let err = serde_yaml::from_str::<LiteralValue>("type: string\nvalue: hello").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown variant") || msg.contains("string"), "{msg}");
    }

    #[test]
    fn variable_scalar_parses() {
        let v: VariableSpec = serde_yaml::from_str("type: text\nvalue: world").unwrap();
        assert_eq!(v, VariableSpec::Text("world".into()));
    }

    #[test]
    fn variable_null_parses() {
        let v: VariableSpec = serde_yaml::from_str("type: none").unwrap();
        assert_eq!(v, VariableSpec::Null);
    }

    #[test]
    fn variable_array_parses() {
        let y = "
type: array
value:
  - type: integer
    value: 1
  - type: integer
    value: 2
  - type: integer
    value: 3
";
        let v: VariableSpec = serde_yaml::from_str(y).unwrap();
        assert_eq!(
            v,
            VariableSpec::Array(vec![
                LiteralValue::Integer(1),
                LiteralValue::Integer(2),
                LiteralValue::Integer(3),
            ])
        );
    }

    #[test]
    fn variable_map_parses() {
        let y = "
type: map
value:
  host:
    type: text
    value: localhost
  port:
    type: text
    value: \"8080\"
";
        let v: VariableSpec = serde_yaml::from_str(y).unwrap();
        let mut expected = BTreeMap::new();
        expected.insert("host".into(), LiteralValue::Text("localhost".into()));
        expected.insert("port".into(), LiteralValue::Text("8080".into()));
        assert_eq!(v, VariableSpec::Map(expected));
    }
}
