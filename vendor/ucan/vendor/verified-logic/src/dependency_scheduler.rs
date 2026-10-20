//! Dependency scheduler composite facade.
//!
//! This facade composes the verified dependency-DAG, topological-sort,
//! cycle-detection, earliest-deadline-first, fair-share, and slot-allocation
//! primitives. It admits one scheduling decision only when dependencies are
//! ready, the graph is acyclic with a valid topological order, EDF and fair-share
//! select the same runnable index, that job receives a non-zero fair allocation,
//! and a concurrency slot can be acquired.

use vstd::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DependencySchedulerOutcome {
    Scheduled {
        selected: usize,
        allocation: u64,
        new_in_use: u64,
        topo_order: Vec<usize>,
    },
    Blocked,
    Cyclic,
    Full,
    Rejected,
}

verus! {

pub open spec fn dependency_scheduler_edges_spec(edges: Seq<(usize, usize)>) -> Seq<(int, int)> {
    edges.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int))
}

pub open spec fn dependency_scheduler_order_spec(order: Seq<usize>) -> Seq<int> {
    order.map(|_i: int, v: usize| v as int)
}

// r[impl composite-primitives.dependency-scheduler]
pub open spec fn dependency_scheduler_accepted_spec(
    deps: Seq<u64>,
    completed: Seq<u64>,
    edges: Seq<(usize, usize)>,
    n: usize,
    topo_order: Seq<usize>,
    deadlines: Seq<u64>,
    vtimes: Seq<u64>,
    total_tokens: u64,
    weight: u64,
    total_weight: u64,
    in_use: u64,
    capacity: u64,
) -> bool {
    &&& crate::job_dag::all_deps_satisfied_spec(deps, completed)
    &&& crate::job_dag::unsatisfied_count_spec(deps, completed) == 0
    &&& crate::topo_sort::is_valid_graph_spec(dependency_scheduler_edges_spec(edges), n as int)
    &&& !crate::cycle_detect::has_cycle_spec(dependency_scheduler_edges_spec(edges), n as int)
    &&& crate::topo_sort::is_topo_order_spec(
        dependency_scheduler_edges_spec(edges),
        n as int,
        dependency_scheduler_order_spec(topo_order),
    )
    &&& deadlines.len() > 0
    &&& vtimes.len() > 0
    &&& crate::deadline_queue::edf_select_spec(deadlines) == crate::fair_share::select_min_vtime_spec(vtimes)
    &&& total_weight > 0
    &&& weight <= total_weight
    &&& crate::fair_share::fair_alloc_spec(total_tokens, weight, total_weight) > 0
    &&& crate::job_slot::has_available_slot_spec(in_use, capacity)
    &&& crate::job_slot::try_acquire_slot_spec(in_use, capacity).0
}

// r[impl composite-primitives.dependency-scheduler]
pub open spec fn dependency_scheduler_ready_spec(
    deps: Seq<u64>,
    completed: Seq<u64>,
    edges: Seq<(usize, usize)>,
    n: usize,
    topo_order: Seq<usize>,
    deadlines: Seq<u64>,
    vtimes: Seq<u64>,
    total_tokens: u64,
    weight: u64,
    total_weight: u64,
    in_use: u64,
    capacity: u64,
) -> bool {
    dependency_scheduler_accepted_spec(
        deps, completed, edges, n, topo_order, deadlines, vtimes,
        total_tokens, weight, total_weight, in_use, capacity,
    )
}

// r[impl composite-primitives.dependency-scheduler]
pub open spec fn dependency_scheduler_reject_preserves_spec(
    old_in_use: u64,
    new_in_use: u64,
) -> bool {
    old_in_use == new_in_use
}

// r[impl composite-primitives.dependency-scheduler]
pub open spec fn dependency_scheduler_outcome_alias_spec(accepted: bool) -> crate::composite_outcome::CompositeOutcomeTag {
    if accepted {
        crate::composite_outcome::CompositeOutcomeTag::Accept
    } else {
        crate::composite_outcome::CompositeOutcomeTag::Reject
    }
}

