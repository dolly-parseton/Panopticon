use crate::imports::*;

/// A draft-time builder for populating an array entry in the [`Store`].
///
/// Returned by [`Pipeline::array`] and [`Store::define_array`]. Methods
/// are chainable so nested structures can be built in a single
/// expression. Nested arrays and maps are added via
/// [`push_array`](ArrayHandle::push_array) and
/// [`push_map`](MapHandle::insert_map) and return child handles borrowing
/// from this one.
pub struct ArrayHandle<'a, T = StoreEntry> {
    name: String,
    data: &'a mut Vec<T>,
}

/// A draft-time builder for populating a map entry in the [`Store`].
///
/// Returned by [`Pipeline::map`] and [`Store::define_map`]. Methods are
/// chainable so nested structures can be built in a single expression.
/// Nested arrays and maps are added via [`insert_array`](MapHandle::insert_array)
/// and [`insert_map`](MapHandle::insert_map) and return child handles
/// borrowing from this one.
pub struct MapHandle<'a, T = StoreEntry> {
    name: String,
    data: &'a mut HashMap<String, T>,
}

impl<'a, T> ArrayHandle<'a, T> {
    /// Wraps a mutable vector in a handle under the given display name.
    /// Intended for extension code; user code obtains handles through
    /// [`Pipeline::array`] or [`Store::define_array`].
    pub fn new<N: Into<String>>(name: N, data: &'a mut Vec<T>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }

    /// Appends a value to the array. Returns a mutable reference for
    /// chaining.
    pub fn push<V: Into<T>>(&mut self, value: V) -> Result<&mut Self, StoreError> {
        self.data.push(value.into());
        Ok(self)
    }
}

impl<'a, T> MapHandle<'a, T> {
    /// Wraps a mutable map in a handle under the given display name.
    /// Intended for extension code; user code obtains handles through
    /// [`Pipeline::map`] or [`Store::define_map`].
    pub fn new<N: Into<String>>(name: N, data: &'a mut HashMap<String, T>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }

    /// Inserts a key/value pair into the map. Returns a mutable reference
    /// for chaining. An existing entry under the same key is overwritten.
    pub fn insert<V: Into<T>, K: Into<String>>(
        &mut self,
        key: K,
        value: V,
    ) -> Result<&mut Self, StoreError> {
        self.data.insert(key.into(), value.into());
        Ok(self)
    }
}

mod store_entry {
    use crate::imports::*;
    // Explicit support for StoreEntry — the default element type these
    // handles work with — where nested arrays and maps are themselves
    // StoreEntry values.
    impl<'a> ArrayHandle<'a, StoreEntry> {
        /// Appends a new nested array entry and returns a child handle
        /// borrowing from this one to populate it.
        pub fn push_array(&mut self) -> Result<ArrayHandle<'_, StoreEntry>, StoreError> {
            self.data.push(StoreEntry::Array(Vec::new()));
            let idx = self.data.len() - 1;
            let inner = match &mut self.data[idx] {
                StoreEntry::Array(arr) => arr,
                _ => unreachable!(),
            };
            let name = format!("{}[{}]", self.name, idx);
            Ok(ArrayHandle::new(name, inner))
        }

        /// Appends a new nested map entry and returns a child handle
        /// borrowing from this one to populate it.
        pub fn push_map(&mut self) -> Result<MapHandle<'_, StoreEntry>, StoreError> {
            self.data.push(StoreEntry::Map(HashMap::new()));
            let idx = self.data.len() - 1;
            let inner = match &mut self.data[idx] {
                StoreEntry::Map(map) => map,
                _ => unreachable!(),
            };
            let name = format!("{}[{}]", self.name, idx);
            Ok(MapHandle::new(name, inner))
        }
    }

    impl<'a> MapHandle<'a, StoreEntry> {
        /// Inserts a new nested array entry under `key` and returns a
        /// child handle borrowing from this one to populate it.
        pub fn insert_array<K: Into<String>>(
            &mut self,
            key: K,
        ) -> Result<ArrayHandle<'_, StoreEntry>, StoreError> {
            let key = key.into();
            self.data.insert(key.clone(), StoreEntry::Array(Vec::new()));
            let inner = match self.data.get_mut(&key) {
                Some(StoreEntry::Array(arr)) => arr,
                _ => unreachable!(),
            };
            let name = format!("{}.{}", self.name, key);
            Ok(ArrayHandle::new(name, inner))
        }

