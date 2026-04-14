/// Errors produced while building or compiling a draft pipeline.
///
/// `DraftError` is returned by construction methods on `Pipeline<Draft>`
/// and by `Pipeline::compile`. Callers should match on the variants
/// when they want to surface specific failures (duplicate names,
/// unresolved references) with custom diagnostics — for example when a
/// loader is translating external configuration into a pipeline.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DraftError {
    /// A [`Pipeline::var`](crate::prelude::Pipeline#method.var) call used
    /// a name already bound to another variable.
    #[error("A variable named '{name}' already exists in the pipeline")]
    DuplicateVariable {
        /// The conflicting variable name.
        name: String,
    },

    /// A [`Pipeline::step`](crate::prelude::Pipeline#method.step) call
    /// used a name already bound to another step.
    #[error("A step named '{name}' already exists in the pipeline")]
    DuplicateStep {
        /// The conflicting step name.
        name: String,
    },

    /// A `Pipeline::returns` call used a name already bound to another
    /// return block.
    #[error("A return named '{name}' already exists in the pipeline")]
    DuplicateReturn {
        /// The conflicting return-block name.
        name: String,
    },

    /// Compile-time simulation found a step parameter referencing a
    /// store entry that is not produced by any preceding step.
    #[error(
        "Step '{step}' references '{reference}' which is not available at that point in the pipeline"
    )]
    UnresolvedReference {
        /// The step whose parameters contained the bad reference.
        step: String,
        /// The dotted store path that failed to resolve.
        reference: String,
    },

    /// Compile-time simulation found a return-block parameter
    /// referencing a store entry that is not available at the end of
    /// the pipeline.
    #[error(
        "Return '{returns}' references '{reference}' which is not available at the end of the pipeline"
    )]
    UnresolvedReturn {
        /// The return block whose parameters contained the bad
        /// reference.
        returns: String,
        /// The dotted store path that failed to resolve.
        reference: String,
    },

    /// Operation metadata failed validation when the operation was
    /// registered — typically a [`NameSpec::DerivedFrom`](crate::extend::NameSpec)
    /// referencing a missing input or an input of the wrong type.
    #[error("Invalid operation metadata for '{operation}': {reason}")]
    InvalidMetadata {
        /// The operation whose metadata failed validation.
        operation: &'static str,
        /// Human-readable explanation of the failure.
        reason: String,
    },

    /// A step declared an extension requirement that is not registered
    /// on the pipeline.
    #[error(
        "Step '{step}' (operation '{operation}') requires extension '{extension}' which is not registered on the pipeline"
    )]
    MissingExtension {
        /// The step whose operation requires the extension.
        step: String,
        /// The operation name from its metadata.
        operation: &'static str,
        /// The extension name that should be registered via
        /// [`Pipeline::extension`](crate::prelude::Pipeline#method.extension).
        extension: String,
    },
}