// r[verify composite-primitives.dependency-scheduler]
pub proof fn dependency_scheduler_refines_sources(
    deps: Seq<u64>,
    completed: Seq<u64>,
    edges: Seq<(usize, usize)>,
    n: usize,
    topo_order: Seq<usize>,
    deadlines: Seq<u64>,
    vtimes: Seq<u64>,
    total_tokens: u64,
    weight: u64,
    total_weight: u64,
    in_use: u64,
    capacity: u64,
)
    ensures
        dependency_scheduler_ready_spec(
            deps, completed, edges, n, topo_order, deadlines, vtimes,
            total_tokens, weight, total_weight, in_use, capacity,
        ) ==> crate::job_dag::all_deps_satisfied_spec(deps, completed),
        dependency_scheduler_ready_spec(
            deps, completed, edges, n, topo_order, deadlines, vtimes,
            total_tokens, weight, total_weight, in_use, capacity,
        ) ==> crate::job_dag::unsatisfied_count_spec(deps, completed) == 0,
        dependency_scheduler_ready_spec(
            deps, completed, edges, n, topo_order, deadlines, vtimes,
            total_tokens, weight, total_weight, in_use, capacity,
        ) ==> crate::topo_sort::is_topo_order_spec(
            dependency_scheduler_edges_spec(edges),
            n as int,
            dependency_scheduler_order_spec(topo_order),
        ),
        dependency_scheduler_ready_spec(
            deps, completed, edges, n, topo_order, deadlines, vtimes,
            total_tokens, weight, total_weight, in_use, capacity,
        ) ==> !crate::cycle_detect::has_cycle_spec(dependency_scheduler_edges_spec(edges), n as int),
        dependency_scheduler_ready_spec(
            deps, completed, edges, n, topo_order, deadlines, vtimes,
            total_tokens, weight, total_weight, in_use, capacity,
        ) ==> crate::deadline_queue::edf_select_spec(deadlines) == crate::fair_share::select_min_vtime_spec(vtimes),
        dependency_scheduler_ready_spec(
            deps, completed, edges, n, topo_order, deadlines, vtimes,
            total_tokens, weight, total_weight, in_use, capacity,
        ) ==> crate::fair_share::fair_alloc_spec(total_tokens, weight, total_weight) > 0,
        dependency_scheduler_ready_spec(
            deps, completed, edges, n, topo_order, deadlines, vtimes,
            total_tokens, weight, total_weight, in_use, capacity,
        ) ==> crate::job_slot::has_available_slot_spec(in_use, capacity),
        dependency_scheduler_ready_spec(
            deps, completed, edges, n, topo_order, deadlines, vtimes,
            total_tokens, weight, total_weight, in_use, capacity,
        ) ==> crate::job_slot::try_acquire_slot_spec(in_use, capacity).0,
{
}

// r[impl composite-primitives.dependency-scheduler]
pub fn dependency_scheduler_outcome_alias(accepted: bool) -> (outcome: crate::composite_outcome::CompositeOutcomeTag)
    ensures outcome == dependency_scheduler_outcome_alias_spec(accepted)
{
    if accepted {
        crate::composite_outcome::composite_accept()
    } else {
        crate::composite_outcome::composite_reject()
    }
}

} // verus!

