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

/// The authoring surface for custom pipeline operations.
///
/// An `Operation` is the reusable definition for a kind of step —
/// [`SetVar`](crate::prelude::SetVar), [`Get`](crate::prelude::Get), and
/// the other types in the `prelude` are all `Operation` implementors.
/// Implementations are unit (or zero-sized) structs: they provide
/// static [`metadata`](Self::metadata) describing their inputs, outputs,
/// and extension requirements, and an [`execute`](Self::execute)
/// function that reads inputs, performs work, and writes outputs via a
/// [`Context`]. Callers instantiate an operation as a step by passing
/// it as the type parameter to
/// [`Pipeline::step`](crate::prelude::Pipeline#method.step).
///
/// ```no_run
/// use panopticon_core::extend::*;
///
/// #[derive(Default)]
/// struct Greet;
///
/// impl Operation for Greet {
///     fn metadata() -> OperationMetadata {
///         OperationMetadata {
///             name: "Greet",
///             description: "Produces a greeting from a name.",
///             inputs: &[InputSpec {
///                 name: "name",
///                 ty: Type::Text,
///                 required: true,
///                 default: None,
///                 description: "The name to greet",
///             }],
///             outputs: &[OutputSpec {
///                 name: NameSpec::Static("greeting"),
///                 ty: Type::Text,
///                 description: "The resulting greeting",
///                 scope: OutputScope::Global,
///             }],
///             requires_extensions: &[],
///         }
///     }
///
///     fn execute(ctx: &mut Context) -> Result<(), OperationError> {
///         let name = ctx.input("name")?.get_value()?.as_text()?.to_string();
///         ctx.set_static_output("greeting", Value::Text(format!("hello, {}", name)))?;
///         Ok(())
///     }
/// }
/// ```
pub trait Operation: 'static {
    /// Returns a static description of this operation's inputs,
    /// outputs, and extension requirements. Called once per operation
    /// when [`Pipeline::step`](crate::prelude::Pipeline#method.step)
    /// first registers it; the result is cached in the operation
    /// [`Registry`].
    fn metadata() -> OperationMetadata
    where
        Self: Sized;
    /// Runs the operation against a [`Context`]. Reads inputs via
    /// `context.input`, reads extensions via `context.extension`, and
    /// writes outputs via `context.set_static_output` or
    /// `context.set_derived_output`.
    fn execute(context: &mut Context) -> Result<(), OperationError>;
}
