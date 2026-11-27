//! Exact owner-side planning for independent, already-admitted work units.
//! A plan never authorizes transport. It is re-derived before reservation.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{
    FleetScope, WHOLE_REPOSITORY_PATH, bounded_token, content_identity, overlaps, parse_scope,
    push_field, refusal,
};
use crate::implementation::{
    AttemptLedger, AttemptPhase, Eligibility, INTERACTIVE_AGENT_EFFECT,
    INTERACTIVE_PROVIDER_PROFILE, ImplementationAttempt, ImplementationDispatchPlan,
    ImplementationWork, WorkLedger, admit_dispatch_plan,
};
use crate::types::{ContentId, CoreResult, EffectId};

pub const MAX_SPLIT_UNITS: usize = 16;
const MIN_SPLIT_UNITS: usize = 2;
const MAX_SPLIT_PATHS: usize = 16;
const MAX_DESTINATION_BYTES: usize = 128;
const SPLIT_SCHEMA: &str = "kiln.fleet-placement-split.v1";
const MOVE_SCHEMA: &str = "kiln.fleet-placement-move.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveRequest {
    pub attempt: ContentId,
    pub fence: u64,
    /// Effect ID of the current reservation, not its reusable plan ID.
    pub reservation: EffectId,
    pub destination: String,
}

