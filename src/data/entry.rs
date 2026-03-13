use crate::imports::*;

#[derive(Debug, Clone, PartialEq)]
pub enum StoreEntry {
    Var { value: Value, ty: Type },
    Array(Vec<StoreEntry>),
    Map(HashMap<String, StoreEntry>),
}

impl StoreEntry {
    // Unambiguous accessors
    pub fn get_key(&self, key: &str) -> Result<&StoreEntry, AccessError> {
        match self {
            StoreEntry::Var { .. } => Err(AccessError::NotAMap("Var")),
            StoreEntry::Array(_) => Err(AccessError::NotAMap("Array")),
            StoreEntry::Map(entries) => entries
                .get(key)
                .ok_or_else(|| AccessError::NotFound(key.into())),
        }
    }
    pub fn get_index(&self, index: usize) -> Result<&StoreEntry, AccessError> {
        match self {
            StoreEntry::Var { .. } => Err(AccessError::NotAnArray("Var")),
            StoreEntry::Array(items) => {
                items.get(index).ok_or(AccessError::IndexOutOfBounds(index))
            }
            StoreEntry::Map(_) => Err(AccessError::NotAnArray("Map")),
        }
    }
    pub fn get_value(&self) -> Result<&Value, AccessError> {
        match self {
            StoreEntry::Var { value, .. } => Ok(value),
            StoreEntry::Array(_) => Err(AccessError::NotAVar("Array")),
            StoreEntry::Map(_) => Err(AccessError::NotAVar("Map")),
        }
    }
    // as_* accessors for when you don't know the type ahead of time.
    pub fn as_var(&self) -> Result<(&Value, &Type), AccessError> {
        match self {
            StoreEntry::Var { value, ty } => Ok((value, ty)),
            StoreEntry::Array(_) => Err(AccessError::NotAVar("Array")),
            StoreEntry::Map(_) => Err(AccessError::NotAVar("Map")),
        }
    }
    pub fn as_array(&self) -> Result<&Vec<StoreEntry>, AccessError> {
        match self {
            StoreEntry::Var { .. } => Err(AccessError::NotAnArray("Var")),
            StoreEntry::Array(items) => Ok(items),
            StoreEntry::Map(_) => Err(AccessError::NotAnArray("Map")),
        }
    }
    pub fn as_map(&self) -> Result<&HashMap<String, StoreEntry>, AccessError> {
        match self {
            StoreEntry::Var { .. } => Err(AccessError::NotAMap("Var")),
            StoreEntry::Array(_) => Err(AccessError::NotAMap("Array")),
            StoreEntry::Map(entries) => Ok(entries),
        }
    }
}

impl std::cmp::Eq for StoreEntry {}

impl std::hash::Hash for StoreEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            StoreEntry::Var { value, ty } => {
                value.hash(state);
                ty.hash(state);
            }
            StoreEntry::Array(items) => items.hash(state),
            StoreEntry::Map(map) => {
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort();
                for key in keys {
                    key.hash(state);
                    map[key].hash(state);
                }
            }
        }
    }
}

impl<T: Into<Value>> From<T> for StoreEntry {
    fn from(v: T) -> Self {
        let value = v.into();
        StoreEntry::Var {
            ty: value.get_type(),
            value,
        }
    }
}

impl From<&Value> for StoreEntry {
    fn from(v: &Value) -> Self {
        let value = v.clone();
        StoreEntry::Var {
            ty: value.get_type(),
            value,
        }
    }
}

impl From<Vec<StoreEntry>> for StoreEntry {
    fn from(items: Vec<StoreEntry>) -> Self {
        StoreEntry::Array(items)
    }
}
