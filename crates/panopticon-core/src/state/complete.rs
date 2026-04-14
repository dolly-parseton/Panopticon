use crate::imports::*;

impl Pipeline<Complete> {
    /// Prints the final variables and returns stores to stdout. Intended
    /// for ad-hoc inspection during development; not part of any stable
    /// output contract.
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

    /// Read-only access to the variables store as it stood when the worker
    /// thread exited — every variable defined at draft time plus every
    /// global output produced during execution.
    pub fn variables(&self) -> &Store<StoreEntry> {
        &self.state.variables
    }

    /// Read-only access to the resolved returns store — the entries produced
    /// by each declared return block, keyed by `"{block_name}.{field_name}"`.
    pub fn returns(&self) -> &Store<StoreEntry> {
        &self.state.returns
    }
}

#[cfg(feature = "serde")]
impl Pipeline<Complete> {
    /// Deserialises the entries produced by a named return block into a
    /// concrete type `T`.
    ///
    /// Strips the `{name}.` prefix from each matching entry and hands the
    /// result to a `StoreEntry`-aware `serde` deserialiser. Requires the
    /// `serde` feature.
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
