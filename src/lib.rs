mod commands;
mod dependencies;
mod namespace;
mod pipeline;
mod spec;
mod values;

// Public API - external consumers use this via `panopticon_core::prelude::*`
pub mod prelude {
    // Built-in Commands
    pub use crate::commands::aggregate::AggregateCommand;
    pub use crate::commands::condition::ConditionCommand;
    pub use crate::commands::file::FileCommand;
    pub use crate::commands::sql::SqlCommand;
    pub use crate::commands::template::TemplateCommand;

    // Pipeline
    pub use crate::pipeline::Pipeline;
    pub use crate::pipeline::results::{ResultSettings, ResultStore};

    // Namespace
    pub use crate::namespace::{Namespace, NamespaceBuilder};

    // Context
    pub use crate::values::scalar::ObjectBuilder;
    pub use crate::values::scalar::ScalarValue;
    pub use crate::values::store_path::StorePath;
    pub use crate::values::tabular::TabularValue;
}

// Extension API - used to add custom commands, etc.
// Consumers building custom commands use `panopticon_core::extend::*`
pub mod extend {
    // Traits - implement these to create a custom command
    pub use crate::pipeline::traits::{
        Command, CommandFactory, Descriptor, Executable, FromAttributes,
    };

    // Spec types - declare your command's attributes and results
    pub use crate::spec::{
        DEFAULT_NAME_POLICY, FieldSpec, LiteralFieldRef, NamePolicy, ObjectFields, ReferenceKind,
        TypeDef,
        attribute::{AttributeSpec, Attributes},
        builder::{CommandSpecBuilder, PendingAttribute},
        result::{ResultKind, ResultSpec},
    };

    // Value types - used in trait signatures and command implementations
    pub use crate::values::context::ExecutionContext;
    pub use crate::values::helpers::InsertBatch;
    pub use crate::values::scalar::{ScalarAsExt, ScalarMapExt, ScalarType};

    // Command helper type for LazyLock-time CommandSchemas
    pub use crate::commands::CommandSchema;

    // Re-exports from external crates needed to implement traits
    pub use async_trait::async_trait;
    pub type Result<T> = anyhow::Result<T>;
    pub use std::sync::LazyLock;
}

// Internal imports - layers on prelude with crate-internal types and std-lib conveniences.
// All internal modules use `use crate::imports::*` as their single import line.
pub(crate) mod imports {
    pub use crate::extend::*;
    pub use crate::prelude::*;

    // Internal value types (not part of public extend API)
    pub(crate) use crate::values::helpers::{is_truthy, parse_scalar, scalar_type_of, to_scalar};
    pub(crate) use crate::values::scalar::ScalarStore;
    pub(crate) use crate::values::tabular::TabularStore;

    // Namespace internals
    pub(crate) use crate::namespace::{
        ExecutionMode, IteratorType, NamespaceHandle, RESERVED_NAMESPACES,
    };

    // Spec internals
    pub(crate) use crate::spec::command::CommandSpec;

    // Pipeline result internals
    pub(crate) use crate::pipeline::{
        order::{ExecutionGroup, ExecutionPlan},
        results::{CommandResults, ResultValue},
    };

    // Result and error handling (shadow extend::Result with identical definition + add Context)
    pub type Result<T> = anyhow::Result<T>;
    pub use anyhow::Context as _;

    // Std library
    pub use std::collections::{HashMap, HashSet, VecDeque};
    pub use std::path::PathBuf;
    pub use std::sync::Arc;
    pub use tokio::sync::RwLock;
}

#[cfg(test)]
pub(crate) mod test_utils {
    pub fn init_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("debug")
            .with_test_writer()
            .try_init();
    }
}
