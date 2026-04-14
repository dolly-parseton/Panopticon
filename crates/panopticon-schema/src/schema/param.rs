//! Schema-side representation of `panopticon_core::extend::Param`.
//!
//! Every value inside a `params:` mapping (on a step node) or inside a
//! `returns:` projection is a [`ParamSpec`]. The five variants mirror
//! core's `Param` enum and are adjacently tagged in YAML — a two-field
//! mapping with `type` (the variant discriminant) and `value` (the
//! variant payload):
//!
//! ```yaml
//! # Literal
//! type: literal
//! value:
//!   type: text
//!   value: hello
//!
//! # Reference
//! type: ref
//! value: greeting
//!
//! # Template (string concatenation)
//! type: template
//! value:
//!   - type: literal
//!     value:
//!       type: text
//!       value: "hello, "
//!   - type: ref
//!     value: target
//! ```
//!
//! `ParamSpec` is recursive: [`Template`][ParamSpec::Template],
//! [`Array`][ParamSpec::Array], and [`Map`][ParamSpec::Map] all contain
//! nested `ParamSpec` values.

use super::value::LiteralValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Schema-side representation of a parameter value. Lowered to
/// `panopticon_core::extend::Param` when a [`Document`] is handed to
/// [`crate::Deserialiser::from_document`].
///
/// [`Document`]: crate::types::Document
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ParamSpec {
    /// A typed constant value. Lowered to `Param::Literal`.
    Literal(LiteralValue),
    /// A reference to another variable or step output in the store.
    /// Lowered to `Param::Ref`.
    Ref(String),
    /// A template of nested `ParamSpec` parts. At execution time each
    /// part is resolved, stringified, and concatenated into a single
    /// `text` value. Lowered to `Param::Template`.
    Template(Vec<ParamSpec>),
    /// A parameter-value array. Each element is itself a `ParamSpec`.
    /// Lowered to `Param::Array`.
    Array(Vec<ParamSpec>),
    /// A parameter-value map. Each value is itself a `ParamSpec`. Lowered
    /// to `Param::Map`.
    Map(BTreeMap<String, ParamSpec>),
}

/// Human-readable formatting for diagnostics. Renders in a compact
/// expression-like form:
///
/// - `Literal(v)` → the underlying `LiteralValue`'s `Display` output.
/// - `Ref(name)` → `$name`.
/// - `Template([..])` → `template(<parts>)`.
/// - `Array([..])` → `[<items>]`.
/// - `Map({..})` → `{k1: v1, k2: v2}`.
///
/// Recursive variants delegate to `Display` on their children.
impl fmt::Display for ParamSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamSpec::Literal(lit) => write!(f, "{lit}"),
            ParamSpec::Ref(path) => write!(f, "${path}"),
            ParamSpec::Template(parts) => {
                write!(f, "template(")?;
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{part}")?;
                }
                write!(f, ")")
            }
            ParamSpec::Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            ParamSpec::Map(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_spec_display() {
        let lit = ParamSpec::Literal(LiteralValue::Text("hi".into()));
        assert_eq!(format!("{lit}"), r#""hi""#);

        let r = ParamSpec::Ref("target".into());
        assert_eq!(format!("{r}"), "$target");

        let t = ParamSpec::Template(vec![
            ParamSpec::Literal(LiteralValue::Text("hello, ".into())),
            ParamSpec::Ref("name".into()),
        ]);
        assert_eq!(format!("{t}"), r#"template("hello, ", $name)"#);

        let a = ParamSpec::Array(vec![
            ParamSpec::Literal(LiteralValue::Integer(1)),
            ParamSpec::Ref("x".into()),
        ]);
        assert_eq!(format!("{a}"), "[1, $x]");

        let mut m = BTreeMap::new();
        m.insert("greeting".into(), ParamSpec::Ref("g".into()));
        m.insert("count".into(), ParamSpec::Literal(LiteralValue::Integer(3)));
        let map = ParamSpec::Map(m);
        // BTreeMap iterates alphabetically: count then greeting.
        assert_eq!(format!("{map}"), "{count: 3, greeting: $g}");
    }

    #[test]
    fn literal_parses() {
        let y = "
type: literal
value:
  type: text
  value: greeting
";
        let p: ParamSpec = serde_yaml::from_str(y).unwrap();
        assert_eq!(p, ParamSpec::Literal(LiteralValue::Text("greeting".into())));
    }

    #[test]
    fn null_literal_parses() {
        let y = "
type: literal
value:
  type: none
";
        let p: ParamSpec = serde_yaml::from_str(y).unwrap();
        assert_eq!(p, ParamSpec::Literal(LiteralValue::Null));
    }

    #[test]
    fn ref_parses() {
        let p: ParamSpec = serde_yaml::from_str("type: ref\nvalue: target").unwrap();
        assert_eq!(p, ParamSpec::Ref("target".into()));
    }

    #[test]
    fn template_parses() {
        let y = "
type: template
value:
  - type: literal
    value:
      type: text
      value: \"hello, \"
  - type: ref
    value: target
";
        let p: ParamSpec = serde_yaml::from_str(y).unwrap();
        assert_eq!(
            p,
            ParamSpec::Template(vec![
                ParamSpec::Literal(LiteralValue::Text("hello, ".into())),
                ParamSpec::Ref("target".into()),
            ])
        );
    }

    #[test]
    fn array_parses() {
        let y = "
type: array
value:
  - type: literal
    value:
      type: integer
      value: 1
  - type: ref
    value: x
";
        let p: ParamSpec = serde_yaml::from_str(y).unwrap();
        assert_eq!(
            p,
            ParamSpec::Array(vec![
                ParamSpec::Literal(LiteralValue::Integer(1)),
                ParamSpec::Ref("x".into()),
            ])
        );
    }

    #[test]
    fn map_parses() {
        let y = "
type: map
value:
  signin:
    type: ref
    value: current_item
  user:
    type: ref
    value: entra_user
";
        let p: ParamSpec = serde_yaml::from_str(y).unwrap();
        let mut expected = BTreeMap::new();
        expected.insert("signin".into(), ParamSpec::Ref("current_item".into()));
        expected.insert("user".into(), ParamSpec::Ref("entra_user".into()));
        assert_eq!(p, ParamSpec::Map(expected));
    }

    #[test]
    fn unknown_tag_rejected() {
        let err = serde_yaml::from_str::<ParamSpec>("type: reference\nvalue: target").unwrap_err();
        assert!(err.to_string().contains("unknown variant"));
    }
}
