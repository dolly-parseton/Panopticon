//! Operation name resolution table and the `register_ops!` macro.
//!
//! YAML step nodes refer to operations by string name (`op: SetVar`). At
//! load time the schema crate resolves each name via an
//! [`OperationRegistrationMap`] — a table of non-capturing closures ("step
//! thunks") that call core's generic `Pipeline::step::<O>()` with a
//! statically-known operation type.
//!
//! The normal way to build one of these maps is with the
//! the `register_ops!` macro. Direct calls to [`OperationRegistrationMap::insert`]
//! are supported for advanced use cases (e.g. plugin-style registration of
//! ops loaded from another crate at startup).

use std::any::TypeId;
use std::collections::HashMap;

use panopticon_core::extend::{Draft, DraftError, Operation, Parameters, Pipeline};

/// A fn-pointer thunk that registers an operation type and appends a step
/// to a draft pipeline.
///
/// Each thunk is built by the `register_ops!` macro for a specific concrete
/// `O: Operation + 'static`, so core's generic `step::<O>()` is called
/// with the real type and the correct `TypeId` ends up in the execution
/// node. The closure that the `register_ops!` macro emits captures nothing, so it
/// coerces to this fn-pointer type at zero cost.
pub type StepThunk =
    fn(&mut Pipeline<Draft>, String, Parameters) -> Result<(), DraftError>;

/// Operation name ↔ type table.
///
/// Holds two parallel lookups:
///
/// - `thunks_by_name`: operation-name → [`StepThunk`], used by
///   [`crate::Deserialiser`] when lowering YAML into a draft.
/// - `names_by_type_id`: `TypeId` → operation-name, used by
///   [`crate::Serialiser`] when lifting a ready pipeline back into
///   schema types.
///
/// Both maps are populated simultaneously by
/// [`OperationRegistrationMap::register`] and by the `register_ops!`
/// macro, so ops registered through those paths are automatically
/// round-trippable.
#[derive(Default, Clone)]
pub struct OperationRegistrationMap {
    thunks_by_name: HashMap<String, StepThunk>,
    names_by_type_id: HashMap<TypeId, String>,
}

impl OperationRegistrationMap {
    /// Create an empty map. Prefer the `register_ops!` macro when the set
    /// of ops is known at compile time.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an operation type `O` under `name`. Populates both the
    /// forward (name → thunk) and reverse (`TypeId` → name) tables.
    ///
    /// This is the preferred registration entry point: ops registered
    /// this way participate in both lowering (YAML → draft) and lifting
    /// (ready → YAML). Prefer the `register_ops!` macro when the op set
    /// is known at compile time; use this method for programmatic
    /// registration.
    pub fn register<O: Operation + 'static>(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.thunks_by_name
            .insert(name.clone(), |pipeline, step_name, params| {
                pipeline.step::<O>(step_name, params)
            });
        self.names_by_type_id.insert(TypeId::of::<O>(), name);
    }

    /// Register a raw thunk under `name` without recording a `TypeId`.
    ///
    /// Ops registered this way are **forward-only** — they can be used to
    /// lower YAML into a draft but cannot be serialised back, because no
    /// reverse lookup from `TypeId` to name exists for them. Prefer
    /// [`OperationRegistrationMap::register`] or the `register_ops!`
    /// macro for full round-trip support; use this method only when you
    /// have a pre-built thunk and explicitly do not need serialisation.
    pub fn insert(&mut self, name: impl Into<String>, thunk: StepThunk) {
        self.thunks_by_name.insert(name.into(), thunk);
    }

    /// Look up the thunk registered under `name`, if any.
    pub fn get(&self, name: &str) -> Option<StepThunk> {
        self.thunks_by_name.get(name).copied()
    }

    /// Look up the operation name registered for a given `TypeId`, if
    /// any. Used by [`crate::Serialiser`] to render
    /// `ExecutionNode::Step { type_id, .. }` back into an op-name string
    /// for the YAML output.
    pub fn name_for_type_id(&self, type_id: TypeId) -> Option<&str> {
        self.names_by_type_id.get(&type_id).map(String::as_str)
    }

    /// True if `name` has a thunk registered.
    pub fn contains(&self, name: &str) -> bool {
        self.thunks_by_name.contains_key(name)
    }

    /// Number of registered thunks.
    pub fn len(&self) -> usize {
        self.thunks_by_name.len()
    }

    /// True if no thunks are registered.
    pub fn is_empty(&self) -> bool {
        self.thunks_by_name.is_empty()
    }
}

/// Build an [`OperationRegistrationMap`] from an iterator of
/// `(name, thunk)` pairs. Handy when the op set is data-driven (e.g.
/// collected from a plugin registry) rather than known at compile time.
///
/// Later entries with the same name overwrite earlier ones.
///
/// ```no_run
/// use panopticon_schema::{OperationRegistrationMap, StepThunk};
/// # fn example(set_var_thunk: StepThunk, get_thunk: StepThunk) {
/// let ops: OperationRegistrationMap = [
///     ("SetVar", set_var_thunk),
///     ("Get", get_thunk),
/// ]
/// .into_iter()
/// .collect();
/// assert!(ops.contains("SetVar"));
/// # }
/// ```
impl<S: Into<String>> FromIterator<(S, StepThunk)> for OperationRegistrationMap {
    fn from_iter<I: IntoIterator<Item = (S, StepThunk)>>(iter: I) -> Self {
        // FromIterator takes raw thunks, so it follows `insert` semantics:
        // no reverse `TypeId` entries are recorded. Ops constructed this way
        // participate in lowering only, not in lifting.
        let thunks_by_name = iter
            .into_iter()
            .map(|(name, thunk)| (name.into(), thunk))
            .collect();
        Self {
            thunks_by_name,
            names_by_type_id: HashMap::new(),
        }
    }
}