/// Errors produced while executing a running pipeline.
///
/// Returned by `Pipeline::wait` when the worker thread reports failure,
/// and surfaced through `OperationError` variants in custom operations.
/// Iteration and guard failures wrap the underlying cause in the
/// [`Iteration`](Self::Iteration) and [`Guard`](Self::Guard) variants so
/// the trace preserves the scope in which the failure occurred.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum OperationError {
    /// Parameter resolution tried to look up a dotted store path that
    /// did not exist at runtime.
    #[error("Reference '{reference}' not found in store")]
    ReferenceNotFound {
        /// The dotted store path that failed to resolve.
        reference: String,
    },

    /// A [`Param::Template`](crate::prelude::Param::Template) contained
    /// a part that resolved to an array or map instead of a scalar.
    #[error("Template parts must resolve to scalar values")]
    InvalidTemplatePart,

    /// Resolving a parameter into the runtime store failed — typically
    /// because the target key was already occupied.
    #[error("Failed to resolve parameter '{parameter}': {reason}")]
    ParameterResolutionFailed {
        /// The parameter name being resolved.
        parameter: String,
        /// Human-readable explanation from the underlying store error.
        reason: String,
    },

    /// A custom operation called `Context::input` with a name that was
    /// not declared in its [`OperationMetadata`](crate::extend::OperationMetadata).
    #[error("Operation '{operation}' has no declared input '{input}'")]
    UndeclaredInput {
        /// The operation name from its metadata.
        operation: &'static str,
        /// The undeclared input name that was requested.
        input: String,
    },

    /// A custom operation called `Context::set_static_output` with a
    /// name not present as a [`NameSpec::Static`](crate::extend::NameSpec)
    /// output in its metadata.
    #[error("Operation '{operation}' has no declared static output '{output}'")]
    UndeclaredOutput {
        /// The operation name from its metadata.
        operation: &'static str,
        /// The undeclared static output name.
        output: String,
    },

    /// A custom operation called `Context::set_derived_output` with an
    /// input name not referenced by any derived output in its metadata.
    #[error("Operation '{operation}' has no declared derived output from input '{input}'")]
    UndeclaredDerivedOutput {
        /// The operation name from its metadata.
        operation: &'static str,
        /// The input name the derived output should have come from.
        input: String,
    },

    /// An iteration node resolved its [`IterSource`](crate::prelude::IterSource)
    /// reference to nothing at runtime.
    #[error("Iteration '{iter_name}': source '{source_ref}' not found")]
    IterSourceNotFound {
        /// The iteration node's name.
        iter_name: String,
        /// The store path that failed to resolve.
        source_ref: String,
    },

    /// An iteration source resolved to an entry of the wrong shape —
    /// an array was expected but a map (or scalar) was found, or vice
    /// versa.
    #[error("Iteration '{iter_name}': source '{source_ref}' is not {expected}")]
    IterSourceTypeMismatch {
        /// The iteration node's name.
        iter_name: String,
        /// The store path whose entry had the wrong shape.
        source_ref: String,
        /// The shape the iteration expected (`"array"` or `"map"`).
        expected: &'static str,
    },

    /// A step inside an iteration body failed. Wraps the underlying
    /// error so the outer cause includes the iteration name, current
    /// index/key, and nesting depth.
    #[error("In iteration '{iter_name}' at index {index}: {source}")]
    Iteration {
        /// The enclosing iteration's name.
        iter_name: String,
        /// The current index or key as rendered by
        /// [`IterIndex::fmt`](crate::extend::IterIndex).
        index: String,
        /// Nesting depth of the iteration (outermost is `0`).
        depth: usize,
        /// The underlying error from the inner step.
        #[source]
        source: Box<OperationError>,
    },

    /// A traversal of a [`StoreEntry`](crate::prelude::StoreEntry) failed.
    #[error("Access error: {0}")]
    AccessError(#[from] AccessError),

    /// A [`Store`](crate::prelude::Store) operation failed at runtime.
    #[error("Store error: {0}")]
    StoreError(#[from] StoreError),

    /// The runner tried to look up the parameters for a step or return
    /// block that was never recorded at draft time — this indicates a
    /// bug in the caller and is effectively unreachable through normal
    /// construction APIs.
    #[error("No parameters found for '{name}'")]
    MissingParameters {
        /// The missing step or return-block name.
        name: String,
    },

    /// A hook interceptor returned [`HookAction::Abort`](crate::extend::HookAction::Abort).
    #[error("Hook '{hook}' aborted execution: {reason}")]
    HookAbort {
        /// The hook that aborted the pipeline.
        hook: String,
        /// The reason returned by the interceptor.
        reason: String,
    },

    /// `Pipeline::cancel` was observed between steps and the worker
    /// exited early.
    #[error("Operation cancelled")]
    Cancelled,

    /// The worker thread panicked. `Pipeline::wait` turns the panic
    /// into this variant rather than unwinding into the caller.
    #[error("Execution thread panicked")]
    ThreadPanic,

    /// `Context::extension` could not find the requested extension —
    /// either the operation did not declare it, or no extension was
    /// registered under that name and type.
    #[error("Extension '{extension}' not found for operation '{operation}'")]
    ExtensionNotFound {
        /// The operation requesting the extension.
        operation: String,
        /// The extension name that could not be resolved.
        extension: String,
    },

    /// A [`GuardSource`](crate::prelude::GuardSource) resolved to an
    /// entry that was not a [`Value::Boolean`](crate::prelude::Value::Boolean).
    #[error("Guard '{guard_name}': reference '{reference}' is not a boolean")]
    GuardTypeMismatch {
        /// The guard node's name.
        guard_name: String,
        /// The store path whose entry was not a boolean.
        reference: String,
    },

    /// A step inside a guard body failed. Wraps the underlying error so
    /// the outer cause includes the guard name.
    #[error("In guard '{guard_name}': {source}")]
    Guard {
        /// The enclosing guard's name.
        guard_name: String,
        /// The underlying error from the inner step.
        #[source]
        source: Box<OperationError>,
    },

    /// A custom operation emitted an ad-hoc error via
    /// [`Context::error`](crate::extend::Context) or the
    /// [`op_error!`](crate::op_error) macro.
    #[error("[{operation}] {message}")]
    Custom {
        /// The operation name (from metadata) that emitted the error.
        operation: String,
        /// The operation-supplied message.
        message: String,
    },
}

/// Errors produced by [`Store`](crate::prelude::Store) key management.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StoreError {
    /// A call to [`Store::insert`](crate::prelude::Store::insert) or a
    /// `merge` collided with an existing entry under the same name.
    #[error("An entry with the name '{0}' already exists in the store.")]
    EntryAlreadyExists(String),
    /// A call to [`Store::get`](crate::prelude::Store::get) did not find
    /// the requested name.
    #[error("No entry found in the store with the name '{0}'.")]
    EntryNotFound(String),
}

