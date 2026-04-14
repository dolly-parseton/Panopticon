use crate::imports::*;

/// One node in a compiled pipeline's execution order.
///
/// `ExecutionNode` is the intermediate representation the runtime walks
/// to execute a pipeline, and the type external tooling (serialisers,
/// alternative drivers, visualisers) inspects via
/// [`Pipeline<Ready>::execution_order`](crate::prelude::Pipeline). The
/// four variants correspond to the four draft-phase construction
/// methods: [`step`](crate::prelude::Pipeline#method.step),
/// [`iter_array`](crate::prelude::Pipeline#method.iter_array),
/// [`iter_map`](crate::prelude::Pipeline#method.iter_map), and
/// [`guard`](crate::prelude::Pipeline#method.guard).
pub enum ExecutionNode {
    /// A single step to execute by looking up its operation by
    /// `type_id` in the [`Registry`](crate::extend::Registry).
    Step {
        /// The step's prefixed name (including any enclosing iteration
        /// or guard scopes).
        name: String,
        /// The `TypeId` of the operation to run.
        type_id: TypeId,
    },
    /// An iteration over an array source. The body executes once per
    /// element with `__index` and `__item` bindings injected into the
    /// cloned store.
    IterArray {
        /// The iteration node's prefixed name.
        name: String,
        /// The draft-time reference to the array to walk.
        source: IterSource,
        /// The fully qualified store path for the current index
        /// binding.
        index_binding: String,
        /// The fully qualified store path for the current item
        /// binding.
        item_binding: String,
        /// The nodes to execute once per iteration.
        body: Vec<ExecutionNode>,
    },
    /// An iteration over a map source. The body executes once per
    /// entry with `__key` and `__value` bindings injected into the
    /// cloned store.
    IterMap {
        /// The iteration node's prefixed name.
        name: String,
        /// The draft-time reference to the map to walk.
        source: IterSource,
        /// The fully qualified store path for the current key binding.
        key_binding: String,
        /// The fully qualified store path for the current value
        /// binding.
        value_binding: String,
        /// The nodes to execute once per iteration.
        body: Vec<ExecutionNode>,
    },
    /// A conditional node whose body executes only when the boolean
    /// referenced by `source` is true at runtime. Unlike iterations,
    /// a guard body shares the parent scope.
    Guard {
        /// The guard node's prefixed name.
        name: String,
        /// The draft-time reference to the boolean entry that gates
        /// the body.
        source: GuardSource,
        /// The nodes to execute when the guard passes.
        body: Vec<ExecutionNode>,
    },
}

pub(crate) fn prefix_node_names(prefix: &str, nodes: Vec<ExecutionNode>) -> Vec<ExecutionNode> {
    nodes
        .into_iter()
        .map(|node| match node {
            ExecutionNode::Step { name, type_id } => ExecutionNode::Step {
                name: format!("{}.{}", prefix, name),
                type_id,
            },
            ExecutionNode::IterArray {
                name,
                source,
                index_binding: _,
                item_binding: _,
                body,
            } => {
                let prefixed = format!("{}.{}", prefix, name);
                ExecutionNode::IterArray {
                    index_binding: format!("{}.{}", prefixed, ITER_INDEX),
                    item_binding: format!("{}.{}", prefixed, ITER_ITEM),
                    body: prefix_node_names(&prefixed, body),
                    name: prefixed,
                    source,
                }
            }
            ExecutionNode::IterMap {
                name,
                source,
                key_binding: _,
                value_binding: _,
                body,
            } => {
                let prefixed = format!("{}.{}", prefix, name);
                ExecutionNode::IterMap {
                    key_binding: format!("{}.{}", prefixed, ITER_KEY),
                    value_binding: format!("{}.{}", prefixed, ITER_VALUE),
                    body: prefix_node_names(&prefixed, body),
                    name: prefixed,
                    source,
                }
            }
            ExecutionNode::Guard {
                name,
                source,
                body,
            } => {
                let prefixed = format!("{}.{}", prefix, name);
                ExecutionNode::Guard {
                    // Body nodes are already prefixed with the guard name from guard() in draft.rs,
                    // so we only add the parent prefix, not the full prefixed guard name.
                    body: prefix_node_names(prefix, body),
                    name: prefixed,
                    source,
                }
            }
        })
        .collect()
}
