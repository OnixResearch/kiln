//! Admission of one bounded implementation-dispatch plan.
//!
//! A plan binds the exact work/version, attempt/fence, repository/base,
//! workspace isolation, provider profile, permitted edits and effects,
//! cancellation-only controls, deadline, token/invocation budgets and expected
//! output contract. Admission produces the deterministic plan identity that the
//! attempt ledger reserves before transport.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{ImplementationWork, content_identity, push_str};
use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef, ProviderProfileId};

pub const DISPATCH_PLAN_SCHEMA: &str = "kiln.implementation-dispatch-plan.v1";
pub const MAX_DISPATCH_PATHS: usize = 64;
pub const MAX_DISPATCH_EFFECTS: usize = 16;
pub const MAX_DISPATCH_TOKENS: u64 = 10_000_000;
pub const MAX_DISPATCH_INVOCATIONS: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceIsolation {
    /// A provider-owned disposable snapshot; no ambient host directory.
    IsolatedSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchWorkspace {
    pub identity: ContentId,
    pub isolation: WorkspaceIsolation,
    /// The worker route never receives a network device or network policy.
    pub network: bool,
    /// Publisher, forge and host credentials remain outside the worker.
    pub credentials: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchControls {
    pub cancel: bool,
    pub pause: bool,
    pub resume: bool,
}

/// Exact provider facts selected for the explicit implementation worker route.
/// These references retain provider grants; they do not grant resources.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchExecutionBindings {
    pub requirements: crate::inventory::WorkNeedsAttachment,
    pub attempt: crate::inventory::AttemptBinding,
    pub worker_request: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationDispatchPlan {
    pub schema: String,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub attempt_identity: ContentId,
    pub fence: u64,
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<ExternalRef>,
    pub workspace: DispatchWorkspace,
    pub provider_profile: ProviderProfileId,
    pub permitted_paths: Vec<String>,
    pub permitted_effects: BTreeSet<String>,
    pub controls: DispatchControls,
    pub deadline_minutes: u32,
    pub token_budget: u64,
    pub invocation_budget: u32,
    pub expected_output: ContentId,
    /// Absence is the explicit legacy configured-route compatibility profile;
    /// it conveys no model-service or physical-allocation qualification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<DispatchExecutionBindings>,
}

/// Admit a plan against the exact work and attempt it names.
///
/// Returns the canonical plan identity. The plan is data only; admission does
/// not reserve or dispatch it.
// r[impl kiln.implementation.dispatch_plan]
pub fn admit_dispatch_plan(
    work: &ImplementationWork,
    attempt: &super::ImplementationAttempt,
    plan: &ImplementationDispatchPlan,
) -> CoreResult<ContentId> {
    if plan.schema != DISPATCH_PLAN_SCHEMA
        || plan.work_identity.validate().is_err()
        || plan.attempt_identity.validate().is_err()
        || plan.workspace.identity.validate().is_err()
        || plan.expected_output.validate().is_err()
        || plan.provider_profile.as_str().is_empty()
        || plan.provider_profile.as_str().chars().any(char::is_control)
        || plan.work_identity != work.identity()?
        || plan.work_revision != work.revision
        || plan.work_identity != attempt.work_identity
        || plan.work_revision != attempt.work_revision
        || plan.attempt_identity != attempt.identity
        || plan.fence != attempt.fence
        || plan.repository != work.repository
        || plan.base.is_none()
        || plan.base != work.base
        || plan.base != attempt.base
    {
        return Err(refusal(
            "implementation_dispatch_binding",
            "dispatch.plan",
            "dispatch plan does not bind the exact work and attempt",
        ));
    }
    if !plan.controls.cancel || plan.controls.pause || plan.controls.resume {
        return Err(refusal(
            "implementation_dispatch_controls",
            "dispatch.controls",
            "implementation dispatch admits cancellation only",
        ));
    }
    if plan.workspace.credentials
        || (plan.workspace.network
            && plan.execution.as_ref().is_none_or(|execution| {
                execution.attempt.model_services.is_empty()
                    || execution.attempt.request_admissions.is_empty()
            }))
    {
        return Err(refusal(
            "implementation_dispatch_workspace",
            "dispatch.workspace",
            "network requires explicit model-service admissions; ambient credentials are refused",
        ));
    }
    if plan.deadline_minutes == 0
        || plan.deadline_minutes > work.budget.minutes_per_attempt
        || plan.token_budget == 0
        || plan.token_budget > MAX_DISPATCH_TOKENS
        || plan.invocation_budget == 0
        || plan.invocation_budget > MAX_DISPATCH_INVOCATIONS
        || plan.permitted_paths.len() > MAX_DISPATCH_PATHS
        || plan.permitted_effects.len() > MAX_DISPATCH_EFFECTS
        || plan.permitted_paths.is_empty()
        || plan.permitted_effects.is_empty()
    {
        return Err(refusal(
            "implementation_dispatch_bounds",
            "dispatch.bounds",
            "dispatch deadline or budgets are empty or exceed their bounds",
        ));
    }
    let admitted_paths: BTreeSet<&str> = work.permitted_paths.iter().map(String::as_str).collect();
    let mut seen_paths = BTreeSet::new();
    for path in &plan.permitted_paths {
        if !admitted_paths.contains(path.as_str()) || !seen_paths.insert(path.as_str()) {
            return Err(refusal(
                "implementation_dispatch_paths",
                "dispatch.permitted_paths",
                "dispatch paths are duplicated or not admitted by the work",
            ));
        }
    }
    for effect in &plan.permitted_effects {
        if !work.permitted_effects.contains(effect) {
            return Err(refusal(
                "implementation_dispatch_effects",
                "dispatch.permitted_effects",
                "dispatch effect is not admitted by the work",
            ));
        }
    }
    if let Some(execution) = &plan.execution {
        execution
            .requirements
            .validate_for(
                &plan.work_identity,
                plan.work_revision,
                &execution.requirements.policy_identity,
            )
            .map_err(|_| {
                refusal(
                    "implementation_dispatch_requirements",
                    "dispatch.execution.requirements",
                    "requirements do not bind the exact admitted work version",
                )
            })?;
        let binding = &execution.attempt;
        if binding.work_version != plan.work_identity.as_str()
            || binding.attempt != plan.attempt_identity.as_str()
            || binding.fence != plan.fence
            || execution.worker_request.validate().is_err()
            || binding.model_services.len() > crate::inventory::MAX_REQUIREMENTS
            || binding.request_admissions.len() > crate::inventory::MAX_REQUIREMENTS
        {
            return Err(refusal(
                "implementation_dispatch_execution",
                "dispatch.execution",
                "execution, model admission or worker references are crossed or unbounded",
            ));
        }
        let mut seen = BTreeSet::new();
        for reference in core::iter::once(&binding.worker)
            .chain(core::iter::once(&binding.execution_grant))
            .chain(binding.model_services.iter())
            .chain(binding.request_admissions.iter())
        {
            if reference.source.is_empty()
                || reference.id.is_empty()
                || reference.source.len() > crate::inventory::MAX_PUBLIC_TEXT
                || reference.id.len() > crate::inventory::MAX_PUBLIC_TEXT
                || reference
                    .source
                    .chars()
                    .chain(reference.id.chars())
                    .any(char::is_control)
                || !seen.insert((reference.source.as_str(), reference.id.as_str()))
            {
                return Err(refusal(
                    "implementation_dispatch_execution",
                    "dispatch.execution",
                    "provider references are malformed, duplicated or ambiguous",
                ));
            }
        }
    }
    let mut bytes = Vec::new();
    push_str(&mut bytes, DISPATCH_PLAN_SCHEMA)?;
    push_str(&mut bytes, plan.work_identity.as_str())?;
    bytes.extend_from_slice(&plan.work_revision.to_be_bytes());
    push_str(&mut bytes, plan.attempt_identity.as_str())?;
    bytes.extend_from_slice(&plan.fence.to_be_bytes());
    push_str(&mut bytes, &plan.repository)?;
    if let Some(base) = &plan.base {
        push_str(&mut bytes, base.provider_namespace.as_str())?;
        push_str(&mut bytes, base.native_kind.as_str())?;
        push_str(&mut bytes, base.native_id.as_str())?;
    }
    push_str(&mut bytes, plan.workspace.identity.as_str())?;
    push_str(&mut bytes, "isolated_snapshot")?;
    push_str(&mut bytes, plan.provider_profile.as_str())?;
    let mut paths: Vec<&str> = plan.permitted_paths.iter().map(String::as_str).collect();
    paths.sort_unstable();
    for path in paths {
        push_str(&mut bytes, path)?;
    }
    for effect in &plan.permitted_effects {
        push_str(&mut bytes, effect)?;
    }
    push_str(&mut bytes, plan.expected_output.as_str())?;
    bytes.push(u8::from(plan.controls.cancel));
    bytes.push(u8::from(plan.controls.pause));
    bytes.push(u8::from(plan.controls.resume));
    bytes.extend_from_slice(&plan.deadline_minutes.to_be_bytes());
    bytes.extend_from_slice(&plan.token_budget.to_be_bytes());
    bytes.extend_from_slice(&plan.invocation_budget.to_be_bytes());
    if let Some(execution) = &plan.execution {
        push_str(&mut bytes, "kiln.implementation-execution-bindings.v1")?;
        bytes.push(u8::from(plan.workspace.network));
        push_str(
            &mut bytes,
            execution
                .requirements
                .identity()
                .map_err(|_| {
                    refusal(
                        "implementation_dispatch_requirements",
                        "dispatch.execution",
                        "requirements identity is invalid",
                    )
                })?
                .as_str(),
        )?;
        push_str(&mut bytes, execution.worker_request.as_str())?;
        let binding = &execution.attempt;
        for reference in core::iter::once(&binding.worker)
            .chain(core::iter::once(&binding.execution_grant))
            .chain(binding.model_services.iter())
            .chain(binding.request_admissions.iter())
        {
            push_str(&mut bytes, &reference.source)?;
            push_str(&mut bytes, &reference.id)?;
        }
        bytes.extend_from_slice(&(binding.model_services.len() as u64).to_be_bytes());
        bytes.extend_from_slice(&(binding.request_admissions.len() as u64).to_be_bytes());
    }
    content_identity(&bytes)
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementation::{
        AcceptanceObligation, AttemptLedger, DEFAULT_LEASE_MS, IMPLEMENTATION_WORK_SCHEMA,
        SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
    };
    use alloc::collections::BTreeSet;
    use alloc::format;
    use alloc::string::ToString;
    use alloc::vec;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn work() -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: Some(
                ExternalRef::new("git", "commit", "base").unwrap_or_else(|_| unreachable!()),
            ),
            title: "Dispatch work".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates/kiln-core".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: content('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "onix/kiln#45")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "onix/kiln#45".to_string(),
                version: content('1'),
            }],
        }
    }

    fn plan(
        work: &ImplementationWork,
        attempt: &super::super::ImplementationAttempt,
    ) -> ImplementationDispatchPlan {
        ImplementationDispatchPlan {
            schema: DISPATCH_PLAN_SCHEMA.to_string(),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            repository: work.repository.clone(),
            base: work.base.clone(),
            workspace: DispatchWorkspace {
                identity: content('d'),
                isolation: WorkspaceIsolation::IsolatedSnapshot,
                network: false,
                credentials: false,
            },
            provider_profile: ProviderProfileId::new("lattice-implementation")
                .unwrap_or_else(|_| unreachable!()),
            permitted_paths: vec!["crates/kiln-core".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            controls: DispatchControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            deadline_minutes: 60,
            token_budget: 10_000,
            invocation_budget: 10,
            expected_output: content('e'),
            execution: None,
        }
    }

    #[test]
    // r[verify kiln.implementation.dispatch_plan.scenario.admit] r[verify kiln.implementation.dispatch_base.scenario.required]
    fn admits_an_exact_plan_and_reserves_it() {
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let plan = plan(&work, &attempt);
        let identity = admit_dispatch_plan(&work, &attempt, &plan)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let reservation = ledger
            .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &plan)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(reservation.plan, identity);
        ledger
            .start_execution(
                &attempt.identity,
                attempt.fence,
                2,
                &ExternalRef::new("lattice", "run", "one").unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!());
    }

    #[test]
    // r[verify kiln.implementation.dispatch_plan.scenario.refused] r[verify kiln.implementation.dispatch_base.scenario.required]
    fn refuses_broader_or_mismatched_plans() {
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let good = plan(&work, &attempt);
        let cases = [
            ("binding", {
                let mut plan = good.clone();
                plan.work_revision = 2;
                plan
            }),
            ("binding", {
                let mut plan = good.clone();
                plan.base = None;
                plan
            }),
            ("controls", {
                let mut plan = good.clone();
                plan.controls.pause = true;
                plan
            }),
            ("workspace", {
                let mut plan = good.clone();
                plan.workspace.network = true;
                plan
            }),
            ("bounds", {
                let mut plan = good.clone();
                plan.deadline_minutes = 61;
                plan
            }),
            ("paths", {
                let mut plan = good.clone();
                plan.permitted_paths = vec!["crates/other".to_string()];
                plan
            }),
            ("effects", {
                let mut plan = good.clone();
                plan.permitted_effects = BTreeSet::from(["push".to_string()]);
                plan
            }),
        ];
        for (field, plan) in cases {
            let error = admit_dispatch_plan(&work, &attempt, &plan)
                .err()
                .unwrap_or_else(|| unreachable!("{field}"));
            assert_eq!(
                error.code.as_str(),
                format!("implementation_dispatch_{field}")
            );
        }
    }

    #[test]
    fn exact_worker_model_and_request_bindings_survive_reservation_and_reject_crossing() {
        use crate::inventory::{
            AttemptBinding, FactRef, ResourceRequirement, WorkNeeds, WorkNeedsAttachment,
        };
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let mut bound = plan(&work, &attempt);
        let policy = content('1');
        let reference = |id: &str| FactRef {
            source: "provider-v1".into(),
            id: id.into(),
        };
        let needs = WorkNeeds {
            work_version: attempt.work_identity.as_str().into(),
            task_family: "feature".into(),
            output_contract: "candidate-v1".into(),
            definition: reference("definition"),
            profile_revision: "worker-v1".into(),
            policy_ref: policy.as_str().into(),
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
        };
        bound.execution = Some(DispatchExecutionBindings {
            requirements: WorkNeedsAttachment::new(
                attempt.work_identity.clone(),
                work.revision,
                policy,
                needs,
            )
            .unwrap_or_else(|_| unreachable!()),
            attempt: AttemptBinding {
                work_version: attempt.work_identity.as_str().into(),
                attempt: attempt.identity.as_str().into(),
                fence: attempt.fence,
                worker: reference("worker"),
                execution_grant: reference("cpu-grant"),
                model_services: vec![reference("shared-model")],
                request_admissions: vec![reference("request-grant")],
            },
            worker_request: content('a'),
        });
        bound.workspace.network = true;
        let digest =
            admit_dispatch_plan(&work, &attempt, &bound).unwrap_or_else(|_| unreachable!());
        ledger
            .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &bound)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(ledger.dispatch_plan(&attempt.identity), Some(&bound));
        let mut crossed = bound.clone();
        crossed
            .execution
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .attempt
            .fence += 1;
        assert!(admit_dispatch_plan(&work, &attempt, &crossed).is_err());
        let mut missing = bound.clone();
        missing
            .execution
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .attempt
            .request_admissions
            .clear();
        assert!(admit_dispatch_plan(&work, &attempt, &missing).is_err());
        let mut changed = bound.clone();
        changed
            .execution
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .worker_request = content('b');
        assert_ne!(
            admit_dispatch_plan(&work, &attempt, &changed).unwrap_or_else(|_| unreachable!()),
            digest
        );
        assert!(
            ledger
                .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 2, &changed)
                .is_err()
        );
        assert_eq!(ledger.dispatch_plan(&attempt.identity), Some(&bound));
    }
}
