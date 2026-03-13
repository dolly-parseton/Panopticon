use crate::imports::*;

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    // Simple types
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Text(String),
}

impl Value {
    pub fn as_text(&self) -> Result<&str, AccessError> {
        match self {
            Value::Text(s) => Ok(s),
            _ => Err(AccessError::TypeMismatch {
                expected: "Text",
                found: self.get_type().name(),
            }),
        }
    }
    pub fn as_integer(&self) -> Result<i64, AccessError> {
        match self {
            Value::Integer(i) => Ok(*i),
            _ => Err(AccessError::TypeMismatch {
                expected: "Integer",
                found: self.get_type().name(),
            }),
        }
    }
    pub fn as_float(&self) -> Result<f64, AccessError> {
        match self {
            Value::Float(f) => Ok(*f),
            _ => Err(AccessError::TypeMismatch {
                expected: "Float",
                found: self.get_type().name(),
            }),
        }
    }
    pub fn as_boolean(&self) -> Result<bool, AccessError> {
        match self {
            Value::Boolean(b) => Ok(*b),
            _ => Err(AccessError::TypeMismatch {
                expected: "Boolean",
                found: self.get_type().name(),
            }),
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Type {
    Null,
    Boolean,
    Integer,
    Float,
    Text,
    // Non 'Value' types used by spec and param resolution.
    Array,
    Map,
    Any,
}

impl Type {
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
