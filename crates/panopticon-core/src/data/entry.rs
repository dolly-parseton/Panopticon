use crate::imports::*;

/// The generic entry type held by the runtime [`Store`].
///
/// A `StoreEntry` is one of three shapes: a scalar [`Var`](Self::Var)
/// pairing a [`Value`] with its [`Type`], an [`Array`](Self::Array) of
/// nested entries, or a [`Map`](Self::Map) of named nested entries. This
/// union is what lets the store hold variables, collection handles, step
/// outputs, and resolved parameters uniformly.
///
/// The `get_*` methods (taking a key or index) traverse collection
/// entries and fail with an [`AccessError`] when the entry has the wrong
/// shape. The `as_*` methods narrow an entry to a specific variant for
/// callers that know what they are holding.
#[derive(Debug, Clone, PartialEq)]
pub enum StoreEntry {
    /// A scalar value paired with its type tag.
    Var {
        /// The stored value.
        value: Value,
        /// The type tag derived from `value` at insertion time.
        ty: Type,
    },
    /// An ordered array of nested entries.
    Array(Vec<StoreEntry>),
    /// A map of named nested entries.
    Map(HashMap<String, StoreEntry>),
}

impl StoreEntry {
    /// Looks up a child entry by key. Fails with
    /// [`AccessError::NotAMap`] if this entry is not a [`Map`](Self::Map),
    /// or [`AccessError::NotFound`] if the key is missing.
    pub fn get_key(&self, key: &str) -> Result<&StoreEntry, AccessError> {
        match self {
            StoreEntry::Var { .. } => Err(AccessError::NotAMap("Var")),
            StoreEntry::Array(_) => Err(AccessError::NotAMap("Array")),
            StoreEntry::Map(entries) => entries
                .get(key)
                .ok_or_else(|| AccessError::NotFound(key.into())),
        }
    }
    /// Looks up a child entry by index. Fails with
    /// [`AccessError::NotAnArray`] if this entry is not an
    /// [`Array`](Self::Array), or [`AccessError::IndexOutOfBounds`] if
    /// the index is out of range.
    pub fn get_index(&self, index: usize) -> Result<&StoreEntry, AccessError> {
        match self {
            StoreEntry::Var { .. } => Err(AccessError::NotAnArray("Var")),
            StoreEntry::Array(items) => {
                items.get(index).ok_or(AccessError::IndexOutOfBounds(index))
            }
            StoreEntry::Map(_) => Err(AccessError::NotAnArray("Map")),
        }
    }
    /// Returns the inner [`Value`] of a [`Var`](Self::Var) entry. Fails
    /// with [`AccessError::NotAVar`] for arrays or maps.
    pub fn get_value(&self) -> Result<&Value, AccessError> {
        match self {
            StoreEntry::Var { value, .. } => Ok(value),
            StoreEntry::Array(_) => Err(AccessError::NotAVar("Array")),
            StoreEntry::Map(_) => Err(AccessError::NotAVar("Map")),
        }
    }
    /// Narrows this entry to a [`Var`](Self::Var), returning its value
    /// and type. Fails with [`AccessError::NotAVar`] for arrays or maps.
    pub fn as_var(&self) -> Result<(&Value, &Type), AccessError> {
        match self {
            StoreEntry::Var { value, ty } => Ok((value, ty)),
            StoreEntry::Array(_) => Err(AccessError::NotAVar("Array")),
            StoreEntry::Map(_) => Err(AccessError::NotAVar("Map")),
        }
    }
    /// Narrows this entry to an [`Array`](Self::Array), returning the
    /// underlying vector. Fails with [`AccessError::NotAnArray`] for
    /// vars or maps.
    pub fn as_array(&self) -> Result<&Vec<StoreEntry>, AccessError> {
        match self {
            StoreEntry::Var { .. } => Err(AccessError::NotAnArray("Var")),
            StoreEntry::Array(items) => Ok(items),
            StoreEntry::Map(_) => Err(AccessError::NotAnArray("Map")),
        }
    }
    /// Narrows this entry to a [`Map`](Self::Map), returning the
    /// underlying hash map. Fails with [`AccessError::NotAMap`] for vars
    /// or arrays.
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
