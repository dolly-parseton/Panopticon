mod event_log;
mod profiler;
mod step_filter;
mod store_validator;
mod timeout;

pub use event_log::{EventKind, EventLog, EventRecord};
pub use profiler::Profiler;
pub use step_filter::StepFilter;
pub use store_validator::StoreValidator;
pub use timeout::Timeout;

use crate::imports::*;
use std::io::{self, Write};

#[derive(Clone)]
pub struct Logger {
    name: String,
    prefix: String,
    indent: String,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl Default for Logger {
    fn default() -> Self {
        Logger::new()
    }
}

impl Logger {
    pub fn new() -> Self {
        Logger {
            name: "logger".into(),
            prefix: "[hook]".into(),
            indent: "  ".into(),
            writer: Arc::new(Mutex::new(Box::new(io::stderr()))),
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    pub fn indent(mut self, spaces: usize) -> Self {
        self.indent = " ".repeat(spaces);
        self
    }

    pub fn writer(mut self, writer: impl Write + Send + 'static) -> Self {
        self.writer = Arc::new(Mutex::new(Box::new(writer)));
        self
    }
}

impl From<Logger> for Hook {
    fn from(logger: Logger) -> Hook {
        let prefix: Arc<str> = Arc::from(logger.prefix.as_str());
        let indent: Arc<str> = Arc::from(logger.indent.as_str());
        let writer = logger.writer;

        Hook::observer(logger.name, move |event, _store| {
            let prefix = &prefix;
            let indent = &indent;
            let mut w = writer.lock().unwrap();

            // Extra indentation per nesting depth when inside an iteration
            let depth_indent = |ctx: &Option<&IterContext>| -> String {
                match ctx {
                    Some(c) => "  ".repeat(c.depth + 1),
                    None => String::new(),
                }
            };
            let iter_tag = |ctx: &Option<&IterContext>| -> String {
                match ctx {
                    Some(c) => format!(" [{}]", c),
                    None => String::new(),
                }
            };

            #[allow(unreachable_patterns)]
            match event {
                HookEvent::BeforeIteration { iter_context } => {
                    let di = "  ".repeat(iter_context.depth);
                    let _ = writeln!(w, "{}{}{} >> iter '{}'", indent, di, prefix, iter_context,);
                }
                HookEvent::AfterIteration { iter_context } => {
                    let di = "  ".repeat(iter_context.depth);
                    let _ = writeln!(w, "{}{}{} << iter '{}'", indent, di, prefix, iter_context,);
                }
                HookEvent::BeforeStep {
                    step_name,
                    metadata,
                    iter_context,
                    ..
                } => {
                    let di = depth_indent(iter_context);
                    let tag = iter_tag(iter_context);
                    let _ = writeln!(
                        w,
                        "{}{}{} -> BeforeStep '{}' (op: {}){}",
                        indent, di, prefix, step_name, metadata.name, tag,
                    );
                }
                HookEvent::AfterStep {
                    step_name,
                    operation_outputs,
                    global_outputs,
                    iter_context,
                    ..
                } => {
                    let di = depth_indent(iter_context);
                    let tag = iter_tag(iter_context);
                    let op_keys: Vec<&String> = operation_outputs.keys().collect();
                    let global_keys: Vec<&String> = global_outputs.keys().collect();
                    let _ = writeln!(
                        w,
                        "{}{}{} <- AfterStep '{}' | op outputs: {:?}, global outputs: {:?}{}",
                        indent, di, prefix, step_name, op_keys, global_keys, tag,
                    );
                }
                HookEvent::BeforeReturns { return_name, .. } => {
                    let _ = writeln!(w, "{}{} -> BeforeReturns '{}'", indent, prefix, return_name,);
                }
                HookEvent::AfterReturns {
                    return_name,
                    outputs,
                    ..
                } => {
                    let keys: Vec<&String> = outputs.keys().collect();
                    let _ = writeln!(
                        w,
                        "{}{} <- AfterReturns '{}' | resolved: {:?}",
                        indent, prefix, return_name, keys,
                    );
                }
                HookEvent::GuardPassed { guard_name } => {
                    let _ = writeln!(w, "{}{} >> guard '{}' passed", indent, prefix, guard_name,);
                }
                HookEvent::GuardFailed { guard_name } => {
                    let _ = writeln!(w, "{}{} xx guard '{}' failed", indent, prefix, guard_name,);
                }
                HookEvent::Complete => {
                    let _ = writeln!(w, "{}{} == Complete", indent, prefix);
                }
                HookEvent::Error { error } => {
                    let _ = writeln!(w, "{}{} !! Error: {}", indent, prefix, error);
                }
                _ => {}
            }
        })
    }
}
