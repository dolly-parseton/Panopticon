use crate::imports::*;

/// A scalar value held inside a [`StoreEntry::Var`].
///
/// `Value` is the narrow, concrete set of primitive types the pipeline
/// understands: nothing, booleans, 64-bit integers, 64-bit floats, and
/// owned text. Collections are represented by [`StoreEntry::Array`] and
/// [`StoreEntry::Map`], not by additional `Value` variants. The enum is
/// marked `#[non_exhaustive]` so future scalar additions do not break
/// external match statements.
///
/// The `as_*` accessors narrow to a specific variant for callers that
/// know what they are holding; each returns an [`AccessError::TypeMismatch`]
/// on the wrong variant. Use [`Value::get_type`] to inspect the type tag
/// without narrowing.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// The absent value.
    Null,
    /// A boolean.
    Boolean(bool),
    /// A 64-bit signed integer.
    Integer(i64),
    /// A 64-bit floating-point number.
    Float(f64),
    /// An owned UTF-8 string.
    Text(String),
}

impl Value {
    /// Narrows to a [`Text`](Self::Text), returning a borrowed string
    /// slice. Fails with [`AccessError::TypeMismatch`] otherwise.
    pub fn as_text(&self) -> Result<&str, AccessError> {
        match self {
            Value::Text(s) => Ok(s),
            _ => Err(AccessError::TypeMismatch {
                expected: "Text",
                found: self.get_type().name(),
            }),
        }
    }
    /// Narrows to an [`Integer`](Self::Integer). Fails with
    /// [`AccessError::TypeMismatch`] otherwise.
    pub fn as_integer(&self) -> Result<i64, AccessError> {
        match self {
            Value::Integer(i) => Ok(*i),
            _ => Err(AccessError::TypeMismatch {
                expected: "Integer",
                found: self.get_type().name(),
            }),
        }
    }
    /// Narrows to a [`Float`](Self::Float). Fails with
    /// [`AccessError::TypeMismatch`] otherwise.
    pub fn as_float(&self) -> Result<f64, AccessError> {
        match self {
            Value::Float(f) => Ok(*f),
            _ => Err(AccessError::TypeMismatch {
                expected: "Float",
                found: self.get_type().name(),
            }),
        }
    }
    /// Narrows to a [`Boolean`](Self::Boolean). Fails with
    /// [`AccessError::TypeMismatch`] otherwise.
    pub fn as_boolean(&self) -> Result<bool, AccessError> {
        match self {
            Value::Boolean(b) => Ok(*b),
            _ => Err(AccessError::TypeMismatch {
                expected: "Boolean",
                found: self.get_type().name(),
            }),
        }
    }
    /// Asserts the value is [`Null`](Self::Null). Fails with
    /// [`AccessError::TypeMismatch`] otherwise.
    pub fn as_null(&self) -> Result<(), AccessError> {
        match self {
            Value::Null => Ok(()),
            _ => Err(AccessError::TypeMismatch {
                expected: "Null",
                found: self.get_type().name(),
            }),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Boolean(b)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Integer(i)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Text(s.into())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Text(s)
    }
}

impl Value {
    /// Returns the [`Type`] tag that corresponds to this value.
    pub fn get_type(&self) -> Type {
        match self {
            Value::Null => Type::Null,
            Value::Boolean(_) => Type::Boolean,
            Value::Integer(_) => Type::Integer,
            Value::Float(_) => Type::Float,
            Value::Text(_) => Type::Text,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, ""),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Integer(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::Text(s) => write!(f, "{}", s),
        }
    }
}

impl std::cmp::Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Value::Null => {}
            Value::Boolean(b) => b.hash(state),
            Value::Integer(i) => i.hash(state),
            Value::Float(f) => f.to_bits().hash(state), // Hash float by its bit representation
            Value::Text(s) => s.hash(state),
        }
    }
}

/// A type tag attached to a [`Value`] or declared on an [`InputSpec`] /
/// [`OutputSpec`].
///
/// The scalar variants mirror the [`Value`] variants. The `Array`, `Map`,
/// and `Any` variants are used by metadata specs and parameter
/// resolution to describe compound shapes — a store entry is tagged with
/// one of the scalar variants, but a declared input can accept a whole
/// array or map. `Any` opts out of type checking for that input. The
/// enum is marked `#[non_exhaustive]` so future type additions do not
/// break external match statements.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Type {
    /// Matches [`Value::Null`].
    Null,
    /// Matches [`Value::Boolean`].
    Boolean,
    /// Matches [`Value::Integer`].
    Integer,
    /// Matches [`Value::Float`].
    Float,
    /// Matches [`Value::Text`].
    Text,
    /// Matches [`StoreEntry::Array`] — used in spec declarations.
    Array,
    /// Matches [`StoreEntry::Map`] — used in spec declarations.
    Map,
    /// Matches any entry — used in spec declarations to opt out of type
    /// checking.
    Any,
}

impl Type {
    /// Returns the human-readable type name used in error messages and
    /// diagnostics.
    pub fn name(&self) -> &'static str {
        match self {
            Type::Null => "Null",
            Type::Boolean => "Boolean",
            Type::Integer => "Integer",
            Type::Float => "Float",
            Type::Text => "Text",
            Type::Array => "Array",
            Type::Map => "Map",
            Type::Any => "Any",
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
