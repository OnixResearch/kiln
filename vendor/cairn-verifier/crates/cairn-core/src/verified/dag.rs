#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopologicalOrder {
    pub(crate) order: Vec<String>,
    pub(crate) blocked_nodes: Vec<String>,
}

pub(crate) fn topological_order(
    node_ids: &[String],
    dependency_edges: &[(String, String)],
) -> TopologicalOrder {
    // r[impl cairn.change_dependency_graph.trellis_primitives]
    // r[impl cairn.task_ordering.graph]
    let mut indegrees = node_ids
        .iter()
        .map(|node_id| (node_id.clone(), 0usize))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut dependents = node_ids
        .iter()
        .map(|node_id| (node_id.clone(), Vec::new()))
        .collect::<std::collections::BTreeMap<_, _>>();

    for (dependency, dependent) in dependency_edges {
        if let Some(indegree) = indegrees.get_mut(dependent) {
            *indegree += 1;
        }
        if let Some(dependent_list) = dependents.get_mut(dependency) {
            dependent_list.push(dependent.clone());
        }
    }
    for dependent_list in dependents.values_mut() {
        dependent_list.sort();
        dependent_list.dedup();
    }

    let mut ready = indegrees
        .iter()
        .filter(|(_node_id, indegree)| **indegree == 0)
        .map(|(node_id, _indegree)| node_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut order = Vec::with_capacity(node_ids.len());

    while let Some(node_id) = ready.pop_first() {
        order.push(node_id.clone());
        if let Some(dependent_list) = dependents.get(&node_id) {
            for dependent in dependent_list {
                if let Some(indegree) = indegrees.get_mut(dependent) {
                    if *indegree > 0 {
                        *indegree -= 1;
                    }
                    if *indegree == 0 {
                        ready.insert(dependent.clone());
                    }
                }
            }
        }
    }

    let ordered = order
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let blocked_nodes = node_ids
        .iter()
        .filter(|node_id| !ordered.contains(*node_id))
        .cloned()
        .collect();

    TopologicalOrder {
        order,
        blocked_nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_dependencies_before_dependents_with_stable_ties() {
        let nodes = ["build", "docs", "test"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let edges = vec![("build".to_string(), "test".to_string())];

        let result = topological_order(&nodes, &edges);

        assert_eq!(result.order, vec!["build", "docs", "test"]);
        assert!(result.blocked_nodes.is_empty());
    }

    #[test]
    fn reports_nodes_blocked_by_a_cycle() {
        let nodes = ["a", "b"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let edges = vec![
            ("a".to_string(), "b".to_string()),
            ("b".to_string(), "a".to_string()),
        ];

        let result = topological_order(&nodes, &edges);

        assert!(result.order.is_empty());
        assert_eq!(result.blocked_nodes, vec!["a", "b"]);
    }
}
