use crate::imports::*;

// TODO Complete Error type
impl Pipeline<Complete> {
    // State machine functions
    pub fn debug(&self) {
        println!("Complete State:");
        println!("Variables:");
        for (key, entry) in self.state.variables.iter() {
            println!("  {}: {:?}", key, entry);
        }
        println!("Returns:");
        for (key, entry) in self.state.returns.iter() {
            println!("  {}: {:?}", key, entry);
        }
    }

    pub fn variables(&self) -> &Store<StoreEntry> {
        &self.state.variables
    }

    pub fn returns(&self) -> &Store<StoreEntry> {
        &self.state.returns
    }
}

#[cfg(feature = "serde")]
impl Pipeline<Complete> {
    pub fn deserialize_returns<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
    ) -> Result<T, DeserializeError> {
        let prefix = format!("{}.", name);
        let entries =
            self.state.returns.iter().filter_map(|(key, entry)| {
                key.strip_prefix(&prefix).map(|stripped| (stripped, entry))
            });
        from_prefix_entries(entries)
    }
}
