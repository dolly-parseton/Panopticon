use crate::imports::*;

#[derive(Debug, Clone)]
pub enum Param {
    Literal(Value),              // Actual value
    Ref(String),                 // Reference to a 'var' in pipeline, includes step output.
    Template(Vec<Param>),        // Template strings containing references, broken down on read.
    Array(Vec<Param>), // Array of parameters, used for variadic parameters and for passing arrays as parameters.
    Map(HashMap<String, Param>), // Map of parameters, used for passing maps as
}

impl Param {
    // Constructors
    pub fn literal(value: impl Into<Value>) -> Self {
        Param::Literal(value.into())
    }
    pub fn reference(path: impl Into<String>) -> Self {
        Param::Ref(path.into())
    }
    pub fn template(parts: Vec<Param>) -> Self {
        Param::Template(parts)
    }
    pub fn array(items: Vec<Param>) -> Self {
        Param::Array(items)
    }
    pub fn map(entries: HashMap<String, Param>) -> Self {
        Param::Map(entries)
    }

    // Resolution to actual Values as StoreEntry.
    pub(crate) fn resolve(&self, store: &Store<StoreEntry>) -> Result<StoreEntry, OperationError> {
        match self {
            Param::Literal(v) => {
                let ty = v.get_type();
                Ok(StoreEntry::Var {
                    value: v.clone(),
                    ty,
                })
            }

            Param::Ref(path) => {
                store
                    .get(path)
                    .cloned()
                    .map_err(|_| OperationError::ReferenceNotFound {
                        reference: path.clone(),
                    })
            }

            Param::Template(parts) => {
                let mut result = String::new();
                for part in parts {
                    let resolved = part.resolve(store)?;
                    match resolved {
                        StoreEntry::Var { value, .. } => {
                            result.push_str(&value.to_string());
                        }
                        _ => {
                            return Err(OperationError::InvalidTemplatePart);
                        }
                    }
                }
                Ok(StoreEntry::Var {
                    ty: Type::Text,
                    value: Value::Text(result),
                })
            }

            Param::Array(items) => {
                let resolved: Result<Vec<StoreEntry>, _> =
                    items.iter().map(|item| item.resolve(store)).collect();
                Ok(StoreEntry::Array(resolved?))
            }

            Param::Map(entries) => {
                let resolved: Result<HashMap<String, StoreEntry>, _> = entries
                    .iter()
                    .map(|(k, v)| v.resolve(store).map(|resolved| (k.clone(), resolved)))
                    .collect();
                Ok(StoreEntry::Map(resolved?))
            }
        }
    }

    pub fn get_references(&self) -> Vec<String> {
        match self {
            Param::Literal(_) => vec![],
            Param::Ref(path) => vec![path.clone()],
            Param::Template(parts) => parts.iter().flat_map(|p| p.get_references()).collect(),
            Param::Array(items) => items.iter().flat_map(|i| i.get_references()).collect(),
            Param::Map(entries) => entries.values().flat_map(|v| v.get_references()).collect(),
        }
    }
}

impl<V: Into<Value>> From<V> for Param {
    fn from(v: V) -> Self {
        // In most cases we'd want to resolve these down into
        Param::Literal(v.into())
    }
}

#[derive(Debug, Clone)]
pub struct Parameters(HashMap<String, Param>);

impl Parameters {
    pub fn new(map: HashMap<String, Param>) -> Self {
        Parameters(map)
    }

