use crate::imports::*;

/// A keyed collection of entries shared across the pipeline lifecycle.
///
/// The store is the single source of truth for pipeline state. It holds
/// draft-time variables, resolved step parameters, per-step global
/// outputs, and the values produced by return blocks. The generic
/// parameter `T` is the entry type — most stores hold [`StoreEntry`]
/// values, but `Store<Parameters>` is used in draft/ready phases to hold
/// the unresolved parameter bindings for each step.
///
/// Keys are flat dotted strings; nesting is modelled by the
/// [`StoreEntry::Array`] and [`StoreEntry::Map`] variants, not by the
/// store itself. Insertion is append-only through [`insert`](Self::insert)
/// (errors on duplicate) or replacement-allowed through
/// [`insert_or_replace`](Self::insert_or_replace).
#[derive(Debug, Clone)]
pub struct Store<T = StoreEntry> {
    entries: HashMap<String, T>,
}

impl Default for Store<StoreEntry> {
    fn default() -> Self {
        Store::new()
    }
}

impl<T> Store<T> {
    /// Constructs an empty store.
    pub fn new() -> Self {
        Store::<T> {
            entries: HashMap::new(),
        }
    }

    /// Looks up an entry by name. Returns [`StoreError::EntryNotFound`]
    /// if the name is not in the store.
    pub fn get<N: AsRef<str>>(&self, name: N) -> Result<&T, StoreError> {
        let name = name.as_ref();
        self.entries
            .get(name)
            .ok_or_else(|| StoreError::EntryNotFound(name.into()))
    }

    /// Inserts an entry under `name`. Returns
    /// [`StoreError::EntryAlreadyExists`] if the name is already in use.
    pub fn insert<N: Into<String>>(&mut self, name: N, entry: T) -> Result<(), StoreError> {
        let name = name.into();
        if self.entries.contains_key(&name) {
            return Err(StoreError::EntryAlreadyExists(name));
        }
        self.entries.insert(name, entry);
        Ok(())
    }

    /// Inserts an entry under `name`, overwriting any existing entry
    /// with the same name. Used by the runtime when an operation rewrites
    /// its own outputs.
    pub fn insert_or_replace<N: Into<String>>(&mut self, name: N, entry: T) {
        self.entries.insert(name.into(), entry);
    }

    /// Merges `other` into this store, optionally prefixing each
    /// incoming key with `"{prefix}."`. Fails on the first key collision
    /// with [`StoreError::EntryAlreadyExists`].
    pub fn merge(&mut self, prefix_opt: Option<&str>, other: Store<T>) -> Result<(), StoreError> {
        let key_prefix = match prefix_opt {
            Some(prefix) => format!("{}.", prefix),
            None => String::new(),
        };
        for (key, value) in other.entries {
            let key = format!("{}{}", key_prefix, key);
            if self.entries.contains_key(&key) {
                return Err(StoreError::EntryAlreadyExists(key));
            }
            self.entries.insert(key, value);
        }
        Ok(())
    }

    /// Iterates over the names of every entry in the store.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    /// Iterates over `(name, entry)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &T)> {
        self.entries.iter()
    }
}

impl Store<StoreEntry> {
    /// Defines a new variable entry — a [`StoreEntry::Var`] wrapping
    /// the given [`Value`] and its derived [`Type`]. Fails with
    /// [`StoreError::EntryAlreadyExists`] if the name is already in use.
    pub fn define_var<N: Into<String>, V: Into<Value>>(
        &mut self,
        name: N,
        value: V,
    ) -> Result<(), StoreError> {
        let name = name.into();
        if self.entries.contains_key(&name) {
            return Err(StoreError::EntryAlreadyExists(name));
        }
        let value = value.into();
        let ty = value.get_type();
        self.entries.insert(name, StoreEntry::Var { value, ty });
        Ok(())
    }

    /// Defines a new empty array entry and returns an [`ArrayHandle`]
    /// for populating it.
    pub fn define_array<N: Into<String>>(
        &mut self,
        name: N,
    ) -> Result<ArrayHandle<'_, StoreEntry>, StoreError> {
        let name = name.into();
        if self.entries.contains_key(&name) {
            return Err(StoreError::EntryAlreadyExists(name));
        }
        self.entries
            .insert(name.clone(), StoreEntry::Array(Vec::new()));
        let data = match self.entries.get_mut(&name) {
            Some(StoreEntry::Array(arr)) => arr,
            _ => unreachable!(),
        };
        Ok(ArrayHandle::new(name, data))
    }

    /// Defines a new empty map entry and returns a [`MapHandle`] for
    /// populating it.
    pub fn define_map<N: Into<String>>(
        &mut self,
        name: N,
    ) -> Result<MapHandle<'_, StoreEntry>, StoreError> {
        let name = name.into();
        if self.entries.contains_key(&name) {
            return Err(StoreError::EntryAlreadyExists(name));
        }
        self.entries
            .insert(name.clone(), StoreEntry::Map(HashMap::new()));
        let data = match self.entries.get_mut(&name) {
            Some(StoreEntry::Map(map)) => map,
            _ => unreachable!(),
        };
        Ok(MapHandle::new(name, data))
    }

    /// Defines a new array entry and passes a handle to a closure for
    /// population. Convenience wrapper over [`define_array`](Self::define_array)
    /// when the nested structure would otherwise force awkward
    /// temporaries.
    pub fn with_array<N: Into<String>, F>(&mut self, name: N, body: F) -> Result<(), StoreError>
    where
        F: FnOnce(&mut ArrayHandle<StoreEntry>) -> Result<(), StoreError>,
    {
        let name = name.into();
        let mut arr_handle = self.define_array(name)?;
        body(&mut arr_handle)
    }
    /// Defines a new map entry and passes a handle to a closure for
    /// population.
    pub fn with_map<N: Into<String>, F>(&mut self, name: N, body: F) -> Result<(), StoreError>
    where
        F: FnOnce(&mut MapHandle<StoreEntry>) -> Result<(), StoreError>,
    {
        let name = name.into();
        let mut map_handle = self.define_map(name)?;
        body(&mut map_handle)
    }
}

impl<T> IntoIterator for Store<T> {
    type Item = (String, T);
    type IntoIter = std::collections::hash_map::IntoIter<String, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
