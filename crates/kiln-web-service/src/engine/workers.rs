use super::api::json_response;
use super::model::{DurableState, RunRecord};
use super::{ApiResponse, Engine, identity};
use crate::profile::ServiceProfile;
use kiln_core::domain::Outcome;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Default, Serialize)]
struct Capacity {
    configured_slots: usize,
    enrolled_slots: usize,
    active: usize,
    free_slots: usize,
    queued: usize,
}

#[derive(Serialize)]
struct Project<'a> {
    id: &'a str,
    label: &'a str,
    enrolled: bool,
    active_run_ids: Vec<&'a str>,
    queued: usize,
}

#[derive(Serialize)]
struct LatestRun<'a> {
    id: &'a str,
    project_id: &'a str,
    phase: &'a str,
    outcome: Outcome,
    sequence: Option<u64>,
    error_code: Option<&'a str>,
    created_at_ms: Option<u64>,
    started_at_ms: Option<u64>,
    completed_at_ms: Option<u64>,
}

#[derive(Serialize)]
struct Worker<'a> {
    id: String,
    provider_profile_id: &'a str,
    platform: &'a str,
    liveness: &'static str,
    status: &'static str,
    capacity: Capacity,
    projects: Vec<Project<'a>>,
    latest_run: Option<LatestRun<'a>>,
}

#[derive(Serialize)]
struct Summary {
    targets: usize,
    #[serde(flatten)]
    capacity: Capacity,
}

#[derive(Serialize)]
struct Inventory<'a> {
    schema: &'static str,
    version: u64,
    bounded: bool,
    storage_failed: bool,
    summary: Summary,
    workers: Vec<Worker<'a>>,
}

struct ProjectFacts<'a> {
    project: Project<'a>,
    ambiguous: bool,
    latest: Option<&'a RunRecord>,
}

struct TargetFacts<'a> {
    projects: Vec<Project<'a>>,
    ambiguous: bool,
    latest: Option<&'a RunRecord>,
}

fn latest<'a>(current: Option<&'a RunRecord>, run: &'a RunRecord) -> Option<&'a RunRecord> {
    Some(match current {
        Some(prior) if (prior.ordinal, &prior.id) > (run.ordinal, &run.id) => prior,
        _ => run,
    })
}

// r[impl kiln.dashboard.workers.inventory]
// r[impl kiln.dashboard.workers.capacity]
// r[impl kiln.dashboard.workers.inventory.scenario.grouping]
// r[impl kiln.dashboard.workers.capacity.scenario.transitions]
// r[impl kiln.dashboard.workers.capacity.scenario.unknown]
// The admitted profile and durable snapshot are the only inputs. In particular,
// idle project admission slots are not provider heartbeat or placement evidence.
fn project_workers<'a>(
    profile: &'a ServiceProfile,
    state: &'a DurableState,
) -> Result<Inventory<'a>, String> {
    let mut projects: BTreeMap<_, _> = profile
        .projects
        .iter()
        .map(|p| {
            (
                p.id.as_str(),
                ProjectFacts {
                    project: Project {
                        id: &p.id,
                        label: &p.label,
                        enrolled: state.enrolled.contains(&p.id),
                        active_run_ids: Vec::new(),
                        queued: 0,
                    },
                    ambiguous: false,
                    latest: None,
                },
            )
        })
        .collect();
    for run in state.runs.values() {
        let facts = projects
            .get_mut(run.project_id.as_str())
            .ok_or("run project missing")?;
        if run.phase == "queued" {
            facts.project.queued += 1;
            continue;
        }
        let outcome = run.outcome();
        // A cancelled queue-only request is terminal but never executed.
        if run.observation.is_some() || run.error_code.is_some() || !outcome.is_terminal() {
            facts.latest = latest(facts.latest, run);
        }
        // Keep this predicate aligned with queue::dispatch_queued. Unknown
        // executions retain their project slot, even without an observation.
        if !outcome.is_terminal() {
            facts.project.active_run_ids.push(&run.id);
            facts.ambiguous |=
                run.phase == "unknown" || outcome == Outcome::Unknown || run.observation.is_none();
        }
    }
    let mut targets: BTreeMap<(&str, &str, &str), TargetFacts<'a>> = BTreeMap::new();
    for p in &profile.projects {
        let facts = projects
            .remove(p.id.as_str())
            .ok_or("project facts missing")?;
        let target = targets
            .entry((
                p.workflow.provider_profile_id.as_str(),
                p.workflow
                    .socket_path
                    .to_str()
                    .ok_or("workflow endpoint encoding invalid")?,
                p.workflow.platform.as_str(),
            ))
            .or_insert_with(|| TargetFacts {
                projects: Vec::new(),
                ambiguous: false,
                latest: None,
            });
        target.ambiguous |= facts.ambiguous;
        if let Some(run) = facts.latest {
            target.latest = latest(target.latest, run);
        }
        target.projects.push(facts.project);
    }
    let mut workers = Vec::with_capacity(targets.len());
    let mut summary = Summary {
        targets: targets.len(),
        capacity: Capacity::default(),
    };
    for ((provider_profile_id, socket_path, platform), mut facts) in targets {
        facts.projects.sort_unstable_by_key(|p| p.id);
        let mut capacity = Capacity::default();
        for p in &facts.projects {
            capacity.configured_slots += 1;
            capacity.enrolled_slots += usize::from(p.enrolled);
            capacity.active += p.active_run_ids.len();
            capacity.free_slots += usize::from(p.enrolled && p.active_run_ids.is_empty());
            capacity.queued += p.queued;
        }
        let status = if capacity.enrolled_slots == 0 {
            "not_enrolled"
        } else if capacity.free_slots > 0 {
            "slots_available"
        } else if facts.ambiguous {
            "unknown"
        } else {
            "busy"
        };
        summary.capacity.configured_slots += capacity.configured_slots;
        summary.capacity.enrolled_slots += capacity.enrolled_slots;
        summary.capacity.active += capacity.active;
        summary.capacity.free_slots += capacity.free_slots;
        summary.capacity.queued += capacity.queued;
        workers.push(Worker {
            id: identity(&(
                "kiln.web-worker-target.v1",
                provider_profile_id,
                socket_path,
                platform,
            ))?
            .as_str()
            .to_owned(),
            provider_profile_id,
            platform,
            liveness: "unknown",
            status,
            capacity,
            projects: facts.projects,
            latest_run: facts.latest.map(|run| LatestRun {
                id: &run.id,
                project_id: &run.project_id,
                phase: &run.phase,
                outcome: run.outcome(),
                sequence: run.observation.as_ref().map(|o| o.sequence),
                error_code: run.error_code.as_deref(),
                created_at_ms: run.created_at_ms,
                started_at_ms: run.started_at_ms,
                completed_at_ms: run.completed_at_ms,
            }),
        });
    }
    workers.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    Ok(Inventory {
        schema: "kiln.web-workers.v1",
        version: state.version,
        bounded: true,
        storage_failed: false,
        summary,
        workers,
    })
}

impl Engine {
    pub(super) fn workers(&self) -> Result<ApiResponse, String> {
        json_response(200, project_workers(&self.profile, &self.state)?)
    }
}
