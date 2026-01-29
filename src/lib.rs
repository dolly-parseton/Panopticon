mod commands;
mod dependencies;
mod namespace;
mod pipeline;
mod spec;
mod values;

// Library exports
pub mod prelude {
    // Built-in Commands
    pub use crate::commands::aggregate::AggregateCommand;
    pub use crate::commands::condition::ConditionCommand;
    pub use crate::commands::file::FileCommand;
    pub use crate::commands::sql::SqlCommand;
    pub use crate::commands::template::TemplateCommand;

    // Pipeline
    pub use crate::pipeline::Pipeline;

    // Namespace
    pub use crate::namespace::{Namespace, NamespaceBuilder};

    // Context
    pub use crate::values::context::ExecutionContext;
    pub use crate::values::scalar::ObjectBuilder;
    pub use crate::values::scalar::ScalarValue;
    pub use crate::values::store_path::StorePath;
    pub use crate::values::tabular::TabularValue;
}

// Internal imports for use within the crate
#[allow(unused_imports)]
pub(crate) mod imports {
    // Built-in Commands
    pub use crate::commands::aggregate::AggregateCommand;
    pub use crate::commands::condition::ConditionCommand;
    pub use crate::commands::file::FileCommand;
    pub use crate::commands::sql::SqlCommand;
    pub use crate::commands::template::TemplateCommand;

    // Core types
    pub use crate::values::{context::*, helpers::*, scalar::*, store_path::StorePath, tabular::*};

    pub use crate::namespace::{ExecutionMode, IteratorType, Namespace, NamespaceBuilder};

    pub use crate::spec::{
        FieldSpec, ReferenceKind, TypeDef,
        attribute::{AttributeSpec, Attributes},
        command::CommandSpec,
        result::ResultSpec,
    };

    pub use crate::pipeline::Pipeline;
    pub use crate::pipeline::traits::{
        Command, CommandFactory, Descriptor, Executable, FromAttributes,
    };

    // Consts
    pub use crate::namespace::RESERVED_NAMESPACES;
    pub use crate::pipeline::traits::COMMON_ATTRIBUTES;

    // Helpers
    // pub use crate::values::helpers::to_scalar;
    // pub use crate::values::scalar::{ScalarMapExt as _, ScalarValueExt as _};

    // Result and error handling
    pub type Result<T> = anyhow::Result<T>;
    pub use anyhow::Context as _;

    // File I/O
    pub use std::path::PathBuf;

    // Collections
    pub use std::collections::{HashMap, HashSet, VecDeque};

    // Async
    pub use std::sync::Arc;
    pub use tokio::sync::RwLock;

    // Lazy initialization
    pub use std::sync::LazyLock;

    // Testing - TODO, consider adding a broader set of test utilities.
    #[cfg(test)]
    pub fn init_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .try_init();
    }
}

/*
    Bits im not sure what do with:
    pub fn scalar_value_from<T: serde::Serialize>(input: T) -> Result<ScalarValue> {
    let value = tera::to_value(input)?;
    Ok(value)
}
*/
