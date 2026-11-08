//! Untrusted pull-request runs in the web service: durable blocked gates,
//! operator approval, supersession, dispatch eligibility and stored-state
//! validation. Decisions come from the pure `kiln_core::untrusted` core.

use super::api::error;
use super::model::{RequestKind, RunRecord};
use super::{ApiResponse, Engine, Role, host_time_ms, identity};
use kiln_core::domain::Outcome;
use kiln_core::untrusted::{
    ApprovalRejection, ApproveUntrustedRun, Approver, ApproverAuthority, UntrustedGate,
    UntrustedState, decide_approval, supersedes,
};
use serde::{Deserialize, Serialize};

/// Run phase of an untrusted run before an operator approves its exact commit.
pub(crate) const AWAITING_APPROVAL: &str = "awaiting_approval";
/// Error code of an unapproved run superseded by a newer commit.
pub(crate) const SUPERSEDED: &str = "untrusted_superseded";
/// Principal id prefixes of interactive operators: the operator bearer token
/// and attested tailnet bindings. Automation, webhook and embedded principals
/// hold operator capabilities internally but never approve untrusted runs.
const INTERACTIVE_OPERATOR_IDS: [&str; 1] = ["local:operator"];
const INTERACTIVE_OPERATOR_PREFIXES: [&str; 1] = ["tailscale:"];

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ApprovalRequest {
    request_id: String,
    source_commit: String,
}

// r[impl kiln.untrusted.approval]
/// The single approver role: an interactive operator principal.
pub(crate) fn authority(principal: &crate::access::Principal) -> ApproverAuthority {
    match principal.role {
        Role::Viewer => ApproverAuthority::Viewer,
        Role::Worker => ApproverAuthority::Worker,
        Role::Operator
            if INTERACTIVE_OPERATOR_IDS.contains(&principal.id.as_str())
                || INTERACTIVE_OPERATOR_PREFIXES
                    .iter()
                    .any(|prefix| principal.id.starts_with(prefix)) =>
        {
            ApproverAuthority::Operator
        }
        Role::Operator => ApproverAuthority::Automation,
    }
}

fn rejection_status(rejection: ApprovalRejection) -> u16 {
    match rejection {
        ApprovalRejection::Unauthorized => 403,
        ApprovalRejection::InvalidActor | ApprovalRejection::InvalidCommit => 400,
        ApprovalRejection::RunMismatch
        | ApprovalRejection::StaleCommit
        | ApprovalRejection::Superseded
        | ApprovalRejection::AlreadyApproved => 409,
    }
}

/// Stored runs must keep the gate bound to the exact source and phase.
pub(super) fn validate_stored(run: &RunRecord) -> Result<(), String> {
    let Some(gate) = &run.untrusted else {
        if run.phase == AWAITING_APPROVAL {
            return Err("stored approval phase without untrusted gate".into());
        }
        return Ok(());
    };
    let exact = run.source.exact_revision.native_id.as_str();
    let rebuilt = UntrustedGate::new(
        gate.reason,
        gate.pull_request,
        gate.head_repository.clone(),
        gate.source_commit.clone(),
        gate.satisfies_required_ci,
    )
    .map_err(|_| "stored untrusted gate rejected")?;
    if rebuilt.source_commit != exact
        || gate
            .approval
            .as_ref()
            .is_some_and(|approval| approval.source_commit != exact)
    {
        return Err("stored untrusted gate source conflict".into());
    }
    let consistent = match run.phase.as_str() {
        AWAITING_APPROVAL => gate.state() == UntrustedState::AwaitingApproval,
        "terminal" => true,
        _ => gate.may_dispatch(),
    };
    if !consistent {
        return Err("stored untrusted gate phase conflict".into());
    }
    Ok(())
}

impl Engine {
    /// Attach the pending gate to a new run before it is stored: blocked runs
    /// wait for approval; an inherited approval (exact retry) stays approved.
    pub(super) fn attach_untrusted(&mut self, record: &mut RunRecord) -> Result<(), String> {
        let Some(gate) = self.pending_untrusted.take() else {
            return Ok(());
        };
        if gate.source_commit != record.source.exact_revision.native_id.as_str() {
            return Err("untrusted gate does not bind the run's exact source".into());
        }
        match gate.state() {
            UntrustedState::AwaitingApproval => record.phase = AWAITING_APPROVAL.into(),
            UntrustedState::Approved => {}
            UntrustedState::Superseded => return Err("superseded untrusted gate reused".into()),
        }
        record.untrusted = Some(gate);
        Ok(())
    }

