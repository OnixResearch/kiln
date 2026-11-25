//! Exact metadata returned by the opt-in Animus implementation host through Lattice.

use serde::{Deserialize, Serialize};

const SCHEMA: &str = "animus.implementation-child-lineage.v1";
const MAX_DIRECT_CHILDREN: usize = 32;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AgentLineage {
    Unsupported,
    Unobserved,
    Recorded { receipt: LineageReceipt },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageReceipt {
    schema: String,
    work_identity: String,
    attempt_identity: String,
    fence: u64,
    request_identity: String,
    worker_incarnation: String,
    profile_revision: String,
    root_id: String,
    coverage: Coverage,
    edges: Vec<Edge>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Coverage {
    Complete,
    Partial,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Edge {
    parent_id: String,
    child_id: String,
    state: ChildState,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ChildState {
    Completed,
    Partial,
    Failed,
    TimedOut,
    Cancelled,
    BoundExceeded,
}

impl LineageReceipt {
    pub(super) fn matches(&self, instance: &super::NativeInstance, digest: &str) -> bool {
        if self.schema != SCHEMA
            || self.request_identity != digest
            || self.worker_incarnation != instance.incarnation
            || self.profile_revision != instance.profile_revision
            || self.work_identity != instance.work_id
            || self.attempt_identity != instance.attempt_id
            || self.fence != instance.fence
            || !super::hash(&self.root_id)
            || self.edges.len() > MAX_DIRECT_CHILDREN
        {
            return false;
        }
        // The selected profile admits direct children only. At this bound a
        // scan needs no set allocation; every child may appear just once.
        self.edges.iter().enumerate().all(|(index, edge)| {
            edge.parent_id == self.root_id
                && edge.child_id != self.root_id
                && super::hash(&edge.child_id)
                && !self.edges[..index].iter().any(|other| other.child_id == edge.child_id)
        })
    }
}
