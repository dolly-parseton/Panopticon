mod context;
mod control;
pub(crate) mod dependencies;
pub(crate) mod node;
pub(crate) mod runner;

// Binding name constants for iteration scopes
pub const ITER_INDEX: &str = "__index";
pub const ITER_ITEM: &str = "__item";
pub const ITER_KEY: &str = "__key";
pub const ITER_VALUE: &str = "__value";

pub use context::*;
pub use control::*;
pub use node::*;
