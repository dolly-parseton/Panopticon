pub mod aggregate;
pub mod condition;
pub mod file;
pub mod sql;
pub mod template;

use crate::imports::*;

// Helper type for CommandSchemas defined with LazyLock
pub type CommandSchema = LazyLock<(
    Vec<AttributeSpec<&'static str>>,
    Vec<ResultSpec<&'static str>>,
)>;