/// Errors produced while narrowing or traversing a
/// [`StoreEntry`](crate::prelude::StoreEntry) or
/// [`Value`](crate::prelude::Value).
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum AccessError {
    /// A nested lookup could not find the given key.
    #[error("Entry '{0}' not found")]
    NotFound(String),

    /// A map lookup failed — reserved for contexts where a bare key is
    /// the most useful piece of information.
    #[error("Key '{0}' not found")]
    KeyNotFound(String),

    /// An array index was out of range.
    #[error("Index {0} out of bounds")]
    IndexOutOfBounds(usize),

    /// A `var` accessor was called on an array or map entry. The inner
    /// `&'static str` is the name of the actual variant for diagnostics.
    #[error("Expected var, found {0}")]
    NotAVar(&'static str),

    /// An `array` accessor was called on a var or map entry.
    #[error("Expected array, found {0}")]
    NotAnArray(&'static str),

    /// A `map` accessor was called on a var or array entry.
    #[error("Expected map, found {0}")]
    NotAMap(&'static str),

    /// A typed [`Value`](crate::prelude::Value) accessor (`as_text`,
    /// `as_integer`, etc.) was called on the wrong variant.
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        /// The type name the caller asked for.
        expected: &'static str,
        /// The type name the value actually had.
        found: &'static str,
    },
}

/// Builds an [`OperationError::Custom`] from format arguments, tagged
/// with the operation name taken from a [`Context`](crate::extend::Context).
///
/// Thin wrapper over `Context::error` that lets custom operations use
/// `format!`-style syntax without the `format!` call. The first
/// argument is the context (`$ctx`); the rest are passed verbatim to
/// `format!`.
///
/// ```no_run
/// use panopticon_core::extend::*;
///
/// fn execute(ctx: &mut Context) -> Result<(), OperationError> {
///     let key: i64 = 42;
///     Err(op_error!(ctx, "key {} out of range", key))
/// }
/// ```
#[macro_export]
macro_rules! op_error {
    ($ctx:expr, $($arg:tt)*) => {
        $ctx.error(format!($($arg)*))
    };
}
