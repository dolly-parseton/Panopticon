use crate::imports::*;
/*
    Types:
    * TabularValue - A tabular data structure, re-export of Polars DataFrame
    * TabularStore - Store for managing TabularValues, used in ExecutionContext.
*/
pub type TabularValue = polars::prelude::DataFrame;

#[derive(Clone, Debug)]
pub struct TabularStore {
    store: Arc<RwLock<HashMap<String, TabularValue>>>,
}

impl Default for TabularStore {
    fn default() -> Self {
        TabularStore {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl TabularStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(
        &self,
        key: &StorePath,
        value: TabularValue,
    ) -> impl std::future::Future<Output = Result<()>> + '_ {
        let key = key.to_dotted();
        let value = value.clone();
        async move {
            self.store.write().await.insert(key, value);
            Ok(())
        }
    }
    pub fn get(
        &self,
        key: &StorePath,
    ) -> impl std::future::Future<Output = Result<Option<TabularValue>>> + '_ {
        let key = key.to_dotted();
        async move { Ok(self.store.read().await.get(&key).cloned()) }
    }
    pub fn remove(
        &self,
        key: &StorePath,
    ) -> impl std::future::Future<Output = Result<Option<TabularValue>>> + '_ {
        let key = key.to_dotted();
        async move {
            let removed = self.store.write().await.remove(&key);
            Ok(removed)
        }
    }
    pub async fn keys(&self) -> Vec<String> {
        self.store.read().await.keys().cloned().collect()
    }
}
