mod context;
pub mod core; // built-in operations
mod metadata;
mod registry;

use crate::imports::*;

pub use context::Context;
pub use metadata::{
    ExtensionSpec, InputSpec, NameSpec, OperationMetadata, OutputScope, OutputSpec,
};
pub use registry::Registry;

// #[allow(unused)]
// pub use core::*;

pub trait Operation: 'static {
    fn metadata() -> OperationMetadata
    where
        Self: Sized;
    fn execute(context: &mut Context) -> Result<(), OperationError>;
}
