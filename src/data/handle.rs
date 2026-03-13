use crate::imports::*;
/*
    Handle types for working with arrays and maps in the store.
*/
pub struct ArrayHandle<'a, T = StoreEntry> {
    name: String,
    data: &'a mut Vec<T>,
}

pub struct MapHandle<'a, T = StoreEntry> {
    name: String,
    data: &'a mut HashMap<String, T>,
}

impl<'a, T> ArrayHandle<'a, T> {
    pub fn new<N: Into<String>>(name: N, data: &'a mut Vec<T>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }

    // Chainable push method for adding items to the array.
    pub fn push<V: Into<T>>(&mut self, value: V) -> Result<&mut Self, StoreError> {
        self.data.push(value.into());
        Ok(self)
    }
}

impl<'a, T> MapHandle<'a, T> {
    pub fn new<N: Into<String>>(name: N, data: &'a mut HashMap<String, T>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }

    // Chainable insert method for adding key-value pairs to the map.
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
    /*
        Explict support for StoreEntry which is the main type these handles work with.
    */
    impl<'a> ArrayHandle<'a, StoreEntry> {
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
        // Simple insert methods for adding nested arrays and maps.
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

        // Closure based methods for more complex nested structures.
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
    /*
        I think I'll want some support in extension crates for working with homogeneous collections of simple types, so these methods will be useful for that. They won't be used by the main store handling code since it needs to work with the more complex StoreEntry types, but they will be useful for extension crates that want to use the same handle patterns for working with simpler data.
    */
    impl<'a, T> ArrayHandle<'a, Vec<T>> {
        pub fn push_array(&mut self) -> Result<ArrayHandle<'_, T>, StoreError> {
            self.data.push(Vec::new());
            let idx = self.data.len() - 1;
            let inner = &mut self.data[idx];
            let name = format!("{}[{}]", self.name, idx);
            Ok(ArrayHandle::new(name, inner))
        }
    }

    impl<'a, T> ArrayHandle<'a, HashMap<String, T>> {
        pub fn push_map(&mut self) -> Result<MapHandle<'_, T>, StoreError> {
            self.data.push(std::collections::HashMap::new());
            let idx = self.data.len() - 1;
            let inner = &mut self.data[idx];
            let name = format!("{}[{}]", self.name, idx);
            Ok(MapHandle::new(name, inner))
        }
    }
    impl<'a, T> MapHandle<'a, Vec<T>> {
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
