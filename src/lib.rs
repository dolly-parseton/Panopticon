mod data;
mod error;
mod execution;
mod extensions;
mod hooks;
mod operation;
mod param;
mod state;

pub mod prelude {
    pub use crate::execution::{GuardSource, IterSource};
    pub use crate::hooks::core::*;
    pub use crate::op_error;
    pub use crate::operation::core::*;
    pub use crate::param::{Param, Parameters};
    pub use crate::params;

    pub use crate::state::Pipeline;

    pub use crate::data::{ArrayHandle, MapHandle, Store, StoreEntry, Type, Value};

    #[cfg(feature = "serde")]
    pub use crate::data::DeserializeError;
}

pub mod extend {
    pub use crate::prelude::*;

    pub use crate::data::{ArrayHandle, MapHandle, Store, StoreEntry, Type, Value};
    pub use crate::error::*;
    pub use crate::extensions::{Extension, Extensions};
    pub use crate::hooks::{Hook, HookAction, HookCallback, HookEvent};
    pub use crate::operation::*;
    pub use crate::param::*;
}

pub(crate) mod imports {
    pub use crate::data::*;
    pub use crate::execution::*;
    pub use crate::extend::*;
    pub(crate) use crate::hooks::emit_all;
    pub use crate::state::*;
    // pub use crate::param::*;

    // std imports
    pub use std::any::TypeId;
    pub use std::collections::HashMap;
    pub use std::sync::{Arc, Mutex, atomic::AtomicBool};
    pub use std::thread;
}
