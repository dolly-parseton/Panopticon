use crate::imports::*;

pub mod order;
pub mod results;
pub mod traits;
pub mod validation;

pub mod completed;
pub mod draft;
pub mod ready;

// We love a genric state machine
pub struct Draft;
pub struct Ready;
pub struct Completed {
    context: ExecutionContext,
}

pub struct Pipeline<T = Draft> {
    pub(crate) services: PipelineServices,
    pub(crate) namespaces: Vec<Namespace>,
    pub(crate) commands: Vec<CommandSpec>,
    state: T,
}

impl Default for Pipeline<Draft> {
    fn default() -> Self {
        Pipeline {
            services: PipelineServices::default(),
            namespaces: Vec::new(),
            commands: Vec::new(),
            state: Draft,
        }
    }
}

impl Pipeline<Draft> {
    pub fn with_services(services: PipelineServices) -> Self {
        Pipeline {
            services,
            namespaces: Vec::new(),
            commands: Vec::new(),
            state: Draft,
        }
    }
}

impl<T> Pipeline<T> {
    // Returns an iterator of namespace and command name pairs
    fn command_ns_pairs_iter(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.commands.iter().map(move |cmd| {
            let ns_name = &self.namespaces[cmd.namespace_index].name();
            (*ns_name, cmd.name.as_str())
        })
    }
}