    // r[impl kiln.untrusted.blocked]
    /// A new commit on the same pull request supersedes every older unapproved
    /// gate of the same repository; those runs never dispatch. A retry re-runs
    /// its parent's exact commit, so it never supersedes another commit's gate.
    pub(super) fn supersede_untrusted(&mut self, newer_id: &str) -> Result<(), String> {
        let newer = self.state.runs.get(newer_id).ok_or("run missing")?;
        let Some(gate) = newer
            .untrusted
            .clone()
            .filter(|_| newer.parent_run_id.is_none())
        else {
            return Ok(());
        };
        let repository = newer.source.repository.clone();
        let older: Vec<String> = self
            .state
            .runs
            .values()
            .filter(|run| {
                run.id != newer_id
                    && run.phase == AWAITING_APPROVAL
                    && run.source.repository == repository
                    && run
                        .untrusted
                        .as_ref()
                        .is_some_and(|older| supersedes(&gate, older))
            })
            .map(|run| run.id.clone())
            .collect();
        let now = host_time_ms()?;
        for id in older {
            let run = self.state.runs.get_mut(&id).ok_or("run missing")?;
            if let Some(older) = run.untrusted.as_mut() {
                older.superseded_by = Some(newer_id.to_owned());
            }
            run.complete(Outcome::Cancelled, None)?;
            run.phase = "terminal".into();
            run.error_code = Some(SUPERSEDED.into());
            run.completed_at_ms = Some(now);
        }
        Ok(())
    }

    /// Dispatch only approved gates, and only to declared untrusted-capable targets.
    pub(super) fn untrusted_dispatchable(&self, run: &RunRecord) -> bool {
        run.untrusted.as_ref().is_none_or(|gate| {
            gate.may_dispatch()
                && self
                    .profile
                    .projects
                    .iter()
                    .any(|project| project.id == run.project_id && project.untrusted_capable)
        })
    }

    // r[impl kiln.untrusted.approval]
    pub(super) fn approve_untrusted(
        &mut self,
        id: &str,
        body: &[u8],
        principal: &crate::access::Principal,
    ) -> Result<ApiResponse, String> {
        let request: ApprovalRequest = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return error(400, "untrusted_approval_invalid"),
        };
        if !crate::profile::identifier(&request.request_id) {
            return error(400, "request_id_invalid");
        }
        let fingerprint = identity(&("untrusted-approval", id, &principal.id, &request))?;
        if let Some(response) = self.request_replay(&request.request_id, &fingerprint)? {
            return Ok(response);
        }
        if self.state.requests.len() >= self.profile.bounds.max_requests {
            return error(409, "request_retention");
        }
        let run = self.state.runs.get(id).ok_or("run missing")?;
        let command = ApproveUntrustedRun {
            actor: Approver {
                id: principal.id.clone(),
                label: principal.label.clone(),
                authority: authority(principal),
            },
            run: id.to_owned(),
            source_commit: request.source_commit.clone(),
        };
        if command.actor.authority != kiln_core::untrusted::APPROVER_AUTHORITY {
            return error(403, ApprovalRejection::Unauthorized.code());
        }
        let Some(gate) = &run.untrusted else {
            return error(409, "untrusted_gate_missing");
        };
        let approval = match decide_approval(id, gate, &command, host_time_ms()?) {
            Ok(approval) => approval,
            Err(rejection) => return error(rejection_status(rejection), rejection.code()),
        };
        if run.phase != AWAITING_APPROVAL {
            return error(409, ApprovalRejection::AlreadyApproved.code());
        }
        if !self
            .profile
            .projects
            .iter()
            .any(|project| project.id == run.project_id && project.untrusted_capable)
        {
            return error(409, "untrusted_runner_unavailable");
        }
        let run = self.state.runs.get_mut(id).ok_or("run missing")?;
        if let Some(gate) = run.untrusted.as_mut() {
            gate.approval = Some(approval);
        }
        run.phase = "queued".into();
        self.reserve_request(
            request.request_id.clone(),
            fingerprint,
            id.to_owned(),
            RequestKind::Control,
        )?;
        self.dispatch_queued()?;
        let value = serde_json::to_value(self.state.runs.get(id).ok_or("run missing")?.view())
            .map_err(|_| "run view encoding failed")?;
        self.finish_request(&request.request_id, 200, value)
    }

    /// Untrusted runs never restore or publish cache entries.
    pub(super) fn untrusted_cache_denied(&self, id: &str) -> Option<Result<ApiResponse, String>> {
        self.state
            .runs
            .get(id)
            .is_some_and(|run| run.untrusted.is_some())
            .then(|| error(403, "untrusted_run_cache_denied"))
    }
}
