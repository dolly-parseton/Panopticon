use crate::imports::*;

pub mod iterative;
#[cfg(test)]
pub mod tests;

// Sealed module to prevent external construction of Namespace (lol how obvious is it I've just learn about generics)
pub(crate) mod sealed {
    pub struct BuilderToken(pub(super) ());
    #[doc(hidden)]
    pub struct Once(());
    #[doc(hidden)]
    pub struct Iterative(());
    #[doc(hidden)]
    pub struct Static(());
    #[doc(hidden)]
    pub trait Build {
        fn build(self) -> super::Result<super::Namespace>; // Cooked w this one
    }
}

/*
    Consts:
    * DEFAULT_ITER_VAR - Default variable name for the current item in iterative namespaces
    * DEFAULT_INDEX_VAR - Default variable name for the current index in iterative namespaces
    * RESERVED_NAMESPACES - List of reserved namespace names that cannot be used
*/
pub const DEFAULT_ITER_VAR: &str = "item";
pub const DEFAULT_INDEX_VAR: &str = "index";
pub const RESERVED_NAMESPACES: [&str; 2] = [DEFAULT_ITER_VAR, DEFAULT_INDEX_VAR];

/*
    Types:
    * Namespace - Represents a namespace with a name and execution mode
    * ExecutionMode - Enum representing the execution mode of a namespace (Once, Iterative, Static)
    * IteratorType - Enum representing the type of iterator for iterative namespaces
    * NamespaceBuilder - Builder pattern for constructing Namespace instances
    * NamespaceHandle - Handle for adding commands to a specific namespace
*/
#[derive(Debug, Clone)]
pub struct Namespace {
    name: String,
    ty: ExecutionMode,
}

impl Namespace {
    pub fn builder(name: &str) -> NamespaceBuilder<sealed::Once> {
        NamespaceBuilder::<sealed::Once>::new(name)
    }

    pub fn new<T: Into<String>>(name: T, ty: ExecutionMode, _: sealed::BuilderToken) -> Self {
        Namespace {
            name: name.into(),
            ty,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ty(&self) -> &ExecutionMode {
        &self.ty
    }
}

#[derive(Debug, Default, Clone)]
pub enum ExecutionMode {
    #[default]
    Once,
    Iterative {
        store_path: StorePath,
        source: IteratorType,
        iter_var: Option<String>,  // If None, defaults to DEFAULT_ITER_VAR
        index_var: Option<String>, // If None, defaults to DEFAULT_INDEX_VAR
    },
    Static {
        values: std::collections::HashMap<String, ScalarValue>,
    },
}

impl ExecutionMode {
    pub async fn resolve_iter_values(
        &self,
        context: &ExecutionContext,
    ) -> Result<Vec<ScalarValue>> {
        iterative::resolve_iterator_values(context, self).await
    }
}

pub struct NamespaceBuilder<T> {
    name: String,
    // options
    store_path: Option<StorePath>,
    source: Option<IteratorType>,
    iter_var: Option<String>,
    index_var: Option<String>,
    values: Option<std::collections::HashMap<String, ScalarValue>>,
    // marker
    _marker: std::marker::PhantomData<T>,
}

pub struct NamespaceHandle<'a, T> {
    pub(crate) commands: &'a mut Pipeline,
    pub(crate) namespace_index: usize,
    pub(crate) _marker: std::marker::PhantomData<T>,
}

impl<'a> NamespaceHandle<'a, sealed::Once> {
    pub fn add_command<T>(&mut self, name: &str, attrs: &Attributes) -> Result<()>
    where
        T: Command,
    {
        self.commands
            .add_command::<T>(self.namespace_index, name, attrs)
    }
}

impl<'a> NamespaceHandle<'a, sealed::Iterative> {
    pub fn add_command<T>(&mut self, name: &str, attrs: &Attributes) -> Result<()>
    where
        T: Command,
    {
        self.commands
            .add_command::<T>(self.namespace_index, name, attrs)
    }
}

#[derive(Debug, Clone)]
pub enum IteratorType {
    ScalarStringSplit {
        delimiter: String,
    },
    ScalarArray {
        range: Option<(usize, usize)>,
    },
    ScalarObjectKeys {
        keys: Option<Vec<String>>,
        exclude: bool,
    },
    TabularColumn {
        column: String,
        range: Option<(usize, usize)>,
    },
}

mod once {
    use super::sealed;
    use super::*;
    impl sealed::Build for NamespaceBuilder<sealed::Once> {
        fn build(self) -> Result<Namespace> {
            NamespaceBuilder::<sealed::Once>::build(self)
        }
    }
    impl NamespaceBuilder<sealed::Once> {
        pub fn new(name: &str) -> Self {
            NamespaceBuilder {
                name: name.to_string(),
                store_path: None,
                source: None,
                iter_var: None,
                index_var: None,
                values: None,
                _marker: std::marker::PhantomData,
            }
        }
        fn build(self) -> Result<Namespace> {
            if RESERVED_NAMESPACES.contains(&self.name.as_str()) {
                return Err(anyhow::anyhow!(
                    "Namespace name '{}' is reserved",
                    self.name
                ));
            }
            Ok(Namespace::new(
                self.name,
                ExecutionMode::Once,
                sealed::BuilderToken(()),
            ))
        }
        pub fn iterative(self) -> NamespaceBuilder<sealed::Iterative> {
            NamespaceBuilder {
                name: self.name,
                store_path: None,
                source: None,
                iter_var: None,
                index_var: None,
                values: None,
                _marker: std::marker::PhantomData,
            }
        }
        pub fn static_ns(self) -> NamespaceBuilder<sealed::Static> {
            NamespaceBuilder {
                name: self.name,
                store_path: None,
                source: None,
                iter_var: None,
                index_var: None,
                values: Some(std::collections::HashMap::new()),
                _marker: std::marker::PhantomData,
            }
        }
    }
}

mod static_ns {
    use super::sealed;
    use super::*;
    impl sealed::Build for NamespaceBuilder<sealed::Static> {
        fn build(self) -> Result<Namespace> {
            NamespaceBuilder::<sealed::Static>::build(self)
        }
    }
    impl NamespaceBuilder<sealed::Static> {
        fn build(self) -> Result<Namespace> {
            let values = self
                .values
                .ok_or_else(|| anyhow::anyhow!("values are required for static namespace"))?;
            Ok(Namespace::new(
                self.name,
                ExecutionMode::Static { values },
                sealed::BuilderToken(()),
            ))
        }
        pub fn insert<T: Into<String>>(mut self, key: T, value: ScalarValue) -> Self {
            if let Some(ref mut vals) = self.values {
                vals.insert(key.into(), value);
            }
            self
        }
        pub fn object<F>(mut self, key: impl Into<String>, f: F) -> Self
        where
            F: FnOnce(ObjectBuilder) -> ObjectBuilder,
        {
            let builder = f(ObjectBuilder::new());
            if let Some(ref mut vals) = self.values {
                vals.insert(key.into(), builder.build_scalar());
            }
            self
        }
    }
}
