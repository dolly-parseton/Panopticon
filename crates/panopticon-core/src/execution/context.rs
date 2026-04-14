/// The current position inside an iteration — either a numeric array
/// index or a borrowed map key.
///
/// Paired with [`IterContext`] in [`HookEvent`](crate::extend::HookEvent)
/// variants so hooks can report or filter on iteration progress. The
/// `Display` impl renders the index for log output.
#[derive(Debug, Clone)]
pub enum IterIndex<'a> {
    /// The current element's zero-based index in an array iteration.
    Array(usize),
    /// The current entry's key in a map iteration.
    Map(&'a str),
}

/// Per-iteration context surfaced to hooks for steps running inside an
/// [`iter_array`](crate::prelude::Pipeline#method.iter_array) or
/// [`iter_map`](crate::prelude::Pipeline#method.iter_map) body.
///
/// Exposes the enclosing iteration's name, the current index or key,
/// and the nesting depth (outermost iteration is `0`). Hooks can use
/// the nesting depth for indentation and the index for filtering or
/// profiling per element.
#[derive(Debug, Clone)]
pub struct IterContext<'a> {
    /// The name of the enclosing iteration node.
    pub iter_name: &'a str,
    /// The current iteration's index or key.
    pub index: IterIndex<'a>,
    /// Zero-based depth: `0` for the outermost iteration, `1` for one
    /// level of nesting, and so on.
    pub depth: usize,
}

impl std::fmt::Display for IterIndex<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IterIndex::Array(i) => write!(f, "{}", i),
            IterIndex::Map(k) => write!(f, "{}", k),
        }
    }
}

impl std::fmt::Display for IterContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]", self.iter_name, self.index)
    }
}
