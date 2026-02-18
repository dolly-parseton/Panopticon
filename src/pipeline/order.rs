use crate::imports::*;
use std::cmp::Reverse;

pub struct ExecutionGroup<'a> {
    pub(crate) namespace: &'a Namespace,
    pub(crate) namespace_index: usize,
    pub(crate) commands: Vec<&'a CommandSpec>,
}

pub struct ExecutionPlan<'a> {
    namespaces: &'a [Namespace],
    commands: &'a [CommandSpec],
    namespace_order: Vec<usize>,
    current: usize,
}

impl<'a> ExecutionPlan<'a> {
    pub fn new(namespaces: &'a [Namespace], commands: &'a [CommandSpec]) -> Result<Self> {
        let namespace_order = compute_namespace_order(namespaces, commands)?;
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

        let order = compute_command_order(&ns_commands, namespace.name())?;

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
                    namespace = namespace.name(),
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

fn compute_namespace_order(
    namespaces: &[Namespace],
    commands: &[CommandSpec],
) -> Result<Vec<usize>> {
    if namespaces.is_empty() {
        return Ok(vec![]);
    }

    let name_to_idx: HashMap<&str, usize> = namespaces
        .iter()
        .enumerate()
        .map(|(i, ns)| (ns.name(), i))
        .collect();

    let mut graph: HashMap<usize, HashSet<usize>> = HashMap::new();

    // Initialize all namespaces in the graph
    for idx in 0..namespaces.len() {
        graph.insert(idx, HashSet::new());
    }

    // Add dependencies from iterative namespaces
    for (idx, namespace) in namespaces.iter().enumerate() {
        if let ExecutionMode::Iterative { store_path, .. } = &namespace.ty() {
            if let Some(ns_name) = store_path.namespace()
                && let Some(&source_idx) = name_to_idx.get(ns_name.as_str())
            {
                if source_idx != idx {
                    graph.get_mut(&idx).unwrap().insert(source_idx);
                }
            } else {
                tracing::debug!(
                    namespace = namespace.name(),
                    store_path = store_path.to_dotted(),
                    "Source namespace not found in namespace list, assuming external input"
                );
            }
        }
    }

    // Add dependencies from command dependencies (cross-namespace references)
    for command in commands {
        let command_ns_idx = command.namespace_index;

        for dep_path in &command.dependencies {
            if let Some(dep_ns_name) = dep_path.namespace()
                && let Some(&dep_ns_idx) = name_to_idx.get(dep_ns_name.as_str())
                && dep_ns_idx != command_ns_idx
            {
                graph.get_mut(&command_ns_idx).unwrap().insert(dep_ns_idx);
            }
        }
    }

    // Add dependencies from extension provides/requires relationships
    let mut extension_providers: HashMap<ExtensionKey, usize> = HashMap::new();
    for command in commands {
        for ext_key in &command.provides_extensions {
            extension_providers
                .entry(ext_key.clone())
                .or_insert(command.namespace_index);
        }
    }

    for command in commands {
        let requiring_ns_idx = command.namespace_index;
        for ext_key in &command.requires_extensions {
            if let Some(&provider_ns_idx) = extension_providers.get(ext_key) {
                if provider_ns_idx != requiring_ns_idx {
                    graph
                        .get_mut(&requiring_ns_idx)
                        .unwrap()
                        .insert(provider_ns_idx);
                }
            }
        }
    }

    // Compute priority: namespaces with extension providers sort first among peers
    let mut priority: HashMap<usize, u32> = HashMap::new();
    for command in commands {
        if !command.provides_extensions.is_empty() {
            let entry = priority.entry(command.namespace_index).or_insert(0);
            *entry = (*entry).max(1);
        }
    }

    topological_sort_with_priority(&graph, namespaces.len(), &priority)
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
        let mut cmd_deps = HashSet::new();

        for dep_path in &command.dependencies {
            for (cmd_prefix, &cmd_idx) in &prefix_to_idx {
                if dep_path.starts_with(cmd_prefix) && cmd_idx != idx {
                    cmd_deps.insert(cmd_idx);
                }
            }
        }
        graph.insert(idx, cmd_deps);
    }

    // Add extension-based edges within the namespace
    let mut ext_provider_idx: HashMap<&ExtensionKey, usize> = HashMap::new();
    for (idx, command) in commands.iter().enumerate() {
        for ext_key in &command.provides_extensions {
            ext_provider_idx.insert(ext_key, idx);
        }
    }

    for (idx, command) in commands.iter().enumerate() {
        for ext_key in &command.requires_extensions {
            if let Some(&provider_idx) = ext_provider_idx.get(ext_key) {
                if provider_idx != idx {
                    graph.get_mut(&idx).unwrap().insert(provider_idx);
                }
            }
        }
    }

    topological_sort(&graph, commands.len())
        .map_err(|_| anyhow::anyhow!("Circular dependency detected in command execution order"))
}

fn topological_sort_with_priority(
    graph: &HashMap<usize, HashSet<usize>>,
    node_count: usize,
    priority: &HashMap<usize, u32>,
) -> Result<Vec<usize>> {
    let mut in_degree: HashMap<usize, usize> = (0..node_count).map(|i| (i, 0)).collect();

    for (node, deps) in graph.iter() {
        *in_degree.get_mut(node).unwrap() = deps.len();
    }

    let mut heap: BinaryHeap<(u32, Reverse<usize>)> = in_degree
        .iter()
        .filter(|&(_, &degree)| degree == 0)
        .map(|(&node, _)| (*priority.get(&node).unwrap_or(&0), Reverse(node)))
        .collect();

    let mut result = Vec::new();

    while let Some((_, Reverse(node))) = heap.pop() {
        result.push(node);

        for (dependent, deps) in graph.iter() {
            if deps.contains(&node) {
                let degree = in_degree.get_mut(dependent).unwrap();
                *degree -= 1;

                if *degree == 0 {
                    heap.push((*priority.get(dependent).unwrap_or(&0), Reverse(*dependent)));
                }
            }
        }
    }

    if result.len() != node_count {
        return Err(anyhow::anyhow!("Circular dependency detected"));
    }

    Ok(result)
}

fn topological_sort(
    graph: &HashMap<usize, HashSet<usize>>,
    node_count: usize,
) -> Result<Vec<usize>> {
    topological_sort_with_priority(graph, node_count, &HashMap::new())
}
