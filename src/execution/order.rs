use crate::execution::namespace::{Namespace, NamespaceType};
use crate::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct ExecutionGroup<'a> {
    pub namespace: &'a Namespace,
    pub namespace_index: usize,
    pub commands: Vec<&'a CommandSpec>,
}

pub struct ExecutionPlan<'a> {
    namespaces: &'a [Namespace],
    commands: &'a [CommandSpec],
    namespace_order: Vec<usize>,
    current: usize,
}

impl<'a> ExecutionPlan<'a> {
    #[tracing::instrument(skip(namespaces, commands))]
    pub fn new(namespaces: &'a [Namespace], commands: &'a [CommandSpec]) -> Result<Self> {
        tracing::debug!(
            namespace_count = namespaces.len(),
            command_count = commands.len(),
            "Computing execution plan"
        );

        let namespace_order = compute_namespace_order(namespaces)?;

        tracing::debug!(?namespace_order, "Computed namespace execution order");

        Ok(ExecutionPlan {
            namespaces,
            commands,
            namespace_order,
            current: 0,
        })
    }

    fn get_ordered_commands_for_namespace(&self, ns_idx: usize) -> Result<Vec<&'a CommandSpec>> {
        let namespace = &self.namespaces[ns_idx];

        // Filter commands belonging to this namespace
        let ns_commands: Vec<&CommandSpec> = self
            .commands
            .iter()
            .filter(|c| c.namespace_index == ns_idx)
            .collect();

        if ns_commands.is_empty() {
            return Ok(vec![]);
        }

        let order = compute_command_order(&ns_commands, &namespace.name)?;

        Ok(order.into_iter().map(|i| ns_commands[i]).collect())
    }
}

impl<'a> Iterator for ExecutionPlan<'a> {
    type Item = Result<ExecutionGroup<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.namespace_order.len() {
            return None;
        }

        let ns_idx = self.namespace_order[self.current];
        self.current += 1;

        let namespace = &self.namespaces[ns_idx];

        match self.get_ordered_commands_for_namespace(ns_idx) {
            Ok(commands) => {
                tracing::debug!(
                    namespace = namespace.name.as_str(),
                    command_count = commands.len(),
                    "Yielding execution group"
                );
                Some(Ok(ExecutionGroup {
                    namespace,
                    namespace_index: ns_idx,
                    commands,
                }))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

fn compute_namespace_order(namespaces: &[Namespace]) -> Result<Vec<usize>> {
    if namespaces.is_empty() {
        return Ok(vec![]);
    }

    let name_to_idx: HashMap<&str, usize> = namespaces
        .iter()
        .enumerate()
        .map(|(i, ns)| (ns.name.as_str(), i))
        .collect();

    let mut graph: HashMap<usize, HashSet<usize>> = HashMap::new();

    for (idx, namespace) in namespaces.iter().enumerate() {
        let mut deps = HashSet::new();

        if let NamespaceType::Iterative { store_path, .. } = &namespace.ty {
            if let Some(namespace) = store_path.namespace()
                && let Some(&source_idx) = name_to_idx.get(namespace.as_str())
            {
                if source_idx != idx {
                    deps.insert(source_idx);
                }
            } else {
                tracing::debug!(
                    namespace = namespace.name.as_str(),
                    store_path = store_path.to_dotted(),
                    "Source namespace not found in namespace list, assuming external input"
                );
            }
        }

        graph.insert(idx, deps);
    }

    topological_sort(&graph, namespaces.len())
        .map_err(|_| anyhow::anyhow!("Circular dependency detected in namespace execution order"))
}

fn compute_command_order(commands: &[&CommandSpec], namespace: &str) -> Result<Vec<usize>> {
    if commands.is_empty() {
        return Ok(vec![]);
    }

    let mut prefix_to_idx: HashMap<StorePath, usize> = HashMap::new();
    for (idx, command) in commands.iter().enumerate() {
        let prefix = StorePath::from_segments([namespace, &command.name]);
        prefix_to_idx.insert(prefix, idx);
    }

    let mut graph: HashMap<usize, HashSet<usize>> = HashMap::new();
    for (idx, command) in commands.iter().enumerate() {
        let path_deps = extract_attribute_dependencies(&command.attributes);
        let mut cmd_deps = HashSet::new();

        for dep_path in &path_deps {
            for (cmd_prefix, &cmd_idx) in &prefix_to_idx {
                if dep_path.starts_with(cmd_prefix) && cmd_idx != idx {
                    cmd_deps.insert(cmd_idx);
                    tracing::trace!(
                        command_idx = idx,
                        depends_on_idx = cmd_idx,
                        dependency_path = %dep_path,
                        command_prefix = %cmd_prefix,
                        "Found command dependency"
                    );
                }
            }
        }
        graph.insert(idx, cmd_deps);
    }

    topological_sort(&graph, commands.len())
        .map_err(|_| anyhow::anyhow!("Circular dependency detected in command execution order"))
}

fn extract_attribute_dependencies(attrs: &Attributes) -> HashSet<StorePath> {
    let mut dependencies = HashSet::new();

    for (_, value) in attrs.iter() {
        extract_value_dependencies(value, &mut dependencies);
    }

    dependencies
}

fn extract_value_dependencies(value: &ScalarValue, deps: &mut HashSet<StorePath>) {
    match value {
        ScalarValue::String(s) => {
            extract_variables(s, deps);
        }
        ScalarValue::Array(arr) => {
            for item in arr {
                extract_value_dependencies(item, deps);
            }
        }
        ScalarValue::Object(obj) => {
            for (_, v) in obj {
                extract_value_dependencies(v, deps);
            }
        }
        _ => {}
    }
}

fn topological_sort(
    graph: &HashMap<usize, HashSet<usize>>,
    node_count: usize,
) -> Result<Vec<usize>> {
    let mut in_degree: HashMap<usize, usize> = (0..node_count).map(|i| (i, 0)).collect();

    for (node, deps) in graph.iter() {
        *in_degree.get_mut(node).unwrap() = deps.len();
    }

    let mut queue: VecDeque<usize> = in_degree
        .iter()
        .filter(|&(_, &degree)| degree == 0)
        .map(|(&node, _)| node)
        .collect();

    let mut result = Vec::new();

    while let Some(node) = queue.pop_front() {
        result.push(node);

        for (dependent, deps) in graph.iter() {
            if deps.contains(&node) {
                let degree = in_degree.get_mut(dependent).unwrap();
                *degree -= 1;

                if *degree == 0 {
                    queue.push_back(*dependent);
                }
            }
        }
    }

    if result.len() != node_count {
        return Err(anyhow::anyhow!("Circular dependency detected"));
    }

    Ok(result)
}
