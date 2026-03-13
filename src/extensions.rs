use crate::imports::*;
use std::any::Any;

/// Marker trait for types that can be registered as pipeline extensions.
///
/// Extensions are shared, read-only services injected at pipeline configuration
/// time (Draft phase) and available to operations via `Context`.
///
/// For stateful extensions that need interior mutability, wrap them in `Arc`
/// and use internal synchronisation (`Mutex`, `RwLock`, etc.):
///
/// ```ignore
/// struct TokenCache {
///     tokens: std::sync::RwLock<HashMap<String, String>>,
/// }
///
/// impl Extension for Arc<TokenCache> {}
/// ```
pub trait Extension: Clone + Send + Sync + 'static {}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct ExtensionKey {
    name: String,
    type_id: TypeId,
}

trait CloneSyncSendAny: Any + Send + Sync {
    fn clone_box(&self) -> Box<dyn CloneSyncSendAny>;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Clone + Send + Sync + 'static> CloneSyncSendAny for T {
    fn clone_box(&self) -> Box<dyn CloneSyncSendAny> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clone for Box<dyn CloneSyncSendAny> {
    fn clone(&self) -> Self {
        (**self).clone_box()
    }
}

#[derive(Clone, Default)]
pub struct Extensions {
    entries: HashMap<ExtensionKey, Box<dyn CloneSyncSendAny>>,
}

impl std::fmt::Debug for Extensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let keys: Vec<_> = self.entries.keys().collect();
        f.debug_struct("Extensions")
            .field("entries", &keys)
            .finish()
    }
}

impl Extensions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: Extension>(&mut self, name: impl Into<String>, value: T) {
        let name = name.into();
        let key = ExtensionKey {
            name,
            type_id: TypeId::of::<T>(),
        };
        self.entries.insert(key, Box::new(value));
    }

    pub fn get<T: Extension>(&self, name: impl Into<String>) -> Option<&T> {
        let name = name.into();
        let key = ExtensionKey {
            name,
            type_id: TypeId::of::<T>(),
        };
        (**self.entries.get(&key)?).as_any().downcast_ref()
    }

    pub fn contains<T: Extension>(&self, name: impl Into<String>) -> bool {
        let name = name.into();
        let key = ExtensionKey {
            name,
            type_id: TypeId::of::<T>(),
        };
        self.entries.contains_key(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::RwLock;

    // Simple value extension
    #[derive(Clone)]
    struct AppConfig {
        debug: bool,
    }

    impl Extension for AppConfig {}

    // Shared mutable extension via Arc
    struct TokenStore {
        tokens: RwLock<HashMap<String, String>>,
    }

    impl Extension for Arc<TokenStore> {}

    #[test]
    fn insert_and_retrieve() {
        let mut ext = Extensions::new();
        ext.insert("app_config", AppConfig { debug: true });
        let cfg = ext.get::<AppConfig>("app_config").unwrap();
        assert!(cfg.debug);
    }

    #[test]
    fn missing_returns_none() {
        let ext = Extensions::new();
        assert!(ext.get::<AppConfig>("app_config").is_none());
    }

    #[test]
    fn contains_check() {
        let mut ext = Extensions::new();
        ext.insert("app_config", AppConfig { debug: false });
        assert!(ext.contains::<AppConfig>("app_config"));
        assert!(!ext.contains::<Arc<TokenStore>>("token_store"));
    }

    #[test]
    fn same_type_different_names() {
        let mut ext = Extensions::new();
        ext.insert(
            "primary",
            Arc::new(TokenStore {
                tokens: RwLock::new(HashMap::from([("a".into(), "1".into())])),
            }),
        );
        ext.insert(
            "secondary",
            Arc::new(TokenStore {
                tokens: RwLock::new(HashMap::from([("b".into(), "2".into())])),
            }),
        );

        let primary = ext.get::<Arc<TokenStore>>("primary").unwrap();
        let secondary = ext.get::<Arc<TokenStore>>("secondary").unwrap();

        assert_eq!(
            primary.tokens.read().unwrap().get("a"),
            Some(&"1".to_string())
        );
        assert_eq!(
            secondary.tokens.read().unwrap().get("b"),
            Some(&"2".to_string())
        );
        assert!(primary.tokens.read().unwrap().get("b").is_none());
    }

    #[test]
    fn arc_extension_shares_state_across_clones() {
        let store = Arc::new(TokenStore {
            tokens: RwLock::new(HashMap::new()),
        });

        let mut ext = Extensions::new();
        ext.insert("token_store", Arc::clone(&store));

        let cloned = ext.clone();

        // Mutate via the clone
        {
            let s = cloned.get::<Arc<TokenStore>>("token_store").unwrap();
            s.tokens
                .write()
                .unwrap()
                .insert("key".into(), "value".into());
        }

        // Original sees the mutation (same Arc)
        let s = ext.get::<Arc<TokenStore>>("token_store").unwrap();
        assert_eq!(
            s.tokens.read().unwrap().get("key"),
            Some(&"value".to_string()),
        );
    }

    #[test]
    fn clone_value_extension_is_independent() {
        let mut ext = Extensions::new();
        ext.insert("cfg", AppConfig { debug: true });

        let mut cloned = ext.clone();
        cloned.insert("cfg", AppConfig { debug: false });

        assert!(ext.get::<AppConfig>("cfg").unwrap().debug);
        assert!(!cloned.get::<AppConfig>("cfg").unwrap().debug);
    }
}
