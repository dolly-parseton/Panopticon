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

    // Namespace
    pub use crate::namespace::{Namespace, NamespaceBuilder};

    // Context
    pub use crate::values::context::ExecutionContext;
    pub use crate::values::scalar::ObjectBuilder;
    pub use crate::values::scalar::ScalarValue;
    pub use crate::values::store_path::StorePath;
    pub use crate::values::tabular::TabularValue;
}

// Internal imports - layers on prelude with crate-internal types and std-lib conveniences.
// All internal modules use `use crate::imports::*` as their single import line.
pub(crate) mod imports {
    pub use crate::prelude::*;

    // Internal value types (not part of public API)
    pub use crate::values::helpers::{
        InsertBatch, is_truthy, parse_scalar, scalar_type_of, to_scalar,
    };
    pub use crate::values::scalar::{ScalarAsExt, ScalarMapExt, ScalarStore, ScalarType};
    pub use crate::values::tabular::TabularStore;

    // Namespace internals
    pub use crate::namespace::{ExecutionMode, IteratorType, NamespaceHandle, RESERVED_NAMESPACES};

    // Spec types
    pub use crate::spec::{
        FieldSpec, LiteralFieldRef, ObjectFields, ReferenceKind, TypeDef,
        attribute::{AttributeSpec, Attributes},
        builder::{CommandSpecBuilder, PendingAttribute},
        command::CommandSpec,
        result::{ResultKind, ResultSpec},
    };

    // Pipeline traits
    pub use crate::pipeline::traits::{
        Command, CommandFactory, Descriptor, Executable, FromAttributes,
    };

    // Result and error handling
    pub type Result<T> = anyhow::Result<T>;
    pub use anyhow::Context as _;

    // Std library
    pub use std::collections::{HashMap, HashSet, VecDeque};
    pub use std::path::PathBuf;
    pub use std::sync::{Arc, LazyLock};
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
