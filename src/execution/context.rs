use crate::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ExecutionContext {
    scalar_store: ScalarStore,
    tabular_store: TabularStore,
}

impl ExecutionContext {
    pub fn new(input_scalars: Option<&HashMap<String, ScalarValue>>) -> Self {
        ExecutionContext {
            scalar_store: match input_scalars {
                Some(inputs) => ScalarStore::with_inputs(inputs),
                None => ScalarStore::default(),
            },
            tabular_store: TabularStore::new(),
        }
    }

    pub fn scalar(&self) -> &ScalarStore {
        &self.scalar_store
    }

    pub fn tabular(&self) -> &TabularStore {
        &self.tabular_store
    }

    pub fn substitute<T: Into<String>>(
        &self,
        template: T,
    ) -> impl std::future::Future<Output = Result<String>> + '_ {
        substitute(template, &self.scalar_store)
    }
}

pub struct ScalarStore {
    tera: Arc<RwLock<tera::Tera>>,
    store: Arc<RwLock<tera::Context>>,
}

impl Default for ScalarStore {
    fn default() -> Self {
        ScalarStore {
            tera: Arc::new(RwLock::new(tera::Tera::default())),
            store: Arc::new(RwLock::new(tera::Context::new())),
        }
    }
}

impl ScalarStore {
    pub fn with_inputs(inputs: &HashMap<String, ScalarValue>) -> Self {
        let mut ctx = tera::Context::new();
        for (key, value) in inputs.iter() {
            ctx.insert(key, value);
        }
        Self {
            tera: Arc::new(RwLock::new(tera::Tera::default())),
            store: Arc::new(RwLock::new(ctx)),
        }
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn insert<'a>(&'a self, key: &'a StorePath, value: ScalarValue) -> Result<()> {
        let store_key = key.namespace().context("StorePath has no namespace")?;
        let mut ctx = self.store.write().await;
        let mut target_value = match ctx.remove(store_key) {
            Some(v) => v,
            None => scalar_value_from(HashMap::<String, ScalarValue>::new())?,
        };
        insert_at_path(&mut target_value, key, value)?;
        ctx.insert(store_key, &target_value);
        Ok(())
    }
    pub async fn get<'a>(&'a self, key: &'a StorePath) -> Result<Option<ScalarValue>> {
        let store_key = key
            .namespace()
            .context("StorePath has no namespace")
            .map(|k| k.to_string());
        let ctx = self.store.read().await;
        let store_key = store_key?;
        if let Some(root_value) = ctx.get(&store_key) {
            if let Some(value) = get_at_path(root_value, key) {
                Ok(Some(value.clone()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    pub fn remove<'a>(
        &'a self,
        key: &'a StorePath,
    ) -> impl std::future::Future<Output = Result<Option<ScalarValue>>> + 'a {
        let store_key = key.namespace().context("StorePath has no namespace");
        async move {
            let store_key = store_key?;
            let mut ctx = self.store.write().await;
            Ok(ctx.remove(store_key))
        }
    }
    // pub fn keys(&self) -> impl std::future::Future<Output = Vec<String>> + '_ {
    //     async move {
    //         let ctx = self.store.read().await;
    //         // tera::Context doesn't expose iter(), so we clone and convert to JSON
    //         let json = ctx.clone().into_json();
    //         match json {
    //             tera::Value::Object(map) => map.keys().cloned().collect(),
    //             _ => Vec::new(),
    //         }
    //     }
    // }
}

pub fn substitute<T: Into<String>>(
    template: T,
    context: &ScalarStore,
) -> impl std::future::Future<Output = Result<String>> + '_ {
    let template = template.into();
    async move {
        let mut tera = context.tera.write().await;
        let store = context.store.read().await;
        tera.render_str(&template, &store).map_err(|e| {
            // Extract the root cause from tera's error chain
            let mut cause = String::new();
            if let Some(source) = e.source() {
                cause = format!(" Caused by: {}", source);
            }
            anyhow::anyhow!(
                "Template rendering failed for '{}': {}{}",
                template,
                e,
                cause
            )
        })
    }
}

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

fn insert_at_path(root: &mut ScalarValue, path: &StorePath, value: ScalarValue) -> Result<()> {
    let segments = &path.segments();
    if segments.is_empty() {
        return Err(anyhow::anyhow!("StorePath has no segments"));
    }

    let mut current = root;
    // We skip the first segment (namespace key), so we iterate over segments[1..]
    // The last segment index after skip is (segments.len() - 1) - 1 = segments.len() - 2
    let last_index = segments.len().saturating_sub(2);

    for (i, segment) in segments.iter().skip(1).enumerate() {
        match current {
            ScalarValue::Object(map) => {
                if i == last_index {
                    map.insert(segment.clone(), value);
                    return Ok(());
                } else {
                    current = map.entry(segment.clone()).or_insert_with(|| {
                        scalar_value_from(HashMap::<String, ScalarValue>::new()).unwrap()
                    });
                }
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Cannot insert into non-object ScalarValue at segment '{}'",
                    segment
                ));
            }
        }
    }

    Ok(())
}

fn get_at_path<'a>(root: &'a ScalarValue, path: &StorePath) -> Option<&'a ScalarValue> {
    let segments = &path.segments();
    if segments.is_empty() {
        return None;
    }

    let mut current = root;

    // Skip the first segment since it's the namespace
    for segment in segments.iter().skip(1) {
        match current {
            ScalarValue::Object(map) => {
                if let Some(next) = map.get(segment) {
                    current = next;
                } else {
                    return None;
                }
            }
            _ => {
                return None;
            }
        }
    }

    Some(current)
}
