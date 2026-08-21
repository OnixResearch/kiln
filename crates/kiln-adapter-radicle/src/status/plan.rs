use std::collections::BTreeSet;

use kiln_core::domain::Outcome;
use kiln_core::types::{CoreResult, Diagnostic, ExternalRef, RunId};
use serde::{Deserialize, Serialize};

pub const CI_JOB_TYPE_NAME: &str = "xyz.radicle.ci-job";

/// One admitted Radicle CI status publication plan.
///
/// Every field is provider-neutral. The shell maps these facts onto the
/// pinned `radicle-cob` cohort.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusPlan {
    /// The admitted repository identity (`rad:<RID>`).
    pub repository: ExternalRef,
    /// The exact ci-job object identity for an update.
    ///
    /// `None` requests first-write creation or idempotency-key discovery.
    /// The successful observation always returns the native object identity.
    pub object_id: Option<ExternalRef>,
    /// The exact current head required for a non-idempotent update.
    ///
    /// First-write and idempotent replay plans use `None`.
    pub expected_head: Option<ExternalRef>,
    /// The Kiln attempt this projection describes.
    pub run_id: RunId,
    /// The idempotency key for repeat writes of one attempt.
    pub idempotency_key: ExternalRef,
    /// Terminal outcome projected into Radicle.
    pub outcome: StatusOutcome,
    /// Maximum serialized payload bytes admitted for one write.
    pub max_payload_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusOutcome {
    Success,
    Failure,
}

pub fn classify_status_outcome(outcome: Outcome) -> CoreResult<StatusOutcome> {
    match outcome {
        Outcome::Success => Ok(StatusOutcome::Success),
        Outcome::DomainFailure
        | Outcome::Denied
        | Outcome::Cancelled
        | Outcome::TimedOut
        | Outcome::ProviderError => Ok(StatusOutcome::Failure),
        Outcome::Pending | Outcome::Unknown => Err(Diagnostic::new(
            "status_outcome_unknown",
            "status_plan.outcome",
            "Non-terminal or unknown Kiln outcome cannot be published as final status",
        )),
    }
}

pub fn admit_idempotent_replay(
    plan: &StatusPlan,
    projected_run_id: &str,
    projected_outcome: StatusOutcome,
) -> CoreResult<()> {
    if projected_run_id != plan.run_id.as_str() || projected_outcome != plan.outcome {
        return Err(Diagnostic::new(
            "status_idempotency_conflict",
            "status_plan.idempotency_key",
            "The idempotency key is already bound to different status facts",
        ));
    }
    Ok(())
}

/// Facts a fresh storage read must supply for single-writer preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedHeads {
    /// Current head identities authored on the target object.
    pub heads: Vec<HeadAuthor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadAuthor {
    pub head: ExternalRef,
    /// The node identity that authored this head, if observable.
    pub author: Option<ExternalRef>,
}

// r[impl kiln.radicle.status_allowlist]
pub fn admit_status_plan(plan: &StatusPlan, allowed_rids: &BTreeSet<String>) -> CoreResult<()> {
    if plan.repository.provider_namespace.as_str() != "radicle"
        || plan.repository.native_kind.as_str() != "repository"
        || plan.repository.native_id.as_str().is_empty()
    {
        return Err(Diagnostic::new(
            "status_repository_identity",
            "status_plan.repository",
            "Radicle status plan names an invalid repository identity",
        ));
    }
    if !allowed_rids.contains(plan.repository.native_id.as_str()) {
        return Err(Diagnostic::new(
            "status_repository_not_admitted",
            "status_plan.repository",
            "Radicle status plan targets a repository outside its allowlist",
        ));
    }
    if let Some(object_id) = &plan.object_id {
        if object_id.provider_namespace.as_str() != "radicle"
            || object_id.native_kind.as_str() != "cob"
            || object_id.native_id.as_str().is_empty()
        {
            return Err(Diagnostic::new(
                "status_object_identity",
                "status_plan.object_id",
                "Radicle status plan names an invalid COB identity",
            ));
        }
        object_id.validate()?;
    }
    if let Some(expected_head) = &plan.expected_head {
        if expected_head.provider_namespace.as_str() != "radicle"
            || expected_head.native_kind.as_str() != "head"
            || expected_head.native_id.as_str().is_empty()
        {
            return Err(Diagnostic::new(
                "status_expected_head_identity",
                "status_plan.expected_head",
                "Radicle status plan names an invalid expected head",
            ));
        }
        expected_head.validate()?;
    }
    if plan.idempotency_key.provider_namespace.as_str() != "kiln"
        || plan.idempotency_key.native_kind.as_str() != "idempotency-key"
        || plan.idempotency_key.native_id.as_str().is_empty()
    {
        return Err(Diagnostic::new(
            "status_idempotency_key",
            "status_plan.idempotency_key",
            "Radicle status plan has no bounded Kiln idempotency key",
        ));
    }
    if plan.max_payload_bytes == 0 {
        return Err(Diagnostic::new(
            "status_payload_bound",
            "status_plan.max_payload_bytes",
            "Radicle status payload bound must be positive",
        ));
    }
    Ok(())
}

// r[impl kiln.radicle.status_allowlist]
pub fn admit_expected_head(
    observed_head: Option<&ExternalRef>,
    expected_head: Option<&ExternalRef>,
) -> CoreResult<()> {
    match (observed_head, expected_head) {
        (None, None) => Ok(()),
        (Some(observed), Some(expected)) if observed == expected => Ok(()),
        _ => Err(Diagnostic::new(
            "status_stale_head",
            "status_plan.expected_head",
            "Radicle ci-job head differs from the exact planned head",
        )),
    }
}

// r[impl kiln.radicle.status_allowlist]
pub fn admit_single_writer_heads(
    observed: &ObservedHeads,
    signer_identity: &ExternalRef,
) -> CoreResult<()> {
    match observed.heads.as_slice() {
        [] => Ok(()),
        [only] => {
            only.head.validate()?;
            match &only.author {
                Some(author) if author == signer_identity => Ok(()),
                _ => Err(Diagnostic::new(
                    "status_foreign_head",
                    "status_plan.heads",
                    "Radicle ci-job head was not authored by the resolved Kiln signer",
                )),
            }
        }
        [..] => Err(Diagnostic::new(
            "status_multi_head",
            "status_plan.heads",
            "Radicle ci-job object has more than one head; single-writer policy rejects it",
        )),
    }
}