/// Merge `(name, thunk)` pairs into an existing
/// [`OperationRegistrationMap`]. Later entries overwrite earlier ones,
/// matching [`FromIterator`] semantics and the behaviour of
/// [`OperationRegistrationMap::insert`].
impl<S: Into<String>> Extend<(S, StepThunk)> for OperationRegistrationMap {
    fn extend<I: IntoIterator<Item = (S, StepThunk)>>(&mut self, iter: I) {
        for (name, thunk) in iter {
            self.thunks_by_name.insert(name.into(), thunk);
        }
    }
}

/// Build an [`OperationRegistrationMap`] from a list of
/// `"name" => OpType` pairs.
///
/// Each entry expands to a non-capturing closure that calls
/// `pipeline.step::<$op>(name, params)`. The closure coerces to the
/// [`StepThunk`] fn-pointer type because it captures nothing; `$op` must
/// satisfy `panopticon_core::extend::Operation + 'static` (enforced by the
/// `step::<O>()` method's own bounds at macro expansion time).
///
/// # Example
///
/// ```no_run
/// use panopticon_core::extend::SetVar;
/// use panopticon_schema::register_ops;
///
/// let ops = register_ops!(
///     "SetVar" => SetVar,
/// );
/// assert!(ops.contains("SetVar"));
/// ```
#[macro_export]
macro_rules! register_ops {
    ( $( $name:literal => $op:ty ),* $(,)? ) => {{
        #[allow(unused_mut)]
        let mut map = $crate::OperationRegistrationMap::new();
        $(
            map.register::<$op>($name);
        )*
        map
    }};
}

#[cfg(test)]
mod tests {
    use panopticon_core::extend::{Param, Parameters, Pipeline, SetVar};
    use std::any::TypeId;
    use std::collections::HashMap;

    #[test]
    fn register_ops_builds_usable_thunks() {
        let ops = crate::register_ops!(
            "SetVar" => SetVar,
        );

        assert!(ops.contains("SetVar"));
        assert_eq!(ops.len(), 1);

        let thunk = ops.get("SetVar").expect("SetVar thunk registered");

        let mut pipe = Pipeline::new();
        pipe.var("source", "hello").unwrap();

        let mut params_map = HashMap::new();
        params_map.insert("name".to_string(), Param::literal("greeting"));
        params_map.insert("value".to_string(), Param::reference("source"));
        let params = Parameters::new(params_map);

        thunk(&mut pipe, "set_greeting".to_string(), params)
            .expect("thunk should register the step and push an execution node");

        // Compiling validates that the thunk really pushed a well-formed step
        // into the draft's execution_order with the correct TypeId.
        pipe.compile().expect("compile should succeed");
    }

    #[test]
    fn register_ops_empty_is_valid() {
        let ops = crate::register_ops!();
        assert!(ops.is_empty());
        assert!(ops.get("anything").is_none());
    }

    #[test]
    fn from_iterator_builds_map() {
        // Pull a thunk out of register_ops! so we don't have to name the
        // fn-pointer type by hand.
        let source = crate::register_ops!(
            "SetVar" => SetVar,
        );
        let set_var = source.get("SetVar").unwrap();

        let ops: super::OperationRegistrationMap = [
            ("alpha", set_var),
            ("beta", set_var),
        ]
        .into_iter()
        .collect();

        assert_eq!(ops.len(), 2);
        assert!(ops.contains("alpha"));
        assert!(ops.contains("beta"));
    }

    #[test]
    fn register_populates_reverse_type_id_map() {
        let mut ops = super::OperationRegistrationMap::new();
        ops.register::<SetVar>("SetVar");

        assert!(ops.contains("SetVar"));
        assert_eq!(
            ops.name_for_type_id(TypeId::of::<SetVar>()),
            Some("SetVar")
        );
    }

    #[test]
    fn register_ops_macro_populates_reverse_map() {
        let ops = crate::register_ops!(
            "SetVar" => SetVar,
        );
        assert_eq!(
            ops.name_for_type_id(TypeId::of::<SetVar>()),
            Some("SetVar")
        );
    }

    #[test]
    fn insert_does_not_populate_reverse_map() {
        // `insert` is the forward-only escape hatch: ops added this way
        // are not serialisable because no reverse TypeId entry exists.
        let source = crate::register_ops!(
            "SetVar" => SetVar,
        );
        let thunk = source.get("SetVar").unwrap();

        let mut ops = super::OperationRegistrationMap::new();
        ops.insert("SetVar", thunk);

        assert!(ops.contains("SetVar"));
        assert_eq!(ops.name_for_type_id(TypeId::of::<SetVar>()), None);
    }

    #[test]
    fn extend_overwrites_existing_entries() {
        let source = crate::register_ops!(
            "SetVar" => SetVar,
        );
        let set_var = source.get("SetVar").unwrap();

        let mut ops = super::OperationRegistrationMap::new();
        ops.insert("foo", set_var);
        assert_eq!(ops.len(), 1);

        ops.extend([("bar", set_var), ("foo", set_var)]);
        assert_eq!(ops.len(), 2);
        assert!(ops.contains("foo"));
        assert!(ops.contains("bar"));
    }
}