    pub fn get(&self, key: &str) -> Option<&Param> {
        self.0.get(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &Param> {
        self.0.values()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Param)> {
        self.0.iter()
    }

    fn resolve_all(
        &self,
        store: &Store<StoreEntry>,
    ) -> Result<HashMap<String, StoreEntry>, OperationError> {
        self.0
            .iter()
            .map(|(k, v)| v.resolve(store).map(|resolved| (k.clone(), resolved)))
            .collect()
    }

    pub fn resolve_in_store(
        &self,
        prefix_part: impl Into<String>,
        store: &mut Store<StoreEntry>,
    ) -> Result<String, OperationError> {
        let resolved = self.resolve_all(store)?;
        let prefix: String = prefix_part.into();

        for (key, value) in resolved {
            store
                .insert(format!("{}.{}", prefix, key), value)
                .map_err(|e| OperationError::ParameterResolutionFailed {
                    parameter: key.clone(),
                    reason: e.to_string(),
                })?;
        }
        Ok(prefix)
    }

    pub fn resolve_to_store(
        &self,
        prefix_part: impl Into<String>,
        src: &Store<StoreEntry>,
        dst: &mut Store<StoreEntry>,
    ) -> Result<(), OperationError> {
        let resolved = self.resolve_all(src)?;
        let prefix: String = prefix_part.into();

        for (key, value) in resolved {
            dst.insert(format!("{}.{}", prefix, key), value)
                .map_err(|e| OperationError::ParameterResolutionFailed {
                    parameter: key.clone(),
                    reason: e.to_string(),
                })?;
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! params {
    () => {
        std::collections::HashMap::<String, Param>::new()
    };
    ($($key:expr => $value:expr),+ $(,)?) => {{
        let mut map = std::collections::HashMap::<String, Param>::new();
        $(
            map.insert($key.into(), $value.into());
        )+
        Parameters::new(map)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_literals() {
        let mut store = Store::<StoreEntry>::new();
        store.define_var("unused", "placeholder").unwrap();

        let p = params!(
            "name" => "number",
            "value" => 42i64,
            "flag" => true,
        );

        let resolved = p.get("name").unwrap().resolve(&store).unwrap();
        assert_eq!(
            resolved,
            StoreEntry::Var {
                value: Value::Text("number".into()),
                ty: Type::Text,
            }
        );

        let resolved = p.get("value").unwrap().resolve(&store).unwrap();
        assert_eq!(
            resolved,
            StoreEntry::Var {
                value: Value::Integer(42),
                ty: Type::Integer,
            }
        );
    }

    #[test]
    fn reference_resolution() {
        let mut store = Store::<StoreEntry>::new();
        store.define_var("workspace_id", "ws-abc-123").unwrap();

        let p = params!(
            "workspace" => Param::reference("workspace_id"),
        );

        let resolved = p.get("workspace").unwrap().resolve(&store).unwrap();
        assert_eq!(
            resolved,
            StoreEntry::Var {
                value: Value::Text("ws-abc-123".into()),
                ty: Type::Text,
            }
        );
    }

    #[test]
    fn reference_missing() {
        let store = Store::<StoreEntry>::new();

        let p = Param::reference("nonexistent");
        assert!(p.resolve(&store).is_err());
    }

    #[test]
    fn template_with_refs() {
        let mut store = Store::<StoreEntry>::new();
        store.define_var("days", 7i64).unwrap();

        let p = Param::template(vec![
            Param::literal("SigninLogs | ago("),
            Param::reference("days"),
            Param::literal("d)"),
        ]);

        let resolved = p.resolve(&store).unwrap();
        assert_eq!(
            resolved,
            StoreEntry::Var {
                value: Value::Text("SigninLogs | ago(7d)".into()),
                ty: Type::Text,
            }
        );
    }

    #[test]
    fn array_of_literals_and_refs() {
        let mut store = Store::<StoreEntry>::new();
        store.define_var("auto_label", "auto-triaged").unwrap();

        let p = Param::array(vec![
            Param::literal("high-priority"),
            Param::reference("auto_label"),
        ]);

        let resolved = p.resolve(&store).unwrap();
        match resolved {
            StoreEntry::Array(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(
                    items[0].get_value().unwrap(),
                    &Value::Text("high-priority".into())
                );
                assert_eq!(
                    items[1].get_value().unwrap(),
                    &Value::Text("auto-triaged".into())
                );
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn inline_map() {
        let mut store = Store::<StoreEntry>::new();
        store.define_var("ws_id", "ws-001").unwrap();

        let p = params!(
            "severity" => "High",
            "config" => Param::map(HashMap::from([
                ("workspace".into(), Param::reference("ws_id")),
                ("timeout".into(), Param::literal(30i64)),
            ])),
        );

        let resolved = p.get("config").unwrap().resolve(&store).unwrap();
        match resolved {
            StoreEntry::Map(entries) => {
                assert_eq!(
                    entries.get("workspace").unwrap().get_value().unwrap(),
                    &Value::Text("ws-001".into())
                );
                assert_eq!(
                    entries.get("timeout").unwrap().get_value().unwrap(),
                    &Value::Integer(30)
                );
            }
            _ => panic!("Expected Map"),
        }
    }

    #[test]
    fn nested_map_with_array_and_template() {
        let mut store = Store::<StoreEntry>::new();
        store.define_var("tenant", "contoso.com").unwrap();
        store.define_var("days", 30i64).unwrap();

        let p = Param::map(HashMap::from([
            ("workspace".into(), Param::literal("ws-sentinel-01")),
            (
                "scopes".into(),
                Param::array(vec![
                    Param::literal("https://graph.microsoft.com/.default"),
                    Param::literal("https://management.azure.com/.default"),
                ]),
            ),
            (
                "query".into(),
                Param::template(vec![
                    Param::literal("SigninLogs | where TenantId == '"),
                    Param::reference("tenant"),
                    Param::literal("' | ago("),
                    Param::reference("days"),
                    Param::literal("d)"),
                ]),
            ),
        ]));

        let resolved = p.resolve(&store).unwrap();
        match resolved {
            StoreEntry::Map(entries) => {
                assert_eq!(
                    entries.get("workspace").unwrap().get_value().unwrap(),
                    &Value::Text("ws-sentinel-01".into())
                );

                match entries.get("scopes").unwrap() {
                    StoreEntry::Array(items) => assert_eq!(items.len(), 2),
                    _ => panic!("Expected Array for scopes"),
                }

                assert_eq!(
                    entries.get("query").unwrap().get_value().unwrap(),
                    &Value::Text("SigninLogs | where TenantId == 'contoso.com' | ago(30d)".into())
                );
            }
            _ => panic!("Expected Map"),
        }
    }

    #[test]
    fn get_references_extracts_all_refs() {
        let p = Param::map(HashMap::from([
            ("literal".into(), Param::literal("ignored")),
            ("ref".into(), Param::reference("ws_id")),
            (
                "nested".into(),
                Param::array(vec![
                    Param::reference("tenant"),
                    Param::template(vec![Param::literal("prefix_"), Param::reference("suffix")]),
                ]),
            ),
        ]));

        let mut refs = p.get_references();
        refs.sort();
        assert_eq!(refs, vec!["suffix", "tenant", "ws_id"]);
    }
}
