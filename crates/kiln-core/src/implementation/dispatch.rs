//! Admission of one bounded implementation-dispatch plan.
//!
//! A plan binds the exact work/version, attempt/fence, repository/base,
//! workspace isolation, provider profile, permitted edits and effects,
//! cancellation-only controls, deadline, token/invocation budgets and expected
//! output contract. Admission produces the deterministic plan identity that the
//! attempt ledger reserves before transport.

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{ImplementationWork, content_identity, push_str};
use crate::projection::materialization::{WORKSPACE_PIN_SCHEMA, WorkspacePin};
use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef, ProviderProfileId};

pub const DISPATCH_PLAN_SCHEMA: &str = "kiln.implementation-dispatch-plan.v2";
pub const MAX_DISPATCH_PATHS: usize = 64;
pub const MAX_DISPATCH_EFFECTS: usize = 16;
pub const MAX_DISPATCH_TOKENS: u64 = 10_000_000;
pub const MAX_DISPATCH_INVOCATIONS: u32 = 1_000;
/// The only provider profile that may run a plan in an interactive worktree.
pub const INTERACTIVE_PROVIDER_PROFILE: &str = "herdr-interactive";
/// Operator policy effect that permits a work item to run in an interactive agent.
pub const INTERACTIVE_AGENT_EFFECT: &str = "interactive_agent";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceIsolation {
    /// A provider-owned disposable snapshot; no ambient host directory.
    IsolatedSnapshot,
    /// A fresh worktree at the exact base on an operator machine, driven by an
    /// interactive agent session with that machine's network and credentials.
    InteractiveWorktree,
}

impl WorkspaceIsolation {
    /// Canonical label hashed into the plan identity.
    const fn label(self) -> &'static str {
        match self {
            Self::IsolatedSnapshot => "isolated_snapshot",
            Self::InteractiveWorktree => "interactive_worktree",
        }
    }
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
    /// The projected workspace this plan prepares, when it uses one. Absent,
    /// the plan keeps the full-tree route and its earlier identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<WorkspacePin>,
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
    /// Native prepared request profile, distinct from the stable agent definition.
    /// Absence selects the explicit historical transport-v1 compatibility cohort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_request_profile: Option<ContentId>,
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

