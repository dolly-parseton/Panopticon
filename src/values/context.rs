use crate::imports::*;

#[derive(Clone, Debug, Default)]
pub struct ExecutionContext {
    scalar_store: ScalarStore,
    tabular_store: TabularStore,
}

impl ExecutionContext {
    pub fn new() -> Self {
        ExecutionContext {
            scalar_store: ScalarStore::new(),
            tabular_store: TabularStore::new(),
        }
    }

    pub fn scalar(&self) -> &ScalarStore {
        &self.scalar_store
    }

    pub fn tabular(&self) -> &TabularStore {
        &self.tabular_store
    }

    // Changed API to async not impl Future return.
    pub async fn substitute<T: Into<String>>(&self, template: T) -> Result<String> {
        self.scalar_store.render_template(template.into()).await
    }
}
