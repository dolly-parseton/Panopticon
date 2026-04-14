use crate::imports::*;

/// A static description of an [`Operation`]'s inputs, outputs, and
/// extension requirements.
///
/// Returned by [`Operation::metadata`] and consumed by the
/// [`Registry`] during draft-time validation. Because every field is a
/// `&'static` reference, the metadata is allocation-free — an
/// implementation typically returns a struct literal built from `const`
/// arrays.
#[derive(Debug, Clone)]
pub struct OperationMetadata {
    /// The operation's canonical name, surfaced in error messages and
    /// in logs from the [`Logger`](crate::prelude::Logger) hook.
    pub name: &'static str,
    /// A human-readable description of what the operation does.
    pub description: &'static str,
    /// The operation's declared inputs, in declaration order.
    pub inputs: &'static [InputSpec],
    /// The operation's declared outputs, in declaration order.
    pub outputs: &'static [OutputSpec],
    /// The extensions the operation requires at runtime.
    pub requires_extensions: &'static [ExtensionSpec],
}

/// A single declared input on an [`OperationMetadata`].
#[derive(Debug, Clone, PartialEq)]
pub struct InputSpec {
    /// The input's name — used as the lookup key for
    /// [`Context::input`](crate::extend::Context).
    pub name: &'static str,
    /// The expected type. Use [`Type::Any`] to opt out of type
    /// checking.
    pub ty: Type,
    /// Whether the input must be supplied by the caller. If `false`
    /// and absent, the operation is expected to supply its own fallback
    /// via `default` or custom logic.
    pub required: bool,
    /// An optional fallback value used when the input is absent.
    pub default: Option<Value>,
    /// A human-readable description of the input's role.
    pub description: &'static str,
}

/// A single declared output on an [`OperationMetadata`].
#[derive(Debug, Clone, PartialEq)]
pub struct OutputSpec {
    /// How the output's runtime name is determined. See [`NameSpec`].
    pub name: NameSpec,
    /// The output's type tag. Use [`Type::Any`] when the type varies
    /// with the input.
    pub ty: Type,
    /// A human-readable description of the output's role.
    pub description: &'static str,
    /// Where the output is visible: globally or only within the
    /// operation's outputs for the current step.
    pub scope: OutputScope,
}

/// How the runtime name of an [`OutputSpec`] or [`ExtensionSpec`] is
/// determined.
///
/// Static names are fixed at declaration time; derived names are
/// looked up from a `Type::Text` input when the step runs, optionally
/// falling back to a static default.
#[derive(Debug, Clone, PartialEq)]
pub enum NameSpec {
    /// A fixed name known at declaration time.
    Static(&'static str),
    /// The name is read from the named input at runtime. The input
    /// must be declared on the operation with `ty: Type::Text`.
    DerivedFrom(&'static str),
    /// As for [`DerivedFrom`](Self::DerivedFrom), but falls back to
    /// `default` when the input is absent at runtime. The referenced
    /// input must be optional (`required: false`).
    DerivedWithDefault {
        /// The name of the input to read the output name from.
        input_name: &'static str,
        /// The fallback name used when the input is absent.
        default: &'static str,
    },
}

/// Where an [`OutputSpec`]'s runtime value is written.
///
/// Global outputs are merged into the runtime [`Store`](crate::prelude::Store)
/// and become visible to every subsequent step and return block.
/// Operation-scoped outputs are visible only within the current step
/// — callers read them from
/// [`HookEvent::AfterStep`](crate::extend::HookEvent) but they do not
/// persist.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputScope {
    /// Visible to every subsequent step and return block.
    Global,
    /// Visible only for the duration of the current step.
    Operation,
}

/// A declared extension requirement on an [`OperationMetadata`].
#[derive(Debug, Clone)]
pub struct ExtensionSpec {
    /// How the runtime name of the required extension is determined.
    pub name: NameSpec,
    /// A human-readable description of what the operation needs from
    /// the extension.
    pub description: &'static str,
    /// A factory that produces the [`TypeId`] of the extension type.
    /// Stored as a function rather than a value because `TypeId::of`
    /// is only `const` on recent compilers.
    pub type_id: fn() -> TypeId,
}