/// Durable proof that one exact request byte string was bound to an admitted plan.
/// The request embeds the plan id, so this seal is separate and persisted before transport.
pub const DISPATCH_REQUEST_BINDING_SCHEMA: &str = "kiln.implementation-dispatch-request-binding.v1";
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchRequestBinding {
    pub schema: String,
    pub attempt: ContentId,
    pub fence: u64,
    pub plan: ContentId,
    pub request_digest: ContentId,
    pub seal: ContentId,
}
impl DispatchRequestBinding {
    pub fn new(
        attempt: &ContentId,
        fence: u64,
        plan: &ContentId,
        request_digest: &ContentId,
    ) -> CoreResult<Self> {
        let mut bytes = Vec::new();
        push_str(&mut bytes, DISPATCH_REQUEST_BINDING_SCHEMA)?;
        push_str(&mut bytes, attempt.as_str())?;
        bytes.extend_from_slice(&fence.to_be_bytes());
        push_str(&mut bytes, plan.as_str())?;
        push_str(&mut bytes, request_digest.as_str())?;
        let seal = ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())?;
        Ok(Self {
            schema: DISPATCH_REQUEST_BINDING_SCHEMA.to_string(),
            attempt: attempt.clone(),
            fence,
            plan: plan.clone(),
            request_digest: request_digest.clone(),
            seal,
        })
    }
    pub fn validate_for(
        &self,
        attempt: &ContentId,
        fence: u64,
        plan: &ContentId,
        request_digest: &ContentId,
    ) -> CoreResult<()> {
        if self != &Self::new(attempt, fence, plan, request_digest)? {
            return Err(Diagnostic::new(
                "implementation_request_binding",
                "dispatch.request",
                "request bytes or plan differ from the retained dispatch seal",
            ));
        }
        Ok(())
    }
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
    // r[impl kiln.fleet.transport.scenario.admission]
    // An interactive worktree must declare the machine's ambient network and
    // credentials, and is admitted only for the interactive profile on work
    // whose operator policy permits interactive agents. The interactive profile
    // is likewise refused in any other isolation, so it never runs a plan that
    // under-declares its ambient authority.
    let interactive_profile = plan.provider_profile.as_str() == INTERACTIVE_PROVIDER_PROFILE;
    let interactive_refused = match plan.workspace.isolation {
        WorkspaceIsolation::InteractiveWorktree => {
            !interactive_profile
                || !work.permitted_effects.contains(INTERACTIVE_AGENT_EFFECT)
                || !plan.workspace.network
                || !plan.workspace.credentials
        }
        WorkspaceIsolation::IsolatedSnapshot => interactive_profile,
    };
    if interactive_refused {
        return Err(refusal(
            "implementation_dispatch_workspace",
            "dispatch.workspace",
            "interactive worktrees require the herdr-interactive profile, work permitting interactive_agent, and declared network and credentials",
        ));
    }
    if plan.workspace.isolation == WorkspaceIsolation::IsolatedSnapshot
        && (plan.workspace.credentials
            || (plan.workspace.network
                && plan.execution.as_ref().is_none_or(|execution| {
                    execution.attempt.model_services.is_empty()
                        || execution.attempt.request_admissions.is_empty()
                })))
    {
        return Err(refusal(
            "implementation_dispatch_workspace",
            "dispatch.workspace",
            "network requires explicit model-service admissions; ambient credentials are refused",
        ));
    }
    // r[impl kiln.source.projection.materialization.scenario.pinned]
    if let Some(pin) = &plan.workspace.projection
        && (pin.validate().is_err()
            || pin.attempt != plan.attempt_identity
            || pin.fence != plan.fence
            || Some(&pin.source) != plan.base.as_ref()
            || pin.overlay != plan.workspace.identity)
    {
        return Err(refusal(
            "implementation_dispatch_projection",
            "dispatch.workspace.projection",
            "workspace pin does not bind this attempt, fence, base and overlay",
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
        if execution
            .native_request_profile
            .as_ref()
            .is_some_and(|profile| profile.validate().is_err())
        {
            return Err(refusal(
                "implementation_dispatch_request_profile",
                "dispatch.execution.native_request_profile",
                "native request profile is invalid",
            ));
        }
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
    bytes.push(u8::from(plan.base.is_some()));
    if let Some(base) = &plan.base {
        push_str(&mut bytes, base.provider_namespace.as_str())?;
        push_str(&mut bytes, base.native_kind.as_str())?;
        push_str(&mut bytes, base.native_id.as_str())?;
    }
    push_str(&mut bytes, plan.workspace.identity.as_str())?;
    push_str(&mut bytes, plan.workspace.isolation.label())?;
    bytes.push(u8::from(plan.workspace.network));
    bytes.push(u8::from(plan.workspace.credentials));
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
    bytes.push(u8::from(plan.execution.is_some()));
    if let Some(execution) = &plan.execution {
        push_str(
            &mut bytes,
            if execution.native_request_profile.is_some() {
                "kiln.implementation-execution-bindings.v3"
            } else {
                "kiln.implementation-execution-bindings.v2"
            },
        )?;
        let requirements = execution.requirements.identity().map_err(|_| {
            refusal(
                "implementation_dispatch_requirements",
                "dispatch.execution",
                "requirements identity is invalid",
            )
        })?;
        push_str(&mut bytes, requirements.as_str())?;
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
        if let Some(profile) = &execution.native_request_profile {
            push_str(&mut bytes, profile.as_str())?;
        }
    }
    // Appended only when present, so a plan without a pin keeps its identity.
    if let Some(pin) = &plan.workspace.projection {
        push_str(&mut bytes, WORKSPACE_PIN_SCHEMA)?;
        for identity in [
            &pin.repository,
            &pin.definition,
            &pin.manifest,
            &pin.profile,
            &pin.input,
            &pin.overlay,
        ] {
            push_str(&mut bytes, identity.as_str())?;
        }
        push_str(&mut bytes, pin.source.provider_namespace.as_str())?;
        push_str(&mut bytes, pin.source.native_kind.as_str())?;
        push_str(&mut bytes, pin.source.native_id.as_str())?;
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
                projection: None,
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
    // r[verify kiln.implementation.lattice_cli.scenario.refusal] r[verify kiln.implementation.lattice_cli.scenario.closed]
    fn only_a_started_transport_without_execution_settles_before_effect() {
        use crate::implementation::{AttemptPhase, FailureClass};
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let dispatch = plan(&work, &attempt);
        let digest = ledger
            .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &dispatch)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .plan;
        let settle = |ledger: &mut AttemptLedger, fence: u64, cause: &str| {
            ledger.settle_dispatch_without_execution(&attempt.identity, fence, 3, cause)
        };
        // No transport intent: nothing was sent, so nothing settles.
        assert_eq!(
            settle(&mut ledger, attempt.fence, "refused").map_err(|error| error.code),
            Err("implementation_settlement_binding".to_string())
        );
        ledger
            .bind_dispatch_request(&work, &attempt.identity, attempt.fence, 2, &content('a'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(
            !ledger
                .begin_dispatch_transport(&attempt.identity, attempt.fence, 2, &digest)
                .unwrap_or_else(|error| unreachable!("{error:?}"))
        );
        assert!(settle(&mut ledger, attempt.fence + 1, "refused").is_err());
        assert!(settle(&mut ledger, attempt.fence, "").is_err());
        assert!(ledger.dispatch_requires_recovery(&attempt.identity));
        settle(
            &mut ledger,
            attempt.fence,
            "lattice_refused_before_effect:crossed_attempt",
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        let settled = ledger
            .get(&attempt.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(settled.phase, AttemptPhase::Failed);
        assert_eq!(settled.failure, Some(FailureClass::Infrastructure));
        assert_eq!(settled.workflow, None);
        assert_eq!(
            settled.cause.as_deref(),
            Some("lattice_refused_before_effect:crossed_attempt")
        );
        assert!(!ledger.dispatch_requires_recovery(&attempt.identity));
        assert_eq!(ledger.unresolved().count(), 0);
        // Terminal: neither a repeated settlement nor a new transport is admitted.
        assert!(settle(&mut ledger, attempt.fence, "refused").is_err());
        assert!(
            ledger
                .begin_dispatch_transport(&attempt.identity, attempt.fence, 4, &digest)
                .is_err()
        );
        // No unknown outcome blocks the next attempt of the same work.
        let next = ledger
            .claim(&work, 5, DEFAULT_LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(next.fence > attempt.fence);

        // A recorded execution can never be settled as no execution.
        let mut executed = AttemptLedger::default();
        let attempt = executed
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let dispatch = plan(&work, &attempt);
        let digest = executed
            .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &dispatch)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .plan;
        executed
            .bind_dispatch_request(&work, &attempt.identity, attempt.fence, 2, &content('a'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        executed
            .begin_dispatch_transport(&attempt.identity, attempt.fence, 2, &digest)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        executed
            .reconcile_dispatch(
                &attempt.identity,
                attempt.fence,
                3,
                &ExternalRef::new("lattice", "implementation-attempt", "one")
                    .unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(
            executed
                .settle_dispatch_without_execution(&attempt.identity, attempt.fence, 4, "closed")
                .is_err()
        );
        assert_eq!(
            executed.get(&attempt.identity).map(|attempt| attempt.phase),
            Some(AttemptPhase::Executing)
        );
        assert!(executed.dispatch_requires_recovery(&attempt.identity));
    }

    #[test]
    // r[verify kiln.implementation.lattice_cli.scenario.bound_root]
    fn transport_launches_record_once_and_recall_only_for_an_existing_intent() {
        use crate::implementation::{TransportLaunch, TransportLaunches, TransportStart};
        let launch = |root: &str| TransportLaunch {
            lattice_program: "/nix/store/a-lattice/bin/lattice".into(),
            executable_blake3: content('b'),
            state_directory: root.into(),
        };
        let attempt = content('a');
        let mut launches = TransportLaunches::default();
        // A fresh intent records its launch.
        assert_eq!(
            launches.start(&attempt, false, Some(&launch("/var/lib/lattice/b"))),
            Ok(TransportStart::Fresh)
        );
        // A later caller under another profile recovers with the recorded
        // launch, which is never overwritten.
        assert_eq!(
            launches.start(&attempt, true, Some(&launch("/var/lib/lattice/a"))),
            Ok(TransportStart::Recover(Some(launch("/var/lib/lattice/b"))))
        );
        // A fresh intent never replaces a different record.
        assert!(
            launches
                .start(&attempt, false, Some(&launch("/var/lib/lattice/a")))
                .is_err()
        );
        assert_eq!(
            launches.recall(&attempt),
            Some(launch("/var/lib/lattice/b"))
        );
        // A legacy intent (no record) recovers with none and gains none.
        let legacy = content('c');
        assert_eq!(
            launches.start(&legacy, true, Some(&launch("/var/lib/lattice/a"))),
            Ok(TransportStart::Recover(None))
        );
        assert_eq!(launches.recall(&legacy), None);
        // Settlement removes the record; the encoding is strict.
        launches.remove(&attempt);
        assert_eq!(launches.recall(&attempt), None);
        let encoded = serde_json::to_value(&launches).unwrap_or_else(|_| unreachable!());
        let reloaded: TransportLaunches =
            serde_json::from_value(encoded).unwrap_or_else(|_| unreachable!());
        assert_eq!(reloaded, launches);
        assert!(serde_json::from_str::<TransportLaunches>(r#"{"launches":{},"extra":1}"#).is_err());
    }

    #[test]
    // r[verify kiln.implementation.lattice_cli.scenario.bound_root]
    fn the_attempt_ledger_keeps_its_published_document_shape() {
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let digest = ledger
            .reserve_dispatch_plan(
                &work,
                &attempt.identity,
                attempt.fence,
                1,
                &plan(&work, &attempt),
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .plan;
        ledger
            .bind_dispatch_request(&work, &attempt.identity, attempt.fence, 2, &content('a'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        ledger
            .begin_dispatch_transport(&attempt.identity, attempt.fence, 3, &digest)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let encoded = serde_json::to_value(&ledger).unwrap_or_else(|_| unreachable!());
        let keys: BTreeSet<&str> = encoded
            .as_object()
            .unwrap_or_else(|| unreachable!())
            .keys()
            .map(String::as_str)
            .collect();
        // The fields of `AttemptLedger` as published at 339116f6.
        assert_eq!(
            keys,
            BTreeSet::from([
                "fence_counter",
                "attempts",
                "current",
                "consumed",
                "resolved_unknown",
                "dispatch_started",
                "dispatch_plans",
            ])
        );
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
            native_request_profile: None,
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
        });
        bound.workspace.network = true;
        let digest =
            admit_dispatch_plan(&work, &attempt, &bound).unwrap_or_else(|_| unreachable!());
        ledger
            .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &bound)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(ledger.dispatch_plan(&attempt.identity), Some(&bound));
        let request_a = content('a');
        let sealed = ledger
            .bind_dispatch_request(&work, &attempt.identity, attempt.fence, 2, &request_a)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(sealed.plan, digest);
        assert!(
            sealed
                .validate_for(&attempt.identity, attempt.fence, &digest, &request_a)
                .is_ok()
        );
        assert!(
            sealed
                .validate_for(&attempt.identity, attempt.fence, &digest, &content('b'))
                .is_err()
        );
        assert!(
            ledger
                .bind_dispatch_request(&work, &attempt.identity, attempt.fence, 3, &content('b'))
                .is_err()
        );
        let mut native_profile_changed = bound.clone();
        native_profile_changed
            .execution
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .native_request_profile = Some(content('c'));
        let native_digest = admit_dispatch_plan(&work, &attempt, &native_profile_changed)
            .unwrap_or_else(|_| unreachable!());
        assert!(
            sealed
                .validate_for(&attempt.identity, attempt.fence, &native_digest, &request_a)
                .is_err()
        );
        assert!(
            ledger
                .reserve_dispatch_plan(
                    &work,
                    &attempt.identity,
                    attempt.fence,
                    3,
                    &native_profile_changed
                )
                .is_err()
        );
        assert!(
            !ledger
                .begin_dispatch_transport(&attempt.identity, attempt.fence, 4, &digest)
                .unwrap_or_else(|_| unreachable!())
        );
        assert!(
            ledger
                .begin_dispatch_transport(&attempt.identity, attempt.fence, 5, &digest)
                .unwrap_or_else(|_| unreachable!())
        );
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
        changed.workspace.identity = content('e');
        assert_ne!(
            admit_dispatch_plan(&work, &attempt, &changed).unwrap_or_else(|_| unreachable!()),
            digest
        );
        let mut no_network = bound.clone();
        no_network.workspace.network = false;
        assert_ne!(
            admit_dispatch_plan(&work, &attempt, &no_network).unwrap_or_else(|_| unreachable!()),
            digest
        );
        assert!(
            ledger
                .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 2, &changed)
                .is_err()
        );
        assert_eq!(ledger.dispatch_plan(&attempt.identity), Some(&bound));
    }

    fn interactive_plan(
        work: &ImplementationWork,
        attempt: &super::super::ImplementationAttempt,
    ) -> ImplementationDispatchPlan {
        let mut plan = plan(work, attempt);
        plan.workspace.isolation = WorkspaceIsolation::InteractiveWorktree;
        plan.workspace.network = true;
        plan.workspace.credentials = true;
        plan.provider_profile =
            ProviderProfileId::new(INTERACTIVE_PROVIDER_PROFILE).unwrap_or_else(|_| unreachable!());
        plan
    }

    fn interactive_work() -> ImplementationWork {
        let mut work = work();
        work.permitted_effects
            .insert(INTERACTIVE_AGENT_EFFECT.to_string());
        work
    }

    #[test]
    fn queued_rebind_refuses_sealed_or_started_transport_and_preserves_plan_controls()
    -> CoreResult<()> {
        use crate::fleet::placement::MoveRequest;

        let work = interactive_work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger.claim(&work, 0, DEFAULT_LEASE_MS)?;
        let old = interactive_plan(&work, &attempt);
        let reservation =
            ledger.reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &old)?;
        let request = MoveRequest {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            reservation: reservation.effect_id,
            destination: "destination".into(),
        };
        assert!(
            ledger
                .rebind_queued_dispatch(&work, &request, 2, old.workspace.identity.clone())
                .is_err()
        );
        let mut rebind_won = ledger.clone();
        let moved = rebind_won.rebind_queued_dispatch(&work, &request, 2, content('c'))?;
        let mut expected = old.clone();
        expected.workspace.identity = content('c');
        assert_eq!(rebind_won.dispatch_plan(&attempt.identity), Some(&expected));
        assert_ne!(moved.plan, reservation.plan);
        assert!(
            rebind_won
                .begin_dispatch_transport(&attempt.identity, attempt.fence, 2, &reservation.plan)
                .is_err()
        );

        ledger.bind_dispatch_request(&work, &attempt.identity, attempt.fence, 2, &content('a'))?;
        assert!(
            ledger
                .rebind_queued_dispatch(&work, &request, 2, content('c'))
                .is_err()
        );
        assert!(!ledger.begin_dispatch_transport(
            &attempt.identity,
            attempt.fence,
            2,
            &reservation.plan
        )?);
        assert!(
            ledger
                .rebind_queued_dispatch(&work, &request, 2, content('c'))
                .is_err()
        );
        assert_eq!(ledger.dispatch_plan(&attempt.identity), Some(&old));
        Ok(())
    }

    #[test]
    // r[verify kiln.fleet.transport.scenario.admission]
    fn interactive_plans_need_the_exact_profile_effect_and_declared_authority() {
        let work = interactive_work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let interactive = interactive_plan(&work, &attempt);
        let identity = admit_dispatch_plan(&work, &attempt, &interactive)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let reservation = ledger
            .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &interactive)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(reservation.plan, identity);
        // The isolation is part of the identity: the same plan in a snapshot
        // differs, and a Lattice plan on the same work is still admitted.
        let lattice = plan(&work, &attempt);
        let lattice_identity = admit_dispatch_plan(&work, &attempt, &lattice)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_ne!(lattice_identity, identity);

        type PlanChange = fn(&mut ImplementationDispatchPlan);
        let refused: [(&str, PlanChange); 6] = [
            ("another profile", |plan| {
                plan.provider_profile = ProviderProfileId::new("lattice-implementation")
                    .unwrap_or_else(|_| unreachable!());
            }),
            ("undeclared network", |plan| plan.workspace.network = false),
            ("undeclared credentials", |plan| {
                plan.workspace.credentials = false;
            }),
            ("snapshot with credentials", |plan| {
                plan.workspace.isolation = WorkspaceIsolation::IsolatedSnapshot;
            }),
            ("snapshot with the interactive profile", |plan| {
                plan.workspace.isolation = WorkspaceIsolation::IsolatedSnapshot;
                plan.workspace.network = false;
                plan.workspace.credentials = false;
            }),
            ("lattice profile with credentials", |plan| {
                plan.workspace.isolation = WorkspaceIsolation::IsolatedSnapshot;
                plan.provider_profile = ProviderProfileId::new("lattice-implementation")
                    .unwrap_or_else(|_| unreachable!());
            }),
        ];
        for (name, change) in refused {
            let mut plan = interactive.clone();
            change(&mut plan);
            assert_eq!(
                admit_dispatch_plan(&work, &attempt, &plan).map_err(|error| error.code),
                Err("implementation_dispatch_workspace".to_string()),
                "{name}"
            );
        }

        // Work whose operator policy does not permit interactive agents.
        let plain = self::work();
        let mut plain_ledger = AttemptLedger::default();
        let plain_attempt = plain_ledger
            .claim(&plain, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            admit_dispatch_plan(
                &plain,
                &plain_attempt,
                &interactive_plan(&plain, &plain_attempt)
            )
            .map_err(|error| error.code),
            Err("implementation_dispatch_workspace".to_string())
        );
        assert!(
            plain_ledger
                .reserve_dispatch_plan(
                    &plain,
                    &plain_attempt.identity,
                    plain_attempt.fence,
                    1,
                    &interactive_plan(&plain, &plain_attempt)
                )
                .is_err()
        );
        // Lattice rules are unchanged on that work.
        assert_eq!(
            admit_dispatch_plan(&plain, &plain_attempt, &plan(&plain, &plain_attempt))
                .map(|identity| identity.as_str().to_string())
                .map_err(|error| error.code),
            Ok("304916eeffed3042a8c0089ef23e2a82637c01dba1a2fa90d79378c7d83ce164".to_string())
        );
    }

    #[test]
    // r[verify kiln.source.projection.materialization.scenario.pinned]
    fn a_plan_without_a_workspace_pin_keeps_its_identity() {
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let identity = admit_dispatch_plan(&work, &attempt, &plan(&work, &attempt))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        // Computed before workspace pins existed; a plan without one keeps it.
        assert_eq!(
            identity.as_str(),
            "304916eeffed3042a8c0089ef23e2a82637c01dba1a2fa90d79378c7d83ce164"
        );
    }

    #[test]
    // r[verify kiln.source.projection.materialization.scenario.pinned]
    fn a_workspace_pin_binds_the_attempt_fence_base_and_overlay() {
        use crate::projection::materialization::overlay_identity;
        // A self-consistent pin for whatever attempt and fence it names.
        fn regenerate(plan: &mut ImplementationDispatchPlan) {
            if let Some(pin) = plan.workspace.projection.as_mut() {
                pin.overlay = overlay_identity(&pin.input, &pin.attempt, pin.fence)
                    .unwrap_or_else(|_| unreachable!());
                plan.workspace.identity = pin.overlay.clone();
            }
        }
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let unpinned = plan(&work, &attempt);
        let unpinned_identity = admit_dispatch_plan(&work, &attempt, &unpinned)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let mut pinned = unpinned.clone();
        pinned.workspace.projection = Some(WorkspacePin {
            schema: WORKSPACE_PIN_SCHEMA.to_string(),
            repository: content('1'),
            source: work.base.clone().unwrap_or_else(|| unreachable!()),
            definition: content('2'),
            manifest: content('3'),
            profile: content('4'),
            input: content('5'),
            overlay: content('5'),
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
        });
        regenerate(&mut pinned);
        let identity = admit_dispatch_plan(&work, &attempt, &pinned)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_ne!(identity, unpinned_identity);
        let reservation = ledger
            .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &pinned)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(reservation.plan, identity);
        let mut other_manifest = pinned.clone();
        if let Some(pin) = other_manifest.workspace.projection.as_mut() {
            pin.manifest = content('6');
        }
        assert_ne!(
            admit_dispatch_plan(&work, &attempt, &other_manifest)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            identity
        );

        type PlanChange = fn(&mut ImplementationDispatchPlan);
        let refused: [(&str, PlanChange); 6] = [
            ("another attempt", |plan| {
                if let Some(pin) = plan.workspace.projection.as_mut() {
                    pin.attempt = content('a');
                }
                regenerate(plan);
            }),
            ("another fence", |plan| {
                if let Some(pin) = plan.workspace.projection.as_mut() {
                    pin.fence += 1;
                }
                regenerate(plan);
            }),
            ("another base", |plan| {
                if let Some(pin) = plan.workspace.projection.as_mut() {
                    pin.source = ExternalRef::new("git", "commit", "other")
                        .unwrap_or_else(|_| unreachable!());
                }
            }),
            ("workspace is not the overlay", |plan| {
                plan.workspace.identity = content('e');
            }),
            ("inconsistent overlay", |plan| {
                if let Some(pin) = plan.workspace.projection.as_mut() {
                    pin.overlay = content('e');
                }
                plan.workspace.identity = content('e');
            }),
            ("schema", |plan| {
                if let Some(pin) = plan.workspace.projection.as_mut() {
                    pin.schema = "kiln.materialization.workspace-pin.v0".to_string();
                }
            }),
        ];
        for (name, change) in refused {
            let mut plan = pinned.clone();
            change(&mut plan);
            assert_eq!(
                admit_dispatch_plan(&work, &attempt, &plan).map_err(|error| error.code),
                Err("implementation_dispatch_projection".to_string()),
                "{name}"
            );
        }
    }
}
