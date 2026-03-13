use crate::imports::*;

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
    pub fn new() -> Self {
        Store::<T> {
            entries: HashMap::new(),
        }
    }

    pub fn get<N: AsRef<str>>(&self, name: N) -> Result<&T, StoreError> {
        let name = name.as_ref();
        self.entries
            .get(name)
            .ok_or_else(|| StoreError::EntryNotFound(name.into()))
    }

    pub fn insert<N: Into<String>>(&mut self, name: N, entry: T) -> Result<(), StoreError> {
        let name = name.into();
        if self.entries.contains_key(&name) {
            return Err(StoreError::EntryAlreadyExists(name));
        }
        self.entries.insert(name, entry);
        Ok(())
    }

    pub fn insert_or_replace<N: Into<String>>(&mut self, name: N, entry: T) {
        self.entries.insert(name.into(), entry);
    }

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

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &T)> {
        self.entries.iter()
    }
}

impl Store<StoreEntry> {
    // Simple methods for defining entries in a store.
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

    // More complex closure methods
    pub fn with_array<N: Into<String>, F>(&mut self, name: N, body: F) -> Result<(), StoreError>
    where
        F: FnOnce(&mut ArrayHandle<StoreEntry>) -> Result<(), StoreError>,
    {
        let name = name.into();
        let mut arr_handle = self.define_array(name)?;
        body(&mut arr_handle)
    }
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
