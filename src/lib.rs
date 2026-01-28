mod commands;
mod execution;
mod traits;
mod types;
mod utils;

pub mod prelude {
    // Execution Module
    pub use crate::execution::*;

    // Core types
    pub use crate::traits::*;
    pub use crate::types::*;
    pub use crate::utils::*;

    // Error handling - Context trait for adding context to errors
    pub use anyhow::Context as _;

    // Commands
    pub use crate::commands::*;
}
