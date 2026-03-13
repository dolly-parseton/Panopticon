mod complete;
mod draft;
mod ready;
mod running;

use crate::imports::*;

pub struct Draft {
    pub(crate) variables: Store<StoreEntry>,  // Defined by 'set'
    pub(crate) parameters: Store<Parameters>, // Defined by 'step'
    pub(crate) returns: Store<Parameters>,    // Defined by 'returns'
    pub(crate) execution_order: Vec<ExecutionNode>, // The order of commands to execute, defined by 'step' and control flow methods.
    pub(crate) registry: Registry, // A registry of available operations, used to validate steps and control flow constructs.
    pub(crate) hooks: Vec<Hook>,
    pub(crate) extensions: Extensions,
}
pub struct Ready {
    pub(crate) variables: Store<StoreEntry>,  // Defined by 'set'
    pub(crate) parameters: Store<Parameters>, // Defined by 'step'
    pub(crate) returns: Store<Parameters>,    // Defined by 'returns'
    pub(crate) execution_order: Vec<ExecutionNode>, // The order of commands to execute, defined by 'step' and control flow methods.
    pub(crate) registry: Registry, // A registry of available operations, used to validate steps and control flow constructs.
    pub(crate) hooks: Vec<Hook>,
    pub(crate) extensions: Extensions,
}

pub struct Running {
    pub(crate) handle: thread::JoinHandle<()>,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) result: Arc<Mutex<Option<Result<Complete, OperationError>>>>,
}

pub enum RunningStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

// TODO - Consider adding 'metadata' logging capture here for durations etc.
pub struct Complete {
    variables: Store<StoreEntry>, // Defined by 'set'
    returns: Store<StoreEntry>,   // Defined by 'returns'
}

pub struct Pipeline<S = Draft> {
    state: S, // The state of the pipeline, dictates which operations are allowed to be performed on it.
}

impl Default for Pipeline {
    fn default() -> Self {
        Pipeline::new()
    }
}
