use crate::imports::*;

#[allow(unused)]
mod sealed {
    #[doc(hidden)]
    pub struct New(());
    #[doc(hidden)]
    pub struct InProgress(());
    #[doc(hidden)]
    pub struct Failed(());
    #[doc(hidden)]
    pub struct Completed(());
}

/*
    Types: TODO, extend result store as a product of ExecutionContext after pipeline execution finishes. It'll need several 'save to file' and display methods.
    * ExecutionContext - Holds the scalar and tabular stores for command execution context
*/

#[derive(Clone, Debug, Default)]
pub struct ExecutionContext {
    services: PipelineServices,
    extensions: Extensions,
    scalar_store: ScalarStore,
    tabular_store: TabularStore,
}

impl ExecutionContext {
    pub fn new(services: PipelineServices) -> Self {
        ExecutionContext {
            services,
            extensions: Extensions::new(),
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

    pub async fn substitute<T: Into<String>>(&self, template: T) -> Result<String> {
        self.scalar_store.render_template(template.into()).await
    }

    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    pub fn services(&self) -> &PipelineServices {
        &self.services
    }
}