// r[impl composite-primitives.dependency-scheduler]
#[allow(clippy::too_many_arguments)]
pub fn dependency_scheduler_schedule(
    deps: &[u64],
    completed: &[u64],
    edges: &[(usize, usize)],
    n: usize,
    deadlines: &[u64],
    vtimes: &[u64],
    total_tokens: u64,
    weight: u64,
    total_weight: u64,
    in_use: u64,
    capacity: u64,
) -> DependencySchedulerOutcome {
    if in_use > capacity || n > usize::MAX - edges.len() {
        return DependencySchedulerOutcome::Rejected;
    }
    for &(u, v) in edges {
        if u >= n || v >= n {
            return DependencySchedulerOutcome::Rejected;
        }
    }
    if !crate::job_dag::all_deps_satisfied(deps, completed) || crate::job_dag::unsatisfied_count(deps, completed) != 0 {
        return DependencySchedulerOutcome::Blocked;
    }
    if crate::cycle_detect::has_cycle(edges, n) {
        return DependencySchedulerOutcome::Cyclic;
    }
    let Some(topo_order) = crate::topo_sort::topo_sort(edges, n) else {
        return DependencySchedulerOutcome::Cyclic;
    };
    if !crate::topo_sort::is_topo_order(edges, n, &topo_order) {
        return DependencySchedulerOutcome::Rejected;
    }
    if deadlines.is_empty() || vtimes.is_empty() || total_weight == 0 || weight > total_weight {
        return DependencySchedulerOutcome::Rejected;
    }
    let by_deadline = crate::deadline_queue::edf_select(deadlines);
    let by_fairness = crate::fair_share::select_min_vtime(vtimes);
    if by_deadline != by_fairness {
        return DependencySchedulerOutcome::Rejected;
    }
    let allocation = crate::fair_share::fair_alloc(total_tokens, weight, total_weight);
    if allocation == 0 {
        return DependencySchedulerOutcome::Rejected;
    }
    if !crate::job_slot::has_available_slot(in_use, capacity) {
        return DependencySchedulerOutcome::Full;
    }
    let (acquired, new_in_use) = crate::job_slot::try_acquire_slot(in_use, capacity);
    if !acquired {
        return DependencySchedulerOutcome::Full;
    }
    DependencySchedulerOutcome::Scheduled {
        selected: by_deadline,
        allocation,
        new_in_use,
        topo_order,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_acyclic_schedules() {
        let deps = [1u64, 2];
        let completed = [2u64, 1];
        let edges = [(0usize, 1usize), (1, 2)];
        let deadlines = [50u64, 10, 30];
        let vtimes = [20u64, 5, 9];

        let outcome =
            dependency_scheduler_schedule(&deps, &completed, &edges, 3, &deadlines, &vtimes, 100, 3, 10, 1, 2);
        assert_eq!(outcome, DependencySchedulerOutcome::Scheduled {
            selected: 1,
            allocation: 30,
            new_in_use: 2,
            topo_order: vec![0, 1, 2],
        },);
    }

    #[test]
    fn blocked_when_dependencies_unsatisfied() {
        let deps = [1u64, 2];
        let completed = [1u64];
        let edges = [(0usize, 1usize)];
        let deadlines = [1u64];
        let vtimes = [0u64];

        assert_eq!(
            dependency_scheduler_schedule(&deps, &completed, &edges, 2, &deadlines, &vtimes, 10, 1, 1, 0, 1,),
            DependencySchedulerOutcome::Blocked,
        );
    }

    #[test]
    fn cyclic_graph_rejected() {
        let deps: [u64; 0] = [];
        let completed: [u64; 0] = [];
        let edges = [(0usize, 1usize), (1, 0)];
        let deadlines = [1u64];
        let vtimes = [0u64];

        assert_eq!(
            dependency_scheduler_schedule(&deps, &completed, &edges, 2, &deadlines, &vtimes, 10, 1, 1, 0, 1,),
            DependencySchedulerOutcome::Cyclic,
        );
    }

    #[test]
    fn full_slots_rejected() {
        let deps: [u64; 0] = [];
        let completed: [u64; 0] = [];
        let edges: [(usize, usize); 0] = [];
        let deadlines = [1u64];
        let vtimes = [0u64];

        assert_eq!(
            dependency_scheduler_schedule(&deps, &completed, &edges, 1, &deadlines, &vtimes, 10, 1, 1, 1, 1,),
            DependencySchedulerOutcome::Full,
        );
    }
}
