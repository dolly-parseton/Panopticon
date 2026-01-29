use crate::imports::*;

/*
    Types:
    * ScalarValue - A scalar value, re-export of Tera Value which is a re-export of serde_json::Value
    * ScalarType - Enum representing the type of a scalar value
    * ObjectBuilder - Builder pattern for constructing complex ScalarValue objects
    * ScalarStore - Store for managing ScalarValues, used in ExecutionContext.
*/
pub type ScalarValue = tera::Value;

#[derive(Debug, Clone, PartialEq, Default, Hash, Eq)]
pub enum ScalarType {
    #[default]
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

#[derive(Debug, Clone, Default)]
pub struct ObjectBuilder {
    map: tera::Map<String, ScalarValue>,
}

impl ObjectBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(mut self, key: impl Into<String>, value: impl Into<ScalarValue>) -> Self {
        self.map.insert(key.into(), value.into());
        self
    }

    pub fn object(mut self, key: impl Into<String>, nested: ObjectBuilder) -> Self {
        self.map.insert(key.into(), ScalarValue::Object(nested.map));
        self
    }

    pub fn build_scalar(self) -> ScalarValue {
        ScalarValue::Object(self.map)
    }

    pub fn build_hashmap(self) -> std::collections::HashMap<String, ScalarValue> {
        self.map.into_iter().collect()
    }
}

#[derive(Clone, Debug)]
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
            None => ObjectBuilder::new().build_scalar(),
        };
        super::helpers::insert_at_path(&mut target_value, key, value)?;
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
            if let Some(value) = super::helpers::get_at_path(root_value, key) {
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

    pub async fn render_with_tera(&self, tera: &tera::Tera, template_name: &str) -> Result<String> {
        let ctx = self.store.read().await;
        tera.render(template_name, &ctx).map_err(|e| {
            anyhow::anyhow!("Template rendering failed for '{}': {}", template_name, e)
        })
    }
    pub async fn render_template<T: Into<String>>(&self, template: T) -> Result<String> {
        let template_str = template.into();
        let mut tera = self.tera.write().await;
        tera.add_raw_template("inline_template", &template_str)?;
        let ctx = self.store.read().await;
        tera.render("inline_template", &ctx)
            .map_err(|e| anyhow::anyhow!("Template rendering failed for '{}': {}", template_str, e))
    }
}

/*
    Extension Traits:
    * ScalarMapExt - Extension trait for tera::Map<String, ScalarValue> and HashMap<String, ScalarValue> to provide helper methods for retrieving typed values
    * ScalarAsExt - Extension trait for ScalarValue to provide helper methods for converting to specific types with error handling (field is the name of the field being accessed for error messages)
*/
pub trait ScalarMapExt {
    fn get(&self, key: &str) -> Option<&ScalarValue>;

    fn get_required(&self, key: &str) -> Result<&ScalarValue> {
        self.get(key)
            .context(format!("missing required key '{}'", key))
    }

    fn get_required_string(&self, key: &str) -> Result<String> {
        self.get_required(key)?
            .as_str_or_err(key)
            .map(|s| s.to_string())
    }

    fn get_required_i64(&self, key: &str) -> Result<i64> {
        self.get_required(key)?.as_i64_or_err(key)
    }

    fn get_required_bool(&self, key: &str) -> Result<bool> {
        self.get_required(key)?.as_bool_or_err(key)
    }

    fn get_optional_string(&self, key: &str) -> Option<String> {
        self.get(key)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
    }

    fn get_optional_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    fn get_optional_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }
}

impl ScalarMapExt for tera::Map<String, ScalarValue> {
    fn get(&self, key: &str) -> Option<&ScalarValue> {
        tera::Map::get(self, key)
    }
}

impl ScalarMapExt for std::collections::HashMap<String, ScalarValue> {
    fn get(&self, key: &str) -> Option<&ScalarValue> {
        std::collections::HashMap::get(self, key)
    }
}

pub trait ScalarAsExt {
    fn as_str_or_err(&self, field: &str) -> Result<&str>;
    fn as_i64_or_err(&self, field: &str) -> Result<i64>;
    fn as_f64_or_err(&self, field: &str) -> Result<f64>;
    fn as_bool_or_err(&self, field: &str) -> Result<bool>;
    fn as_array_or_err(&self, field: &str) -> Result<&Vec<ScalarValue>>;
    fn as_object_or_err(&self, field: &str) -> Result<&tera::Map<String, ScalarValue>>;
}

impl ScalarAsExt for ScalarValue {
    fn as_str_or_err(&self, field: &str) -> Result<&str> {
        self.as_str()
            .context(format!("'{}' must be a string", field))
    }

    fn as_i64_or_err(&self, field: &str) -> Result<i64> {
        self.as_i64()
            .context(format!("'{}' must be an integer", field))
    }

    fn as_f64_or_err(&self, field: &str) -> Result<f64> {
        self.as_f64()
            .context(format!("'{}' must be a number", field))
    }

    fn as_bool_or_err(&self, field: &str) -> Result<bool> {
        self.as_bool()
            .context(format!("'{}' must be a boolean", field))
    }

    fn as_array_or_err(&self, field: &str) -> Result<&Vec<ScalarValue>> {
        self.as_array()
            .context(format!("'{}' must be an array", field))
    }

    fn as_object_or_err(&self, field: &str) -> Result<&tera::Map<String, ScalarValue>> {
        self.as_object()
            .context(format!("'{}' must be an object", field))
    }
}
