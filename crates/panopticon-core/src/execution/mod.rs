mod context;
mod control;
pub(crate) mod dependencies;
pub(crate) mod node;
pub(crate) mod runner;

/// Binding suffix for the current element's index inside an
/// [`iter_array`](crate::prelude::Pipeline#method.iter_array) body.
/// The binding is injected into each iteration's cloned store as
/// `"{iter_name}.{ITER_INDEX}"`.
pub const ITER_INDEX: &str = "__index";
/// Binding suffix for the current element's value inside an
/// [`iter_array`](crate::prelude::Pipeline#method.iter_array) body.
/// Injected as `"{iter_name}.{ITER_ITEM}"`.
pub const ITER_ITEM: &str = "__item";
/// Binding suffix for the current entry's key inside an
/// [`iter_map`](crate::prelude::Pipeline#method.iter_map) body.
/// Injected as `"{iter_name}.{ITER_KEY}"`.
pub const ITER_KEY: &str = "__key";
/// Binding suffix for the current entry's value inside an
/// [`iter_map`](crate::prelude::Pipeline#method.iter_map) body.
/// Injected as `"{iter_name}.{ITER_VALUE}"`.
pub const ITER_VALUE: &str = "__value";

pub use context::*;
pub use control::*;
pub use node::*;