impl MoveRequest {
    pub fn validate(&self) -> CoreResult<()> {
        self.attempt.validate()?;
        if self.fence == 0
            || !bounded_token(&self.destination, MAX_DESTINATION_BYTES)
            || self
                .reservation
                .as_str()
                .strip_prefix("dispatch-")
                .is_none_or(|hash| {
                    hash.len() != 64
                        || !hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
        {
            return Err(refusal(
                "fleet_placement_source",
                "move",
                "move needs an exact fence, reservation effect ID and destination label",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveSubmit {
    pub plan: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
    pub reservation: EffectId,
    pub destination: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovePlan {
    pub identity: ContentId,
    pub request: MoveRequest,
    pub source: String,
    pub work: ContentId,
    pub work_revision: u64,
    pub dispatch_plan: ContentId,
    pub new_reservation: EffectId,
}

// r[impl kiln.sdlc.queued_workload_move]
pub fn plan_move(
    request: &MoveRequest,
    source: String,
    attempt: &ImplementationAttempt,
    replacement: &crate::implementation::DispatchReservation,
) -> CoreResult<MovePlan> {
    let mut bytes = Vec::new();
    for value in [
        MOVE_SCHEMA,
        request.attempt.as_str(),
        request.reservation.as_str(),
        &source,
        &request.destination,
        replacement.effect_id.as_str(),
    ] {
        push_field(&mut bytes, value, "move.plan")?;
    }
    bytes.extend_from_slice(&request.fence.to_be_bytes());
    Ok(MovePlan {
        identity: content_identity(&bytes)?,
        request: request.clone(),
        source,
        work: attempt.work_identity.clone(),
        work_revision: attempt.work_revision,
        dispatch_plan: replacement.plan.clone(),
        new_reservation: replacement.effect_id.clone(),
    })
}

/// Check the exact live reservation and reject any evidence of dispatch starting.
pub fn queued_move_source<'a>(
    attempts: &'a AttemptLedger,
    work: &ImplementationWork,
    request: &MoveRequest,
    now_ms: u64,
) -> CoreResult<(&'a ImplementationAttempt, &'a ImplementationDispatchPlan)> {
    request.validate()?;
    let stale = || {
        refusal(
            "fleet_placement_stale",
            "move",
            "attempt, fence, reservation or work revision changed",
        )
    };
    let attempt = attempts
        .get(&request.attempt)
        .filter(|attempt| attempts.is_current(&attempt.identity, request.fence, now_ms))
        .ok_or_else(stale)?;
    let reservation = attempt
        .dispatch
        .as_ref()
        .filter(|reservation| reservation.effect_id == request.reservation)
        .ok_or_else(stale)?;
    if attempt.phase != AttemptPhase::Claimed
        || attempt.workflow.is_some()
        || attempt.dispatch_request_binding.is_some()
        || attempts.dispatch_requires_recovery(&attempt.identity)
        || attempts
            .unresolved()
            .any(|prior| prior.canonical_key == attempt.canonical_key)
    {
        return Err(refusal(
            "fleet_placement_source",
            "move",
            "attempt already has execution or unresolved launch evidence",
        ));
    }
    if attempt.superseded_by.is_some()
        || work.revision != attempt.work_revision
        || work.identity()? != attempt.work_identity
    {
        return Err(stale());
    }
    let old = attempts
        .dispatch_plan(&request.attempt)
        .filter(|plan| {
            reservation.provider_profile.as_str() == INTERACTIVE_PROVIDER_PROFILE
                && plan.provider_profile == reservation.provider_profile
                && plan.execution.is_none()
        })
        .ok_or_else(|| {
            refusal(
                "fleet_placement_source",
                "move",
                "move requires a retained native interactive reservation",
            )
        })?;
    if admit_dispatch_plan(work, attempt, old)? != reservation.plan {
        return Err(stale());
    }
    Ok((attempt, old))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitUnit {
    pub attempt: ContentId,
    pub fence: u64,
    pub destination: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitRequest {
    pub units: Vec<SplitUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitSubmit {
    pub plan: ContentId,
    pub units: Vec<SplitUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedUnit {
    pub attempt: ContentId,
    pub fence: u64,
    pub destination: String,
    pub work: ContentId,
    pub work_revision: u64,
    pub scopes: Vec<FleetScope>,
    pub dependencies: Vec<ContentId>,
    pub dispatch_plan: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitPlan {
    pub identity: ContentId,
    pub units: Vec<PlannedUnit>,
}

impl SplitPlan {
    /// Replay is scoped to the same ordered units, including fences and hosts.
    #[must_use]
    pub fn matches_units(&self, units: &[&SplitUnit]) -> bool {
        self.units.len() == units.len()
            && self.units.iter().zip(units).all(|(plan, unit)| {
                plan.attempt == unit.attempt
                    && plan.fence == unit.fence
                    && plan.destination == unit.destination
            })
    }
}

impl SplitRequest {
    /// Canonical request order; refuse duplicate attempts before consulting a provider.
    pub fn ordered(&self) -> CoreResult<Vec<&SplitUnit>> {
        if !(MIN_SPLIT_UNITS..=MAX_SPLIT_UNITS).contains(&self.units.len()) {
            return Err(refusal(
                "fleet_placement_units",
                "units",
                "split requires 2–16 units",
            ));
        }
        let mut ordered: Vec<_> = self.units.iter().collect();
        ordered.sort_unstable_by(|a, b| a.attempt.cmp(&b.attempt));
        let mut destinations = BTreeSet::new();
        for unit in &ordered {
            unit.attempt.validate()?;
            if unit.fence == 0 || !bounded_token(&unit.destination, MAX_DESTINATION_BYTES) {
                return Err(refusal(
                    "fleet_placement_units",
                    "units",
                    "each unit needs an exact fence and destination label",
                ));
            }
            destinations.insert(unit.destination.as_str());
        }
        if ordered
            .windows(2)
            .any(|pair| matches!(pair, [left, right] if left.attempt == right.attempt))
            || destinations.len() < MIN_SPLIT_UNITS
        {
            return Err(refusal(
                "fleet_placement_units",
                "units",
                "split needs distinct attempts across at least two destinations",
            ));
        }
        Ok(ordered)
    }
}

/// Conservative claim coverage: stop at the first glob segment; an unknown
/// scope must never be taken as evidence of independence.
#[must_use]
pub fn work_scopes(work: &ImplementationWork) -> Vec<FleetScope> {
    let whole = || parse_scope(&work.repository, WHOLE_REPOSITORY_PATH).ok();
    if work.permitted_paths.is_empty() {
        return whole().into_iter().collect();
    }
    let mut scopes = BTreeSet::new();
    for path in &work.permitted_paths {
        let prefix = path
            .split('/')
            .take_while(|segment| !segment.contains(['*', '?', '[', '{', '\\']));
        let end = prefix
            .map(|segment| segment.len() + 1)
            .sum::<usize>()
            .saturating_sub(1);
        if let Some(scope) = path
            .get(..end)
            .and_then(|prefix| parse_scope(&work.repository, prefix).ok())
            .or_else(whole)
        {
            scopes.insert(scope);
        }
    }
    scopes.into_iter().collect()
}

/// Resolve exact current, undispatched attempt and work facts from the existing owner ledgers.
pub fn split_work<'work, 'attempt>(
    work: &'work WorkLedger,
    attempts: &'attempt AttemptLedger,
    unit: &SplitUnit,
    now_ms: u64,
) -> CoreResult<(&'work ImplementationWork, &'attempt ImplementationAttempt)> {
    let stale = || {
        refusal(
            "fleet_placement_stale",
            "units",
            "split requires an exact current claimed attempt and work revision",
        )
    };
    let attempt = attempts
        .get(&unit.attempt)
        .filter(|a| attempts.is_current(&a.identity, unit.fence, now_ms))
        .ok_or_else(stale)?;
    if attempt.phase != AttemptPhase::Claimed
        || attempt.dispatch.is_some()
        || attempt.workflow.is_some()
        || attempts.dispatch_plan(&attempt.identity).is_some()
        || attempts
            .unresolved()
            .any(|prior| prior.canonical_key == attempt.canonical_key)
    {
        return Err(refusal(
            "fleet_placement_source",
            "units",
            "reserved, executing or unresolved attempts cannot be split",
        ));
    }
    let work_record = work.get(&attempt.canonical_key).ok_or_else(stale)?;
    if work_record.revision != attempt.work_revision
        || work_record.identity()? != attempt.work_identity
    {
        return Err(stale());
    }
    if !work_record
        .permitted_effects
        .contains(INTERACTIVE_AGENT_EFFECT)
    {
        return Err(refusal(
            "fleet_placement_source",
            "units",
            "work does not admit interactive dispatch",
        ));
    }
    check_split_dependencies(work, attempts, work_record, attempt)?;
    Ok((work_record, attempt))
}

/// Check that the exact work still has independent admission at launch time.
pub fn check_split_dependencies(
    work: &WorkLedger,
    attempts: &AttemptLedger,
    work_record: &ImplementationWork,
    attempt: &ImplementationAttempt,
) -> CoreResult<()> {
    if work.get(&attempt.canonical_key).is_none_or(|current| {
        current.revision != attempt.work_revision
            || current.identity().ok().as_ref() != Some(&attempt.work_identity)
    }) {
        return Err(refusal(
            "fleet_placement_stale",
            "units",
            "work revision changed after placement admission",
        ));
    }
    if !matches!(
        work.eligibility_of(&attempt.canonical_key),
        Some(Eligibility::Ready)
    ) || work_record.dependencies.iter().any(|dependency| {
        !attempts.attempts().any(|done| {
            &done.work_identity == dependency
                && done.phase == AttemptPhase::Verified
                && done.verification.is_some()
                && work
                    .get(&done.canonical_key)
                    .is_some_and(|current| current.revision == done.work_revision)
                && attempts
                    .authorize_publication(&done.identity, done.fence)
                    .is_ok()
        })
    }) {
        return Err(refusal(
            "fleet_placement_dependencies",
            "units",
            "split units must be independently eligible with all exact prerequisites already verified",
        ));
    }
    Ok(())
}

// r[impl kiln.sdlc.workload_placement]
/// Bind provider-selected native plans to exact work/attempt lineage. The
/// provider checks the actual destination and available capacity independently.
pub fn plan_split<'a>(
    units: impl IntoIterator<
        Item = (
            &'a SplitUnit,
            &'a ImplementationWork,
            &'a ImplementationAttempt,
            &'a ImplementationDispatchPlan,
        ),
    >,
) -> CoreResult<SplitPlan> {
    let mut planned: Vec<PlannedUnit> = Vec::new();
    let mut bytes = Vec::new();
    push_field(&mut bytes, SPLIT_SCHEMA, "plan")?;
    for (unit, work, attempt, plan) in units {
        if unit.attempt != attempt.identity || unit.fence != attempt.fence {
            return Err(refusal(
                "fleet_placement_stale",
                "units",
                "split request differs from the exact provider attempt and fence",
            ));
        }
        let scopes = work_scopes(work);
        if scopes.is_empty()
            || scopes.len() > MAX_SPLIT_PATHS
            || scopes
                .iter()
                .any(|scope| scope.path_text() == WHOLE_REPOSITORY_PATH)
        {
            return Err(refusal(
                "fleet_placement_overlap",
                "units",
                "each split unit needs a bounded, non-whole-repository scope",
            ));
        }
        if planned
            .iter()
            .flat_map(|entry| &entry.scopes)
            .any(|prior| scopes.iter().any(|scope| overlaps(prior, scope)))
        {
            return Err(refusal(
                "fleet_placement_overlap",
                "units",
                "split unit scopes overlap",
            ));
        }
        if plan.provider_profile.as_str() != INTERACTIVE_PROVIDER_PROFILE
            || plan.execution.is_some()
        {
            return Err(refusal(
                "fleet_placement_source",
                "units",
                "split requires the native interactive provider plan",
            ));
        }
        let dispatch_plan = admit_dispatch_plan(work, attempt, plan)?;
        push_field(&mut bytes, unit.attempt.as_str(), "attempt")?;
        bytes.extend_from_slice(&unit.fence.to_be_bytes());
        push_field(&mut bytes, &unit.destination, "destination")?;
        push_field(&mut bytes, dispatch_plan.as_str(), "dispatch_plan")?;
        planned.push(PlannedUnit {
            attempt: unit.attempt.clone(),
            fence: unit.fence,
            destination: unit.destination.clone(),
            work: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            scopes,
            dependencies: work.dependencies.clone(),
            dispatch_plan,
        });
    }
    Ok(SplitPlan {
        identity: content_identity(&bytes)?,
        units: planned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn canonical_split_order_never_admits_duplicate_attempts_or_single_host() -> CoreResult<()> {
        let first = SplitUnit {
            attempt: content_identity(b"first")?,
            fence: 1,
            destination: "one".into(),
        };
        let mut second = SplitUnit {
            attempt: content_identity(b"second")?,
            fence: 2,
            destination: "two".into(),
        };
        let ordered = SplitRequest {
            units: vec![first.clone(), second.clone()],
        };
        let reversed = SplitRequest {
            units: vec![second.clone(), first.clone()],
        };
        assert_eq!(ordered.ordered()?, reversed.ordered()?);

        second.destination = first.destination.clone();
        let single_host = SplitRequest {
            units: vec![first.clone(), second],
        };
        assert!(single_host.ordered().is_err());
        let mut duplicate = first.clone();
        duplicate.destination = "two".into();
        assert!(
            SplitRequest {
                units: vec![first, duplicate]
            }
            .ordered()
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn queued_move_source_allows_re_move_and_refuses_execution_or_unresolved_prior() -> CoreResult<()>
    {
        use crate::implementation::{
            AcceptanceObligation, DEFAULT_LEASE_MS, DispatchControls, DispatchExecutionBindings,
            DispatchWorkspace, FailureClass, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork,
            SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind, WorkspaceIsolation,
        };
        use crate::inventory::{
            AttemptBinding, FactRef, ResourceRequirement, WorkNeeds, WorkNeedsAttachment,
        };
        use crate::types::{ExternalRef, ProviderProfileId};

        let base = Some(ExternalRef::new("git", "commit", &"1".repeat(40))?);
        let work = ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.into(),
            revision: 1,
            actor: "operator".into(),
            repository: "onix/kiln".into(),
            project: "kiln".into(),
            base: base.clone(),
            title: "move work".into(),
            requested_outcome: "implement".into(),
            acceptance: vec![AcceptanceObligation {
                id: "accept".into(),
                statement: "accept".into(),
            }],
            dependencies: vec![],
            permitted_paths: vec!["src".into()],
            permitted_effects: BTreeSet::from([
                "write_source".into(),
                INTERACTIVE_AGENT_EFFECT.into(),
            ]),
            verification_profile: content_identity(b"verify")?,
            budget: WorkBudget {
                attempts: 3,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "onix/kiln#1")?,
                native_id: "onix/kiln#1".into(),
                version: content_identity(b"src")?,
            }],
        };

        let make_plan = |attempt: &ImplementationAttempt, ws: &[u8]| -> CoreResult<ImplementationDispatchPlan> {
            Ok(ImplementationDispatchPlan {
                schema: crate::implementation::DISPATCH_PLAN_SCHEMA.into(),
                work_identity: attempt.work_identity.clone(),
                work_revision: work.revision,
                attempt_identity: attempt.identity.clone(),
                fence: attempt.fence,
                repository: work.repository.clone(),
                base: base.clone(),
                workspace: DispatchWorkspace {
                    isolation: WorkspaceIsolation::InteractiveWorktree,
                    identity: content_identity(ws)?,
                    network: true,
                    credentials: true,
                    projection: None,
                },
                provider_profile: ProviderProfileId::new(INTERACTIVE_PROVIDER_PROFILE)?,
                permitted_paths: work.permitted_paths.clone(),
                permitted_effects: work.permitted_effects.clone(),
                controls: DispatchControls {
                    cancel: true,
                    pause: false,
                    resume: false,
                },
                deadline_minutes: 60,
                token_budget: 1_000,
                invocation_budget: 10,
                expected_output: content_identity(b"output")?,
                execution: None,
            })
        };

        // 1. Positive: transport_start_bound (from an earlier move) is still allowed to re-move.
        let mut ledger = AttemptLedger::default();
        let attempt = ledger.claim(&work, 10, DEFAULT_LEASE_MS)?;
        let plan = make_plan(&attempt, b"ws-1")?;
        let reservation = ledger.reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 11, &plan)?;
        ledger.bind_transport_start(&attempt.identity)?;
        let req = MoveRequest {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            reservation: reservation.effect_id,
            destination: "aspen3".into(),
        };
        assert!(queued_move_source(&ledger, &work, &req, 12).is_ok());

        // 2. Negative: plan.execution.is_some() is refused.
        let mut exec_ledger = AttemptLedger::default();
        let exec_attempt = exec_ledger.claim(&work, 10, DEFAULT_LEASE_MS)?;
        let mut exec_plan = make_plan(&exec_attempt, b"ws-2")?;
        let policy_id = content_identity(b"policy")?;
        let reference = |id: &str| FactRef {
            source: "provider-v1".into(),
            id: id.into(),
        };
        exec_plan.execution = Some(DispatchExecutionBindings {
            native_request_profile: None,
            requirements: WorkNeedsAttachment::new(
                exec_attempt.work_identity.clone(),
                work.revision,
                policy_id.clone(),
                WorkNeeds {
                    work_version: exec_attempt.work_identity.as_str().into(),
                    task_family: "feature".into(),
                    output_contract: "candidate-v1".into(),
                    definition: reference("definition"),
                    profile_revision: "worker-v1".into(),
                    policy_ref: policy_id.as_str().into(),
                    harness: "omp-jsonl".into(),
                    tools: vec!["edit".into()],
                    sandbox: "bubblewrap".into(),
                    execution_backend: "cpu-linux".into(),
                    model_backend: "remote-api".into(),
                    context_tokens: 1024,
                    execution_resources: vec![ResourceRequirement {
                        unit: "cpu".into(),
                        amount: 1,
                    }],
                    modalities: vec!["text".into()],
                    model_artifact: Some("model-v1".into()),
                    qualification_subject: "subject".into(),
                    qualification_corpus: "corpus".into(),
                    qualification_protocol: "v1".into(),
                },
            )
            .map_err(|msg| refusal("test", "needs", msg))?,
            attempt: AttemptBinding {
                work_version: exec_attempt.work_identity.as_str().into(),
                attempt: exec_attempt.identity.as_str().into(),
                fence: exec_attempt.fence,
                worker: reference("worker"),
                execution_grant: reference("cpu-grant"),
                model_services: vec![reference("shared-model")],
                request_admissions: vec![reference("request-grant")],
            },
        });
        let exec_res = exec_ledger.reserve_dispatch_plan(
            &work,
            &exec_attempt.identity,
            exec_attempt.fence,
            11,
            &exec_plan,
        )?;
        let exec_req = MoveRequest {
            attempt: exec_attempt.identity,
            fence: exec_attempt.fence,
            reservation: exec_res.effect_id,
            destination: "aspen3".into(),
        };
        assert_eq!(
            queued_move_source(&exec_ledger, &work, &exec_req, 12)
                .err()
                .map(|e| e.code),
            Some("fleet_placement_source".into())
        );

        // 3. Negative: unresolved prior attempt on canonical_key is refused.
        let mut prior_ledger = AttemptLedger::default();
        let a1 = prior_ledger.claim(&work, 10, DEFAULT_LEASE_MS)?;
        let p1 = make_plan(&a1, b"ws-3")?;
        let _r1 = prior_ledger.reserve_dispatch_plan(&work, &a1.identity, a1.fence, 11, &p1)?;
        prior_ledger.fail(&a1.identity, a1.fence, 12, FailureClass::Infrastructure, "tmp")?;
        let a2 = prior_ledger.claim(&work, 13, DEFAULT_LEASE_MS)?;
        let p2 = make_plan(&a2, b"ws-4")?;
        let r2 = prior_ledger.reserve_dispatch_plan(&work, &a2.identity, a2.fence, 14, &p2)?;
        let mut ledger_json = serde_json::to_value(&prior_ledger).expect("serialize ledger");
        ledger_json["attempts"][a1.identity.as_str()]["failure"] =
            serde_json::to_value(FailureClass::Unknown).expect("serialize failure class");
        let prior_ledger: AttemptLedger =
            serde_json::from_value(ledger_json).expect("deserialize ledger");
        let req2 = MoveRequest {
            attempt: a2.identity,
            fence: a2.fence,
            reservation: r2.effect_id,
            destination: "aspen3".into(),
        };
        assert_eq!(
            queued_move_source(&prior_ledger, &work, &req2, 15)
                .err()
                .map(|e| e.code),
            Some("fleet_placement_source".into())
        );
        Ok(())
    }
}
