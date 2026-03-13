use crate::imports::*;

pub enum ExecutionNode {
    Step {
        name: String,
        type_id: TypeId,
    },
    IterArray {
        name: String,
        source: IterSource,
        index_binding: String,
        item_binding: String,
        body: Vec<ExecutionNode>,
    },
    IterMap {
        name: String,
        source: IterSource,
        key_binding: String,
        value_binding: String,
        body: Vec<ExecutionNode>,
    },
    Guard {
        name: String,
        source: GuardSource,
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