        /// Inserts a new nested map entry under `key` and returns a child
        /// handle borrowing from this one to populate it.
        pub fn insert_map<K: Into<String>>(
            &mut self,
            key: K,
        ) -> Result<MapHandle<'_, StoreEntry>, StoreError> {
            let key = key.into();
            self.data
                .insert(key.clone(), StoreEntry::Map(HashMap::new()));
            let inner = match self.data.get_mut(&key) {
                Some(StoreEntry::Map(map)) => map,
                _ => unreachable!(),
            };
            let name = format!("{}.{}", self.name, key);
            Ok(MapHandle::new(name, inner))
        }

        /// Inserts a nested array under `key` and hands it to a closure
        /// for population. Useful when the nested structure would
        /// otherwise force awkward temporaries.
        pub fn with_array<
            K: Into<String>,
            F: FnOnce(&mut ArrayHandle<'_, StoreEntry>) -> Result<(), StoreError>,
        >(
            &mut self,
            key: K,
            body: F,
        ) -> Result<(), StoreError> {
            let key = key.into();
            let mut arr_handle = self.insert_array(key)?;
            body(&mut arr_handle)
        }
        /// Inserts a nested map under `key` and hands it to a closure for
        /// population.
        pub fn with_map<
            K: Into<String>,
            F: FnOnce(&mut MapHandle<'_, StoreEntry>) -> Result<(), StoreError>,
        >(
            &mut self,
            key: K,
            body: F,
        ) -> Result<(), StoreError> {
            let key = key.into();
            let mut map_handle = self.insert_map(key)?;
            body(&mut map_handle)
        }
    }
}

mod homogeneous {
    use crate::imports::*;
    // Extension-facing helpers for homogeneous collections (`Vec<T>`,
    // `HashMap<String, T>`) where the element type isn't `StoreEntry`.
    // Core pipeline code works in terms of `StoreEntry`, but extension
    // crates sometimes want the same handle patterns for simpler data.
    impl<'a, T> ArrayHandle<'a, Vec<T>> {
        /// Appends a new inner `Vec<T>` and returns a child array handle
        /// for populating it.
        pub fn push_array(&mut self) -> Result<ArrayHandle<'_, T>, StoreError> {
            self.data.push(Vec::new());
            let idx = self.data.len() - 1;
            let inner = &mut self.data[idx];
            let name = format!("{}[{}]", self.name, idx);
            Ok(ArrayHandle::new(name, inner))
        }
    }

    impl<'a, T> ArrayHandle<'a, HashMap<String, T>> {
        /// Appends a new inner `HashMap<String, T>` and returns a child
        /// map handle for populating it.
        pub fn push_map(&mut self) -> Result<MapHandle<'_, T>, StoreError> {
            self.data.push(std::collections::HashMap::new());
            let idx = self.data.len() - 1;
            let inner = &mut self.data[idx];
            let name = format!("{}[{}]", self.name, idx);
            Ok(MapHandle::new(name, inner))
        }
    }
    impl<'a, T> MapHandle<'a, Vec<T>> {
        /// Inserts a new inner `Vec<T>` under `key` and returns a child
        /// array handle for populating it.
        pub fn insert_array<K: Into<String>>(
            &mut self,
            key: K,
        ) -> Result<ArrayHandle<'_, T>, StoreError> {
            let key = key.into();
            self.data.insert(key.clone(), Vec::new());
            let inner = self.data.get_mut(&key).unwrap();
            let name = format!("{}.{}", self.name, key);
            Ok(ArrayHandle::new(name, inner))
        }
    }

    impl<'a, T> MapHandle<'a, HashMap<String, T>> {
        /// Inserts a new inner `HashMap<String, T>` under `key` and
        /// returns a child map handle for populating it.
        pub fn insert_map<K: Into<String>>(
            &mut self,
            key: K,
        ) -> Result<MapHandle<'_, T>, StoreError> {
            let key = key.into();
            self.data
                .insert(key.clone(), std::collections::HashMap::new());
            let inner = self.data.get_mut(&key).unwrap();
            let name = format!("{}.{}", self.name, key);
            Ok(MapHandle::new(name, inner))
        }
    }
}
