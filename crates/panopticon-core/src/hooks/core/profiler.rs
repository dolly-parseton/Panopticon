use crate::imports::*;
use std::io::{self, Write};
use std::time::{Duration, Instant};

/// A built-in observer hook that measures per-step execution time.
///
/// Records a [`Duration`](std::time::Duration) for each step (keyed by
/// step name, with iteration index appended for steps inside an
/// iteration) and, on pipeline completion, writes a sorted timings
/// report to a writer (stderr by default). Retain a clone of
/// [`timings`](Self::timings) before moving the profiler into the
/// pipeline to inspect the data programmatically.
pub struct Profiler {
    name: String,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    timings: Arc<Mutex<HashMap<String, Duration>>>,
}

impl Default for Profiler {
    fn default() -> Self {
        Profiler::new()
    }
}

impl Profiler {
    /// Constructs a profiler with the default name and stderr as the
    /// report writer.
    pub fn new() -> Self {
        Profiler {
            name: "profiler".into(),
            writer: Arc::new(Mutex::new(Box::new(io::stderr()))),
            timings: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Overrides the hook name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Redirects the on-completion report to a user-supplied writer.
    pub fn writer(mut self, writer: impl Write + Send + 'static) -> Self {
        self.writer = Arc::new(Mutex::new(Box::new(writer)));
        self
    }

    /// Returns a cloned handle to the timings map. Retain this before
    /// moving the `Profiler` into [`Pipeline::hook`] so the data can be
    /// inspected programmatically once the pipeline finishes.
    pub fn timings(&self) -> Arc<Mutex<HashMap<String, Duration>>> {
        Arc::clone(&self.timings)
    }
}

impl From<Profiler> for Hook {
    fn from(profiler: Profiler) -> Hook {
        let timings = profiler.timings;
        let writer = profiler.writer;
        let pending: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        Hook::observer(profiler.name, move |event, _store| {
            // Build a timing key that includes iteration index when present
            let timing_key = |step_name: &str, iter_context: &Option<&IterContext>| -> String {
                match iter_context {
                    Some(ctx) => format!("{}[{}]", step_name, ctx.index),
                    None => step_name.to_string(),
                }
            };

            #[allow(unreachable_patterns)]
            match event {
                HookEvent::BeforeStep {
                    step_name,
                    iter_context,
                    ..
                } => {
                    let key = timing_key(step_name, iter_context);
                    let mut p = pending.lock().unwrap();
                    p.insert(key, Instant::now());
                }
                HookEvent::AfterStep {
                    step_name,
                    iter_context,
                    ..
                } => {
                    let key = timing_key(step_name, iter_context);
                    let mut p = pending.lock().unwrap();
                    if let Some(start) = p.remove(&key) {
                        let elapsed = start.elapsed();
                        let mut t = timings.lock().unwrap();
                        t.insert(key, elapsed);
                    }
                }
                HookEvent::Complete => {
                    let t = timings.lock().unwrap();
                    let mut w = writer.lock().unwrap();
                    let _ = writeln!(w, "[profiler] Step timings:");
                    let mut entries: Vec<_> = t.iter().collect();
                    entries.sort_by_key(|(name, _)| (*name).clone());
                    for (name, duration) in entries {
                        let _ = writeln!(w, "  {}: {:.3}ms", name, duration.as_secs_f64() * 1000.0);
                    }
                }
                _ => {}
            }
        })
    }
}
