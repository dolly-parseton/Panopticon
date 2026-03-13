/*
    Error types for various stages and components of the library.
*/
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DraftError {
    #[error("A variable named '{name}' already exists in the pipeline")]
    DuplicateVariable { name: String },

    #[error("A step named '{name}' already exists in the pipeline")]
    DuplicateStep { name: String },

    #[error("A return named '{name}' already exists in the pipeline")]
    DuplicateReturn { name: String },

    #[error(
        "Step '{step}' references '{reference}' which is not available at that point in the pipeline"
    )]
    UnresolvedReference { step: String, reference: String },

    #[error(
        "Return '{returns}' references '{reference}' which is not available at the end of the pipeline"
    )]
    UnresolvedReturn { returns: String, reference: String },

    #[error("Invalid operation metadata for '{operation}': {reason}")]
    InvalidMetadata {
        operation: &'static str,
        reason: String,
    },
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum OperationError {
    #[error("Reference '{reference}' not found in store")]
    ReferenceNotFound { reference: String },

    #[error("Template parts must resolve to scalar values")]
    InvalidTemplatePart,

    #[error("Failed to resolve parameter '{parameter}': {reason}")]
    ParameterResolutionFailed { parameter: String, reason: String },

    #[error("Operation '{operation}' has no declared input '{input}'")]
    UndeclaredInput {
        operation: &'static str,
        input: String,
    },

    #[error("Operation '{operation}' has no declared static output '{output}'")]
    UndeclaredOutput {
        operation: &'static str,
        output: String,
    },

    #[error("Operation '{operation}' has no declared derived output from input '{input}'")]
    UndeclaredDerivedOutput {
        operation: &'static str,
        input: String,
    },

    #[error("Iteration '{iter_name}': source '{source_ref}' not found")]
    IterSourceNotFound {
        iter_name: String,
        source_ref: String,
    },

    #[error("Iteration '{iter_name}': source '{source_ref}' is not {expected}")]
    IterSourceTypeMismatch {
        iter_name: String,
        source_ref: String,
        expected: &'static str,
    },

    #[error("In iteration '{iter_name}' at index {index}: {source}")]
    Iteration {
        iter_name: String,
        index: String,
        depth: usize,
        #[source]
        source: Box<OperationError>,
    },

    #[error("Access error: {0}")]
    AccessError(#[from] AccessError),

    #[error("Store error: {0}")]
    StoreError(#[from] StoreError),

    #[error("No parameters found for '{name}'")]
    MissingParameters { name: String },

    #[error("Hook '{hook}' aborted execution: {reason}")]
    HookAbort { hook: String, reason: String },

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Execution thread panicked")]
    ThreadPanic,

    #[error("Extension '{extension}' not found for operation '{operation}'")]
    ExtensionNotFound {
        operation: String,
        extension: String,
    },

    #[error("Guard '{guard_name}': reference '{reference}' is not a boolean")]
    GuardTypeMismatch {
        guard_name: String,
        reference: String,
    },

    #[error("In guard '{guard_name}': {source}")]
    Guard {
        guard_name: String,
        #[source]
        source: Box<OperationError>,
    },

    #[error("[{operation}] {message}")]
    Custom {
        operation: String,
        message: String,
    },
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StoreError {
    #[error("An entry with the name '{0}' already exists in the store.")]
    EntryAlreadyExists(String),
    #[error("No entry found in the store with the name '{0}'.")]
    EntryNotFound(String),
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum AccessError {
    #[error("Entry '{0}' not found")]
    NotFound(String),

    #[error("Key '{0}' not found")]
    KeyNotFound(String),

    #[error("Index {0} out of bounds")]
    IndexOutOfBounds(usize),

    #[error("Expected var, found {0}")]
    NotAVar(&'static str),

    #[error("Expected array, found {0}")]
    NotAnArray(&'static str),

    #[error("Expected map, found {0}")]
    NotAMap(&'static str),

    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        expected: &'static str,
        found: &'static str,
    },
}

#[macro_export]
macro_rules! op_error {
    ($ctx:expr, $($arg:tt)*) => {
        $ctx.error(format!($($arg)*))
    };
}
