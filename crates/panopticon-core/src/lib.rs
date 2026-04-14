#![warn(missing_docs)]
//! # panopticon-core
//!
//! A typestate pipeline engine with compile-time reference validation,
//! hooks, and an extension system.
//!
//! The public surface is split into two tiers. Pick the one that matches
//! what you're doing:
//!
//! - [`prelude`] — building and running pipelines. Start here if you just
//!   want to declare variables, chain steps, attach built-in hooks, and
//!   read results. This is the import most users want.
//!
//! - [`extend`] — writing custom operations, hooks, or extensions, and
//!   tooling that walks compiled pipelines (serialisers, alternative
//!   drivers). Re-exports everything in [`prelude`] and adds the authoring
//!   APIs, error types, typestate markers, and the execution graph.
//!
//! ```no_run
//! use panopticon_core::prelude::*;
//!
//! let mut pipe = Pipeline::default();
//! pipe.var("name", "world")?;
//! pipe.step::<SetVar>(
//!     "greet",
//!     params!("name" => "greeting", "value" => "hello, world"),
//! )?;
//! let complete = pipe.compile()?.run().wait()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod data;
mod error;
mod execution;
mod extensions;
mod hooks;
mod operation;
mod param;
mod state;

pub mod prelude {
    //! Everything needed to build and run a pipeline.
    //!
    //! Start here for declaring variables, chaining steps, attaching built-in
    //! hooks, and reading results. For authoring custom operations, hooks, or
    //! extensions, use [`crate::extend`] instead.
    //!
    //! # Reading order
    //!
    //! The items below are listed in the order a reader typically encounters
    //! them when building a pipeline from scratch.
    //!
    //! **Pipeline construction** — [`Pipeline`], [`Param`], [`params!`],
    //! [`GuardSource`], [`IterSource`].
    //!
    //! **Collection handles** — [`ArrayHandle`], [`MapHandle`].
    //!
    //! **Store and values** — [`Store`], [`StoreEntry`], [`Type`], [`Value`],
    //! `DeserializeError` (requires the `serde` feature).
    //!
    //! **Built-in hooks** — [`EventKind`], [`EventLog`], [`EventRecord`],
    //! [`Logger`], [`Profiler`], [`StepFilter`], [`StoreValidator`], [`Timeout`].
    //!
    //! **Built-in operations** — [`SetVar`], [`Get`], [`Coerce`], [`Compare`],
    //! [`Dedupe`], [`Len`].

    // Pipeline construction.
    pub use crate::execution::{GuardSource, IterSource};
    pub use crate::param::Param;
    pub use crate::params;
    pub use crate::state::Pipeline;

    // Handles returned by `Pipeline::array` / `Pipeline::map`.
    pub use crate::data::{ArrayHandle, MapHandle};

    // Data types exposed when inspecting results or populating a `Store`
    // through `with_array` / `with_map` closures.
    pub use crate::data::{Store, StoreEntry, Type, Value};

    #[cfg(feature = "serde")]
    pub use crate::data::DeserializeError;

    // Built-in hooks.
    pub use crate::hooks::core::{
        EventKind, EventLog, EventRecord, Logger, Profiler, StepFilter, StoreValidator, Timeout,
    };

    // Built-in operations.
    pub use crate::operation::core::{Coerce, Compare, Dedupe, Get, Len, SetVar};
}

pub mod extend {
    //! Authoring surface for custom operations, hooks, extensions, and tooling
    //! that walks compiled pipelines.
    //!
    //! Re-exports everything in [`crate::prelude`] and adds the types needed
    //! to author new pipeline components or to introspect a compiled pipeline
    //! (for serialisers, alternative drivers, and similar tooling).
    //!
    //! # Reading order
    //!
    //! The items below are listed in the order a reader typically encounters
    //! them when extending the crate. Prelude re-exports are omitted.
    //!
    //! **Error types** — [`DraftError`], [`OperationError`], [`AccessError`],
    //! [`StoreError`], [`op_error!`].
    //!
    //! **Operation authoring** — [`Operation`], [`Context`], [`OperationMetadata`],
    //! [`InputSpec`], [`OutputSpec`], [`NameSpec`], [`OutputScope`],
    //! [`ExtensionSpec`], [`Parameters`], [`Registry`].
    //!
    //! **Hook authoring** — [`Hook`], [`HookEvent`], [`HookAction`].
    //!
    //! **Iteration context** — [`IterContext`], [`IterIndex`], [`ITER_INDEX`],
    //! [`ITER_ITEM`], [`ITER_KEY`], [`ITER_VALUE`].
    //!
    //! **Extensions** — [`Extension`].
    //!
    //! **Typestates and execution graph** — [`Draft`], [`Ready`], [`Running`],
    //! [`RunningStatus`], [`Complete`], [`ExecutionNode`].

    pub use crate::prelude::*;

    // Error types.
    pub use crate::error::{AccessError, DraftError, OperationError, StoreError};

    // Operation authoring.
    pub use crate::op_error;
    pub use crate::operation::{
        Context, ExtensionSpec, InputSpec, NameSpec, Operation, OperationMetadata, OutputScope,
        OutputSpec, Registry,
    };
    pub use crate::param::Parameters;

    // Hook authoring.
    pub use crate::execution::{IterContext, IterIndex};
    pub use crate::execution::{ITER_INDEX, ITER_ITEM, ITER_KEY, ITER_VALUE};
    pub use crate::hooks::{Hook, HookAction, HookEvent};

    // Extension authoring.
    pub use crate::extensions::Extension;

    // Typestate markers and execution graph — for tooling that walks or
    // transforms a compiled pipeline.
    pub use crate::execution::ExecutionNode;
    pub use crate::state::{Complete, Draft, Ready, Running, RunningStatus};
}

/// Crate-internal prelude. Gathers the names used across modules so each
/// file can `use crate::imports::*;` once instead of maintaining a per-file
/// import list. Some re-exports (built-in ops, built-in hooks, the `params!`
/// macro) are only touched by in-module tests, so the `allow(unused_imports)`
/// suppresses false positives in non-test builds. Not part of the public API.
#[allow(unused_imports)]
pub(crate) mod imports {
    pub use crate::data::*;
    pub use crate::error::*;
    pub use crate::execution::*;
    pub use crate::extensions::{Extension, Extensions};
    pub use crate::hooks::core::*;
    pub use crate::hooks::{Hook, HookAction, HookEvent};
    pub(crate) use crate::hooks::emit_all;
    pub use crate::operation::core::*;
    pub use crate::op_error;
    pub use crate::operation::*;
    pub use crate::param::*;
    pub use crate::params;
    pub use crate::state::*;

    pub use std::any::TypeId;
    pub use std::collections::HashMap;
    pub use std::sync::{Arc, Mutex, atomic::AtomicBool};
    pub use std::thread;
}
