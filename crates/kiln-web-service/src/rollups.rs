//! Retained read-only result roll-ups across `#97` runbook dispatches and `#98` parent fan-out runs.
//!
//! Projects retained run records, Patchgate review receipts (`kiln.patchgate.review.v1`),
//! verification receipts, read-only `ImplementationObservation` facts, and validated `https://`
//! PR / stack links without mutating state or claiming review, merge, gate, or release authority.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kiln_core::domain::Outcome;
use kiln_core::implementation::{
    FanOutRun, FanoutAdmissionState, FanoutChildStatus, PublicationPhase, VerificationOutcome,
};
use kiln_implementer::observation::ImplementationObservation;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engine::model::{ReportRecord, RunRecord, SummaryDeliveryRecord};
use crate::reports::ReportKind;
use crate::runbooks::{RunbookMode, RunbookRegistry};

pub const RUNBOOK_DISPATCHES_VIEW_SCHEMA: &str = "kiln.runbook-dispatches-view.v1";
pub const RUNBOOK_ROLLUP_SCHEMA: &str = "kiln.runbook-rollup.v1";
pub const RUNBOOK_SUMMARY_NOTIFICATION_SCHEMA: &str = "kiln.runbook-summary-notification.v1";
pub const PATCHGATE_RECEIPT_SCHEMA: &str = "kiln.patchgate.review.v1";

pub const ROLLUP_CLAIM_BOUNDARY: &str =
    "Evidence only; grants no review, acceptance, merge, gate, or release authority.";

pub const ROLLUP_NON_CLAIMS: [&str; 3] = [
    "Result roll-ups project retained run, receipt, and publication records only; they grant no merge, approval, gate, or release authority.",
    "Outcome::Unknown and missing or unreadable receipts remain explicit and are never inferred as passing.",
    "Observed pull request or stack links record producer or implementation state only and do not establish landing or CI acceptance.",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunbookDispatchSummary {
    pub run_id: String,
    pub ordinal: u64,
    pub runbook_id: String,
    pub mode: RunbookMode,
    pub target: String,
    pub project_id: String,
    pub source_revision: String,
    pub outcome: Outcome,
    pub phase: String,
    pub status: &'static str,
    pub lattice_bundle_id: Option<String>,
    pub lattice_workflow_name: String,
    pub lattice_workflow_revision: String,
    pub params_identity: String,
    pub created_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub patchgate_receipt_state: &'static str,
    pub verification_receipt_state: &'static str,
    pub pull_request_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunbookDispatchPage {
    pub schema: &'static str,
    pub dispatches: Vec<RunbookDispatchSummary>,
    pub next_cursor: Option<String>,
    pub total: usize,
    pub bounded: bool,
    pub grants_authority: bool,
    pub non_claims: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RollupPatchgateReceipt {
    pub state: &'static str,
    pub unavailable_reason: Option<String>,
    pub report_id: Option<String>,
    pub report_name: Option<String>,
    pub node_id: Option<String>,
    pub source_blake3: Option<String>,
    pub artifact_download_path: Option<String>,
    pub effect_id: Option<String>,
    pub candidate_id: Option<String>,
    pub candidate_root: Option<String>,
    pub report_identity: Option<String>,
    pub isolated: Option<bool>,
    pub authorizes_acceptance: Option<bool>,
    pub grants_authority: bool,
    pub reviews_count: usize,
    pub decision_counts: BTreeMap<String, usize>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RollupTestSummary {
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub errors: u64,
    pub skipped: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RollupCoverageSummary {
    pub format: String,
    pub lines_found: u64,
    pub lines_hit: u64,
    pub line_rate: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RollupVerificationReportRef {
    pub report_id: String,
    pub kind: ReportKind,
    pub name: String,
    pub node_id: String,
    pub source_blake3: Option<String>,
    pub artifact_download_path: Option<String>,
    pub unavailable: Option<String>,
    pub tests_summary: Option<RollupTestSummary>,
    pub coverage_summary: Option<RollupCoverageSummary>,
    pub evidence_claim_boundary: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RollupIndependentVerificationRef {
    pub run_id: String,
    pub outcome: Outcome,
    pub phase: String,
    pub subject_hash: String,
    pub independent_receipt: Option<String>,
    pub artifact_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RollupImplementationVerificationRef {
    pub attempt_id: String,
    pub run_id: String,
    pub success: bool,
    pub full_run: bool,
    pub clean: bool,
    pub terminal: bool,
    pub verification_outcome: Option<String>,
    pub checks_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RollupVerificationReceipt {
    pub state: &'static str,
    pub unavailable_reason: Option<String>,
    pub outcome: Option<String>,
    pub grants_authority: bool,
    pub implementation_observation_status: &'static str,
    pub run_reports: Vec<RollupVerificationReportRef>,
    pub independent_verifications: Vec<RollupIndependentVerificationRef>,
    pub implementation_verification: Option<RollupImplementationVerificationRef>,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RollupPullRequestLink {
    pub url: String,
    pub kind: &'static str,
    pub state: String,
    pub number: Option<u64>,
    pub source: &'static str,
    pub reference: Option<String>,
    pub branch: Option<String>,
    pub base_branch: Option<String>,
    pub merge_commit: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RollupTargetView {
    pub index: u16,
    pub project_id: String,
    pub repository: String,
    pub target: String,
    pub resolved_reference: String,
    pub source_revision: String,
    pub canonical_tree: String,
    pub write_scope: Option<String>,
    pub status: &'static str,
    pub run_id: Option<String>,
    pub run_outcome: Option<Outcome>,
    pub run_phase: Option<String>,
    pub error_code: Option<String>,
    pub reason: Option<String>,
    pub patchgate_receipt: RollupPatchgateReceipt,
    pub verification_receipt: RollupVerificationReceipt,
    pub pull_request_state: &'static str,
    pub pull_requests: Vec<RollupPullRequestLink>,
    pub admitted_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RollupCounts {
    pub total: usize,
    pub pending: usize,
    pub active: usize,
    pub success: usize,
    pub failure: usize,
    pub unknown: usize,
    pub skipped: usize,
    pub patchgate_present: usize,
    pub patchgate_missing: usize,
    pub patchgate_unrun: usize,
    pub patchgate_unavailable: usize,
    pub verification_present: usize,
    pub verification_missing: usize,
    pub verification_unrun: usize,
    pub verification_unavailable: usize,
    pub pull_requests_observed: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RollupNotificationDeliveryView {
    pub registration_id: String,
    pub delivery_id: String,
    pub status: String,
    pub reserved_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RollupNotificationStatus {
    pub state: &'static str,
    pub delivery_id: Option<String>,
    pub completed_at_ms: Option<u64>,
    pub error_code: Option<String>,
    pub deliveries: Vec<RollupNotificationDeliveryView>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RunbookRollupPage {
    pub schema: &'static str,
    pub subject_kind: &'static str,
    pub subject_id: String,
    pub runbook_id: String,
    pub mode: RunbookMode,
    pub run_id: Option<String>,
    pub fanout_id: Option<String>,
    pub principal: Option<String>,
    pub phase: String,
    pub overall_outcome: Option<Outcome>,
    pub has_unknown: bool,
    pub admission: Option<FanoutAdmissionState>,
    pub stop_reason: Option<String>,
    pub lattice_bundle_id: Option<String>,
    pub lattice_bundle_identity: String,
    pub lattice_workflow_name: String,
    pub lattice_workflow_revision: String,
    pub params_identity: String,
    pub counts: RollupCounts,
    pub notification: RollupNotificationStatus,
    pub targets: Vec<RollupTargetView>,
    pub next_cursor: Option<String>,
    pub total: usize,
    pub bounded: bool,
    pub grants_authority: bool,
    pub non_claims: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunbookSummaryNotificationTarget {
    pub index: u16,
    pub project_id: String,
    pub target: String,
    pub source_revision: String,
    pub status: String,
    pub run_outcome: Option<Outcome>,
    pub patchgate_state: String,
    pub patchgate_decision_counts: BTreeMap<String, usize>,
    pub verification_state: String,
    pub verification_outcome: Option<String>,
    pub pull_request_urls: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunbookSummaryNotificationBody {
    pub schema: String,
    pub idempotency_key: String,
    pub notification_policy: String,
    pub subject_kind: String,
    pub subject_id: String,
    pub runbook_id: String,
    pub mode: RunbookMode,
    pub phase: String,
    pub overall_outcome: Option<Outcome>,
    pub has_unknown: bool,
    pub counts: SummaryNotificationCounts,
    pub targets_truncated: bool,
    pub targets: Vec<RunbookSummaryNotificationTarget>,
    pub grants_authority: bool,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SummaryNotificationCounts {
    pub total: usize,
    pub pending: usize,
    pub active: usize,
    pub success: usize,
    pub failure: usize,
    pub unknown: usize,
    pub skipped: usize,
}

#[derive(Clone, Debug, Default)]
pub enum ImplementationWorkIndex {
    #[default]
    Unconfigured,
    Unavailable,
    Available {
        by_revision: BTreeMap<String, Vec<AttemptObservation>>,
    },
}

#[derive(Clone, Debug)]
pub struct AttemptObservation {
    pub project: Option<String>,
    pub repository: Option<String>,
    pub publication_repository: Option<String>,
    pub verification: Option<RollupImplementationVerificationRef>,
    pub pull_requests: Vec<RollupPullRequestLink>,
}

impl ImplementationWorkIndex {
    #[must_use]
    pub fn observation_status(&self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Unavailable => "unavailable",
            Self::Available { .. } => "available",
        }
    }

    pub fn lookup(
        &self,
        project_id: &str,
        repository: &str,
        source_revision: &str,
    ) -> Vec<&AttemptObservation> {
        let Self::Available { by_revision } = self else {
            return Vec::new();
        };
        let Some(entries) = by_revision.get(source_revision) else {
            return Vec::new();
        };
        entries
            .iter()
            .filter(|entry| {
                entry.project.as_deref() == Some(project_id)
                    || entry
                        .repository
                        .as_deref()
                        .is_some_and(|repo| repo.eq_ignore_ascii_case(repository))
                    || entry
                        .publication_repository
                        .as_deref()
                        .is_some_and(|repo| repo.eq_ignore_ascii_case(repository))
            })
            .collect()
    }
}

#[must_use]
pub fn load_implementation_work_index(path: &Path) -> ImplementationWorkIndex {
    let Ok(observation) = ImplementationObservation::open(path) else {
        return ImplementationWorkIndex::Unavailable;
    };
    let Ok(snapshot) = observation.snapshot() else {
        return ImplementationWorkIndex::Unavailable;
    };

    let mut work_by_identity: BTreeMap<String, (String, String)> = BTreeMap::new();
    if let Some(work_ledger) = &snapshot.work {
        for work in work_ledger.current().chain(work_ledger.superseded().iter()) {
            if let Ok(identity) = work.identity() {
                work_by_identity.insert(
                    identity.as_str().to_owned(),
                    (work.project.clone(), work.repository.clone()),
                );
            }
        }
    }

    let mut by_revision: BTreeMap<String, Vec<AttemptObservation>> = BTreeMap::new();
    if let Some(attempts) = &snapshot.attempts {
        for attempt in attempts.attempts() {
            let Some(candidate) = &attempt.candidate else {
                continue;
            };
            let revision = candidate.native_id.as_str().to_owned();
            let (project, repository) = work_by_identity
                .get(attempt.work_identity.as_str())
                .cloned()
                .map_or((None, None), |(p, r)| (Some(p), Some(r)));
            let verification = attempt.verification.as_ref().map(|v| {
                let verification_outcome = attempt.verification_outcome.as_ref().map(|o| match o {
                    VerificationOutcome::Verified => "Verified".to_owned(),
                    VerificationOutcome::Incomplete => "Incomplete".to_owned(),
                    VerificationOutcome::Rejected => "Rejected".to_owned(),
                    VerificationOutcome::Unknown => "Unknown".to_owned(),
                });
                RollupImplementationVerificationRef {
                    attempt_id: attempt.identity.as_str().to_owned(),
                    run_id: v.run.run_id.clone(),
                    success: v.run.success,
                    full_run: v.run.full_run,
                    clean: v.run.clean,
                    terminal: v.run.terminal,
                    verification_outcome,
                    checks_count: v.run.checks.len(),
                }
            });
            let mut publication_repository = None;
            let mut pull_requests = Vec::new();
            if let Some(pub_rec) = &attempt.publication {
                publication_repository = Some(pub_rec.plan.repository.clone());
                if let Some(pr_ref) = &pub_rec.pull_request
                    && let Some(link) = pull_request_link_from_publication(pub_rec, pr_ref)
                {
                    pull_requests.push(link);
                }
            }
            if verification.is_some() || !pull_requests.is_empty() {
                by_revision
                    .entry(revision)
                    .or_default()
                    .push(AttemptObservation {
                        project,
                        repository,
                        publication_repository,
                        verification,
                        pull_requests,
                    });
            }
        }
    }

    ImplementationWorkIndex::Available { by_revision }
}

fn pull_request_link_from_publication(
    pub_rec: &kiln_core::implementation::PublicationRecord,
    pr_ref: &kiln_core::types::ExternalRef,
) -> Option<RollupPullRequestLink> {
    if pr_ref.provider_namespace.as_str() != "github"
        || pr_ref.native_kind.as_str() != "pull_request"
    {
        return None;
    }
    let raw = pr_ref.native_id.as_str();
    let (repo_part, num_part) = raw.split_once('#')?;
    let (owner, repo) = repo_part.split_once('/')?;
    if !valid_github_segment(owner) || !valid_github_segment(repo) {
        return None;
    }
    let number: u64 = num_part.parse().ok().filter(|n| *n > 0)?;
    let candidate_url = format!("https://github.com/{owner}/{repo}/pull/{number}");
    let url = validate_https_url(&candidate_url)?;
    let state = if pub_rec.integration.is_some() || pub_rec.phase == PublicationPhase::Integrated {
        "merged".to_owned()
    } else if let Some(status) = &pub_rec.source_status {
        normalize_pr_state(&status.state)
    } else {
        match pub_rec.phase {
            PublicationPhase::PullRequestPublished | PublicationPhase::SourceReconciled => {
                "open".to_owned()
            }
            PublicationPhase::Prepared | PublicationPhase::BranchPublished => {
                "unobserved".to_owned()
            }
            PublicationPhase::Integrated => "merged".to_owned(),
        }
    };
    Some(RollupPullRequestLink {
        url,
        kind: "pull_request",
        state,
        number: Some(number),
        source: "implementation_publication",
        reference: Some(raw.to_owned()),
        branch: Some(pub_rec.plan.branch.clone()),
        base_branch: Some(pub_rec.plan.base_branch.clone()),
        merge_commit: pub_rec
            .integration
            .as_ref()
            .map(|r| r.merge_commit.native_id.as_str().to_owned()),
    })
}

fn valid_github_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= 100
        && segment != "."
        && segment != ".."
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

#[must_use]
pub fn validate_https_url(raw: &str) -> Option<String> {
    if raw.is_empty()
        || raw.len() > 2048
        || raw.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return None;
    }
    let url = reqwest::Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
    {
        return None;
    }
    Some(url.into())
}

fn normalize_pr_state(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "open" | "opened" | "reopened" => "open".to_owned(),
        "closed" => "closed".to_owned(),
        "merged" | "integrated" => "merged".to_owned(),
        "draft" => "draft".to_owned(),
        "unobserved" | "none" => "unobserved".to_owned(),
        _ => "unknown".to_owned(),
    }
}

#[must_use]
pub fn outcome_to_rollup_status(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pending => "pending",
        Outcome::Success => "success",
        Outcome::DomainFailure
        | Outcome::Denied
        | Outcome::Cancelled
        | Outcome::TimedOut
        | Outcome::ProviderError => "failure",
        Outcome::Unknown => "unknown",
    }
}

#[must_use]
pub fn fanout_status_str(status: FanoutChildStatus) -> &'static str {
    match status {
        FanoutChildStatus::Pending => "pending",
        FanoutChildStatus::Active => "active",
        FanoutChildStatus::Success => "success",
        FanoutChildStatus::Failure => "failure",
        FanoutChildStatus::Unknown => "unknown",
        FanoutChildStatus::Skipped => "skipped",
    }
}

fn target_is_unrun(target_status: &str, run: Option<&RunRecord>) -> bool {
    let Some(run) = run else {
        return true;
    };
    if matches!(target_status, "pending" | "skipped") {
        return true;
    }
    run.outcome() == Outcome::Pending
}

#[must_use]
pub fn extract_patchgate_receipt(
    run: Option<&RunRecord>,
    target_status: &str,
) -> RollupPatchgateReceipt {
    let Some(run) = run else {
        return empty_patchgate_receipt("unrun", None);
    };
    let mut unavailable_reason: Option<String> = None;
    let mut matched_record: Option<&ReportRecord> = None;

    for record in &run.reports {
        if record.kind != ReportKind::Review {
            continue;
        }
        if let Some(reason) = &record.unavailable {
            if unavailable_reason.is_none() {
                unavailable_reason = Some(reason.clone());
            }
            continue;
        }
        let Some(report) = &record.report else {
            continue;
        };
        let Some(evidence) = &report.evidence else {
            continue;
        };
        if evidence.document.get("schema").and_then(Value::as_str) == Some(PATCHGATE_RECEIPT_SCHEMA)
        {
            matched_record = Some(record);
            break;
        }
    }

    if let Some(record) = matched_record
        && let Some(report) = &record.report
        && let Some(evidence) = &report.evidence
    {
        let observation = evidence.document.get("observation").unwrap_or(&Value::Null);
        let reviews = observation
            .get("reviews")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut decision_counts = BTreeMap::new();
        for review in reviews {
            let decision = review
                .get("decision")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            *decision_counts.entry(decision).or_default() += 1;
        }
        return RollupPatchgateReceipt {
            state: "present",
            unavailable_reason: None,
            report_id: Some(record.id.clone()),
            report_name: Some(record.name.clone()),
            node_id: Some(record.node_id.clone()),
            source_blake3: Some(report.source_blake3.clone()),
            artifact_download_path: record.artifact.as_ref().map(|a| a.download_path.clone()),
            effect_id: observation
                .get("effect_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
            candidate_id: observation
                .get("candidate_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
            candidate_root: observation
                .get("candidate_root")
                .and_then(Value::as_str)
                .map(str::to_owned),
            report_identity: observation
                .get("report_identity")
                .and_then(Value::as_str)
                .map(str::to_owned),
            isolated: observation.get("isolated").and_then(Value::as_bool),
            authorizes_acceptance: observation
                .get("authorizes_acceptance")
                .and_then(Value::as_bool),
            grants_authority: false,
            reviews_count: reviews.len(),
            decision_counts,
            claim_boundary: evidence.claim_boundary.clone(),
        };
    }

    if let Some(reason) = unavailable_reason {
        return empty_patchgate_receipt("unavailable", Some(reason));
    }
    if target_is_unrun(target_status, Some(run)) {
        return empty_patchgate_receipt("unrun", None);
    }
    empty_patchgate_receipt("missing", None)
}

fn empty_patchgate_receipt(
    state: &'static str,
    unavailable_reason: Option<String>,
) -> RollupPatchgateReceipt {
    RollupPatchgateReceipt {
        state,
        unavailable_reason,
        report_id: None,
        report_name: None,
        node_id: None,
        source_blake3: None,
        artifact_download_path: None,
        effect_id: None,
        candidate_id: None,
        candidate_root: None,
        report_identity: None,
        isolated: None,
        authorizes_acceptance: None,
        grants_authority: false,
        reviews_count: 0,
        decision_counts: BTreeMap::new(),
        claim_boundary: ROLLUP_CLAIM_BOUNDARY.to_owned(),
    }
}

pub fn extract_verification_receipt(
    run: Option<&RunRecord>,
    target_status: &str,
    project_id: &str,
    repository: &str,
    source_revision: &str,
    all_runs: &BTreeMap<String, RunRecord>,
    impl_work: &ImplementationWorkIndex,
) -> RollupVerificationReceipt {
    let mut run_reports = Vec::new();
    let mut unavailable_reason: Option<String> = None;

    if let Some(run) = run {
        for record in &run.reports {
            let is_patchgate = record.kind == ReportKind::Review
                && record
                    .report
                    .as_ref()
                    .and_then(|r| r.evidence.as_ref())
                    .is_some_and(|e| {
                        e.document.get("schema").and_then(Value::as_str)
                            == Some(PATCHGATE_RECEIPT_SCHEMA)
                    });
            if is_patchgate {
                continue;
            }
            if !matches!(
                record.kind,
                ReportKind::Gate
                    | ReportKind::Provenance
                    | ReportKind::Junit
                    | ReportKind::Cobertura
                    | ReportKind::Lcov
            ) {
                continue;
            }
            if let Some(reason) = &record.unavailable
                && unavailable_reason.is_none()
            {
                unavailable_reason = Some(reason.clone());
            }
            if run_reports.len() >= 16 {
                continue;
            }
            let tests_summary = record
                .report
                .as_ref()
                .and_then(|r| r.tests.as_ref())
                .map(|t| RollupTestSummary {
                    total: t.total,
                    passed: t.passed,
                    failed: t.failed,
                    errors: t.errors,
                    skipped: t.skipped,
                });
            let coverage_summary = record
                .report
                .as_ref()
                .and_then(|r| r.coverage.as_ref())
                .map(|c| RollupCoverageSummary {
                    format: c.format.clone(),
                    lines_found: c.lines_found,
                    lines_hit: c.lines_hit,
                    line_rate: c.line_rate,
                });
            let evidence_claim_boundary = record
                .report
                .as_ref()
                .and_then(|r| r.evidence.as_ref())
                .map(|e| e.claim_boundary.clone());
            run_reports.push(RollupVerificationReportRef {
                report_id: record.id.clone(),
                kind: record.kind,
                name: record.name.clone(),
                node_id: record.node_id.clone(),
                source_blake3: record.report.as_ref().map(|r| r.source_blake3.clone()),
                artifact_download_path: record.artifact.as_ref().map(|a| a.download_path.clone()),
                unavailable: record.unavailable.clone(),
                tests_summary,
                coverage_summary,
                evidence_claim_boundary,
            });
        }
    }

    let mut independent_verifications = Vec::new();
    for candidate in all_runs.values() {
        if !candidate.is_independent_verification()
            || candidate.project_id != project_id
            || candidate.source.exact_revision.native_id.as_str() != source_revision
        {
            continue;
        }
        if independent_verifications.len() >= 8 {
            break;
        }
        independent_verifications.push(RollupIndependentVerificationRef {
            run_id: candidate.id.clone(),
            outcome: candidate.outcome(),
            phase: candidate.phase.clone(),
            subject_hash: candidate
                .implementation_verification
                .as_ref()
                .map(|v| v.subject_hash.clone())
                .unwrap_or_default(),
            independent_receipt: candidate
                .implementation_verification
                .as_ref()
                .and_then(|v| v.independent_receipt.as_ref())
                .map(|id| id.as_str().to_owned()),
            artifact_count: candidate.artifacts.len(),
        });
    }

    let implementation_verification = impl_work
        .lookup(project_id, repository, source_revision)
        .into_iter()
        .find_map(|obs| obs.verification.clone());

    let has_present_report = run_reports.iter().any(|r| r.unavailable.is_none());
    let has_any_present = has_present_report
        || !independent_verifications.is_empty()
        || implementation_verification.is_some();

    let (state, out_unavailable_reason) = if has_any_present {
        ("present", None)
    } else if let Some(reason) = unavailable_reason {
        ("unavailable", Some(reason))
    } else if target_is_unrun(target_status, run) {
        ("unrun", None)
    } else if matches!(impl_work, ImplementationWorkIndex::Unavailable) {
        (
            "unavailable",
            Some("implementation_work_unavailable".to_owned()),
        )
    } else {
        ("missing", None)
    };

    let outcome = derive_verification_outcome(
        &independent_verifications,
        implementation_verification.as_ref(),
        &run_reports,
    );

    RollupVerificationReceipt {
        state,
        unavailable_reason: out_unavailable_reason,
        outcome,
        grants_authority: false,
        implementation_observation_status: impl_work.observation_status(),
        run_reports,
        independent_verifications,
        implementation_verification,
        claim_boundary: ROLLUP_CLAIM_BOUNDARY,
    }
}

fn derive_verification_outcome(
    independent: &[RollupIndependentVerificationRef],
    implementation: Option<&RollupImplementationVerificationRef>,
    reports: &[RollupVerificationReportRef],
) -> Option<String> {
    if let Some(first) = independent.first() {
        return Some(format!("{:?}", first.outcome));
    }
    if let Some(imp) = implementation {
        if let Some(vo) = &imp.verification_outcome {
            return Some(vo.clone());
        }
        return Some(if imp.success {
            "Success".to_owned()
        } else {
            "Unknown".to_owned()
        });
    }
    for report in reports {
        if report.unavailable.is_some() {
            continue;
        }
        if let Some(tests) = &report.tests_summary {
            return Some(if tests.failed > 0 || tests.errors > 0 {
                "Failure".to_owned()
            } else if tests.passed > 0 {
                "Success".to_owned()
            } else {
                "Unknown".to_owned()
            });
        }
    }
    None
}

pub fn extract_pull_requests(
    run: Option<&RunRecord>,
    target_status: &str,
    project_id: &str,
    repository: &str,
    source_revision: &str,
    impl_work: &ImplementationWorkIndex,
) -> (&'static str, Vec<RollupPullRequestLink>) {
    let mut links = Vec::new();
    let mut seen: BTreeSet<(String, &'static str)> = BTreeSet::new();

    for obs in impl_work.lookup(project_id, repository, source_revision) {
        for pr in &obs.pull_requests {
            if links.len() >= 16 {
                break;
            }
            if seen.insert((pr.url.clone(), pr.kind)) {
                links.push(pr.clone());
            }
        }
    }

    if let Some(run) = run {
        for record in &run.reports {
            if !matches!(
                record.kind,
                ReportKind::Provenance | ReportKind::Gate | ReportKind::Review
            ) {
                continue;
            }
            let Some(evidence) = record.report.as_ref().and_then(|r| r.evidence.as_ref()) else {
                continue;
            };
            collect_evidence_pr_links(&evidence.document, &evidence.links, &mut links, &mut seen);
        }
    }

    let state = if !links.is_empty() {
        "observed"
    } else if target_is_unrun(target_status, run) {
        "unrun"
    } else {
        "none_recorded"
    };
    (state, links)
}

fn collect_evidence_pr_links(
    document: &Value,
    evidence_links: &[crate::reports::EvidenceField],
    out: &mut Vec<RollupPullRequestLink>,
    seen: &mut BTreeSet<(String, &'static str)>,
) {
    let doc_pr_state = document
        .get("pull_request_state")
        .or_else(|| document.get("pr_state"))
        .or_else(|| document.get("state"))
        .and_then(Value::as_str)
        .map(normalize_pr_state);
    let doc_stack_state = document
        .get("stack_state")
        .or_else(|| document.get("state"))
        .and_then(Value::as_str)
        .map(normalize_pr_state);

    for (key, kind, fallback_state) in [
        ("pull_request_url", "pull_request", doc_pr_state.clone()),
        ("pr_url", "pull_request", doc_pr_state.clone()),
        ("stack_url", "stack", doc_stack_state.clone()),
        ("change_review_url", "change_review", doc_pr_state.clone()),
    ] {
        if let Some(raw) = document.get(key).and_then(Value::as_str) {
            push_evidence_url(
                raw,
                kind,
                fallback_state.as_deref().unwrap_or("unobserved"),
                out,
                seen,
            );
        }
    }

    for (array_key, default_kind) in [("pull_requests", "pull_request"), ("stacks", "stack")] {
        if let Some(items) = document.get(array_key).and_then(Value::as_array) {
            for item in items {
                match item {
                    Value::String(raw) => {
                        let st = if default_kind == "stack" {
                            doc_stack_state.as_deref()
                        } else {
                            doc_pr_state.as_deref()
                        };
                        push_evidence_url(raw, default_kind, st.unwrap_or("unobserved"), out, seen);
                    }
                    Value::Object(map) => {
                        if let Some(raw) = map
                            .get("url")
                            .or_else(|| map.get("uri"))
                            .and_then(Value::as_str)
                        {
                            let kind = match map.get("kind").and_then(Value::as_str) {
                                Some("stack") => "stack",
                                Some("change_review") => "change_review",
                                Some("pull_request") => "pull_request",
                                _ => default_kind,
                            };
                            let st = map
                                .get("state")
                                .and_then(Value::as_str)
                                .map(normalize_pr_state)
                                .unwrap_or_else(|| "unobserved".to_owned());
                            push_evidence_url(raw, kind, &st, out, seen);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for field in evidence_links {
        let Some(raw) = field.value.as_str() else {
            continue;
        };
        let lower_path = field.path.to_ascii_lowercase();
        let kind = if lower_path.contains("stack") || raw.contains("/stack") {
            Some("stack")
        } else if lower_path.contains("pull_request")
            || lower_path.ends_with("/pr_url")
            || lower_path.ends_with("/pr_uri")
            || raw.contains("/pull/")
        {
            Some("pull_request")
        } else if lower_path.contains("change_review") || lower_path.contains("review_url") {
            Some("change_review")
        } else {
            None
        };
        if let Some(kind) = kind {
            let st = if kind == "stack" {
                doc_stack_state.as_deref()
            } else {
                doc_pr_state.as_deref()
            };
            push_evidence_url(raw, kind, st.unwrap_or("unobserved"), out, seen);
        }
    }
}

fn push_evidence_url(
    raw: &str,
    kind: &'static str,
    state: &str,
    out: &mut Vec<RollupPullRequestLink>,
    seen: &mut BTreeSet<(String, &'static str)>,
) {
    if out.len() >= 16 {
        return;
    }
    let Some(url) = validate_https_url(raw) else {
        return;
    };
    if !seen.insert((url.clone(), kind)) {
        return;
    }
    let number = url
        .rsplit('/')
        .next()
        .and_then(|seg| seg.parse::<u64>().ok())
        .filter(|n| *n > 0);
    out.push(RollupPullRequestLink {
        url,
        kind,
        state: state.to_owned(),
        number,
        source: "report_evidence",
        reference: None,
        branch: None,
        base_branch: None,
        merge_commit: None,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn build_target_view_for_run(
    index: u16,
    target: String,
    repository: String,
    resolved_reference: String,
    write_scope: Option<String>,
    status: &'static str,
    reason: Option<String>,
    admitted_at_ms: Option<u64>,
    completed_at_ms: Option<u64>,
    run: &RunRecord,
    all_runs: &BTreeMap<String, RunRecord>,
    impl_work: &ImplementationWorkIndex,
) -> RollupTargetView {
    let source_revision = run.source.exact_revision.native_id.as_str().to_owned();
    let patchgate_receipt = extract_patchgate_receipt(Some(run), status);
    let verification_receipt = extract_verification_receipt(
        Some(run),
        status,
        &run.project_id,
        &repository,
        &source_revision,
        all_runs,
        impl_work,
    );
    let (pull_request_state, pull_requests) = extract_pull_requests(
        Some(run),
        status,
        &run.project_id,
        &repository,
        &source_revision,
        impl_work,
    );
    RollupTargetView {
        index,
        project_id: run.project_id.clone(),
        repository,
        target,
        resolved_reference,
        source_revision,
        canonical_tree: run.source.canonical_tree.as_str().to_owned(),
        write_scope,
        status,
        run_id: Some(run.id.clone()),
        run_outcome: Some(run.outcome()),
        run_phase: Some(run.phase.clone()),
        error_code: run.error_code.clone(),
        reason,
        patchgate_receipt,
        verification_receipt,
        pull_request_state,
        pull_requests,
        admitted_at_ms,
        completed_at_ms,
    }
}

pub fn build_fanout_target_views(
    fanout: &FanOutRun,
    all_runs: &BTreeMap<String, RunRecord>,
    impl_work: &ImplementationWorkIndex,
) -> Vec<RollupTargetView> {
    fanout
        .children
        .iter()
        .map(|child| {
            let child_run = child.run_id.as_deref().and_then(|id| all_runs.get(id));
            if let Some(run) = child_run {
                let status = outcome_to_rollup_status(run.outcome());
                build_target_view_for_run(
                    child.index,
                    child.target.clone(),
                    child.repository.clone(),
                    child.resolved_reference.clone(),
                    child.write_scope.as_ref().map(|s| s.text()),
                    status,
                    child.reason.clone(),
                    child.admitted_at_ms.or(run.created_at_ms),
                    child.completed_at_ms.or(run.completed_at_ms),
                    run,
                    all_runs,
                    impl_work,
                )
            } else {
                let status = fanout_status_str(child.status);
                let patchgate_receipt = extract_patchgate_receipt(None, status);
                let verification_receipt = extract_verification_receipt(
                    None,
                    status,
                    &child.project_id,
                    &child.repository,
                    &child.source_revision,
                    all_runs,
                    impl_work,
                );
                let (pull_request_state, pull_requests) = extract_pull_requests(
                    None,
                    status,
                    &child.project_id,
                    &child.repository,
                    &child.source_revision,
                    impl_work,
                );
                RollupTargetView {
                    index: child.index,
                    project_id: child.project_id.clone(),
                    repository: child.repository.clone(),
                    target: child.target.clone(),
                    resolved_reference: child.resolved_reference.clone(),
                    source_revision: child.source_revision.clone(),
                    canonical_tree: child.source.canonical_tree.as_str().to_owned(),
                    write_scope: child.write_scope.as_ref().map(|s| s.text()),
                    status,
                    run_id: child.run_id.clone(),
                    run_outcome: None,
                    run_phase: None,
                    error_code: None,
                    reason: child.reason.clone(),
                    patchgate_receipt,
                    verification_receipt,
                    pull_request_state,
                    pull_requests,
                    admitted_at_ms: child.admitted_at_ms,
                    completed_at_ms: child.completed_at_ms,
                }
            }
        })
        .collect()
}

pub fn build_dispatch_target_view(
    run: &RunRecord,
    all_runs: &BTreeMap<String, RunRecord>,
    impl_work: &ImplementationWorkIndex,
) -> Result<RollupTargetView, String> {
    let rb = run
        .runbook
        .as_ref()
        .ok_or_else(|| "run is not a runbook dispatch".to_owned())?;
    let status = outcome_to_rollup_status(run.outcome());
    Ok(build_target_view_for_run(
        0,
        rb.target.clone(),
        run.source.repository.native_id.as_str().to_owned(),
        run.source.symbolic_reference.native_id.as_str().to_owned(),
        None,
        status,
        run.error_code.clone(),
        run.created_at_ms,
        run.completed_at_ms,
        run,
        all_runs,
        impl_work,
    ))
}

#[must_use]
pub fn compute_counts(targets: &[RollupTargetView]) -> RollupCounts {
    let mut counts = RollupCounts {
        total: targets.len(),
        ..RollupCounts::default()
    };
    for target in targets {
        match target.status {
            "pending" => counts.pending += 1,
            "active" => counts.active += 1,
            "success" => counts.success += 1,
            "failure" => counts.failure += 1,
            "unknown" => counts.unknown += 1,
            "skipped" => counts.skipped += 1,
            _ => {}
        }
        match target.patchgate_receipt.state {
            "present" => counts.patchgate_present += 1,
            "missing" => counts.patchgate_missing += 1,
            "unrun" => counts.patchgate_unrun += 1,
            "unavailable" => counts.patchgate_unavailable += 1,
            _ => {}
        }
        match target.verification_receipt.state {
            "present" => counts.verification_present += 1,
            "missing" => counts.verification_missing += 1,
            "unrun" => counts.verification_unrun += 1,
            "unavailable" => counts.verification_unavailable += 1,
            _ => {}
        }
        if !target.pull_requests.is_empty() {
            counts.pull_requests_observed += 1;
        }
    }
    counts
}

#[must_use]
pub fn summarize_notification_status(
    subject_kind: &str,
    subject_id: &str,
    runbook_id: &str,
    records: &BTreeMap<String, SummaryDeliveryRecord>,
    registry: Option<&RunbookRegistry>,
) -> RollupNotificationStatus {
    let mut deliveries: Vec<RollupNotificationDeliveryView> = records
        .values()
        .filter(|r| r.subject_kind == subject_kind && r.subject_id == subject_id)
        .map(|r| RollupNotificationDeliveryView {
            registration_id: r.registration_id.clone(),
            delivery_id: r.id.clone(),
            status: r.status.clone(),
            reserved_at_ms: r.reserved_at_ms,
            completed_at_ms: r.completed_at_ms,
            http_status: r.http_status,
            error_code: r.error_code.clone(),
        })
        .collect();
    deliveries.sort_by(|a, b| {
        a.reserved_at_ms
            .cmp(&b.reserved_at_ms)
            .then_with(|| a.delivery_id.cmp(&b.delivery_id))
    });
    if deliveries.len() > 16 {
        deliveries.truncate(16);
    }

    if let Some(latest) = deliveries.last() {
        let state = if deliveries.iter().any(|d| d.status == "reserved") {
            "pending"
        } else if deliveries.iter().any(|d| d.status == "ambiguous") {
            "ambiguous"
        } else if deliveries.iter().all(|d| d.status == "delivered") {
            "delivered"
        } else {
            "rejected"
        };
        return RollupNotificationStatus {
            state,
            delivery_id: Some(latest.delivery_id.clone()),
            completed_at_ms: latest.completed_at_ms,
            error_code: latest.error_code.clone(),
            deliveries,
        };
    }

    let configured_for_runbook = registry.is_some_and(|reg| {
        reg.notifications
            .iter()
            .any(|n| n.enabled && n.applies_to(runbook_id))
    });

    RollupNotificationStatus {
        state: if configured_for_runbook {
            "pending"
        } else {
            "not_configured"
        },
        delivery_id: None,
        completed_at_ms: None,
        error_code: None,
        deliveries: Vec::new(),
    }
}

#[derive(Clone, Debug)]
pub struct SummaryNotificationInput<'a> {
    pub idempotency_key: &'a str,
    pub registration_id: &'a str,
    pub subject_kind: &'a str,
    pub subject_id: &'a str,
    pub runbook_id: &'a str,
    pub mode: RunbookMode,
    pub phase: &'a str,
    pub overall_outcome: Option<Outcome>,
    pub targets: &'a [RollupTargetView],
    pub max_request_bytes: usize,
}

pub fn build_summary_notification_body(
    input: &SummaryNotificationInput<'_>,
) -> Result<Vec<u8>, String> {
    let counts = compute_counts(input.targets);
    let summary_counts = SummaryNotificationCounts {
        total: counts.total,
        pending: counts.pending,
        active: counts.active,
        success: counts.success,
        failure: counts.failure,
        unknown: counts.unknown,
        skipped: counts.skipped,
    };
    let has_unknown = counts.unknown > 0 || input.overall_outcome == Some(Outcome::Unknown);

    let full_targets: Vec<RunbookSummaryNotificationTarget> = input
        .targets
        .iter()
        .map(|t| RunbookSummaryNotificationTarget {
            index: t.index,
            project_id: t.project_id.clone(),
            target: t.target.clone(),
            source_revision: t.source_revision.clone(),
            status: t.status.to_owned(),
            run_outcome: t.run_outcome,
            patchgate_state: t.patchgate_receipt.state.to_owned(),
            patchgate_decision_counts: t.patchgate_receipt.decision_counts.clone(),
            verification_state: t.verification_receipt.state.to_owned(),
            verification_outcome: t.verification_receipt.outcome.clone(),
            pull_request_urls: t.pull_requests.iter().map(|pr| pr.url.clone()).collect(),
        })
        .collect();

    let mut limit = full_targets.len();
    loop {
        let sliced = full_targets.get(..limit).unwrap_or(&[]);
        let body = RunbookSummaryNotificationBody {
            schema: RUNBOOK_SUMMARY_NOTIFICATION_SCHEMA.to_owned(),
            idempotency_key: input.idempotency_key.to_owned(),
            notification_policy: input.registration_id.to_owned(),
            subject_kind: input.subject_kind.to_owned(),
            subject_id: input.subject_id.to_owned(),
            runbook_id: input.runbook_id.to_owned(),
            mode: input.mode,
            phase: input.phase.to_owned(),
            overall_outcome: input.overall_outcome,
            has_unknown,
            counts: summary_counts.clone(),
            targets_truncated: limit < full_targets.len(),
            targets: sliced.to_vec(),
            grants_authority: false,
            non_claims: ROLLUP_NON_CLAIMS.iter().map(|s| (*s).to_owned()).collect(),
        };
        let bytes = serde_json::to_vec(&body)
            .map_err(|_| "summary notification body encoding failed".to_owned())?;
        if bytes.len() <= input.max_request_bytes {
            return Ok(bytes);
        }
        if limit == 0 {
            return Err("summary notification body exceeds registration bound".into());
        }
        limit /= 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_url_validation_rejects_non_https_userinfo_and_control_bytes() {
        assert_eq!(
            validate_https_url("https://github.com/onixresearch/kiln/pull/100"),
            Some("https://github.com/onixresearch/kiln/pull/100".to_owned())
        );
        assert_eq!(
            validate_https_url("http://github.com/onixresearch/kiln/pull/100"),
            None
        );
        assert_eq!(validate_https_url("javascript:alert(1)"), None);
        assert_eq!(
            validate_https_url("https://user:pass@github.com/onixresearch/kiln/pull/100"),
            None
        );
        assert_eq!(
            validate_https_url("https://user@github.com/onixresearch/kiln/pull/100"),
            None
        );
        assert_eq!(
            validate_https_url("https://github.com/onixresearch/kiln/pull/100\n"),
            None
        );
        assert_eq!(
            validate_https_url("https://github.com/onixresearch/kiln/pull/100 "),
            None
        );
        assert_eq!(
            validate_https_url("https://github.com/onixresearch/kiln/pull/100\u{00a0}"),
            None
        );
        assert_eq!(
            validate_https_url("https://github.com/onixresearch/kiln/pull/100\u{2003}"),
            None
        );
        assert_eq!(
            validate_https_url("https://github.com/onixresearch/kiln/pull/100\u{007f}"),
            None
        );
        assert_eq!(
            validate_https_url("https://github.com/onixresearch/kiln/pull/100\u{009b}"),
            None
        );
    }

    #[test]
    fn outcome_mapping_preserves_unknown_explicitly() {
        assert_eq!(outcome_to_rollup_status(Outcome::Unknown), "unknown");
        assert_eq!(outcome_to_rollup_status(Outcome::Success), "success");
        assert_eq!(outcome_to_rollup_status(Outcome::DomainFailure), "failure");
        assert_eq!(outcome_to_rollup_status(Outcome::Pending), "pending");
    }

    #[test]
    fn summary_notification_body_omits_params_secrets_and_logs() {
        let target = RollupTargetView {
            index: 0,
            project_id: "kiln".into(),
            repository: "onixresearch/kiln".into(),
            target: "onixresearch/kiln@refs/heads/main".into(),
            resolved_reference: "refs/heads/main".into(),
            source_revision: "a".repeat(40),
            canonical_tree: "b".repeat(64),
            write_scope: None,
            status: "unknown",
            run_id: Some("web-run-1".into()),
            run_outcome: Some(Outcome::Unknown),
            run_phase: Some("unknown".into()),
            error_code: Some("provider_unknown".into()),
            reason: None,
            patchgate_receipt: empty_patchgate_receipt("missing", None),
            verification_receipt: RollupVerificationReceipt {
                state: "missing",
                unavailable_reason: None,
                outcome: None,
                grants_authority: false,
                implementation_observation_status: "unconfigured",
                run_reports: Vec::new(),
                independent_verifications: Vec::new(),
                implementation_verification: None,
                claim_boundary: ROLLUP_CLAIM_BOUNDARY,
            },
            pull_request_state: "none_recorded",
            pull_requests: Vec::new(),
            admitted_at_ms: Some(10),
            completed_at_ms: None,
        };
        let targets = [target];
        let bytes = build_summary_notification_body(&SummaryNotificationInput {
            idempotency_key: "delivery-1",
            registration_id: "ops-hook",
            subject_kind: "dispatch",
            subject_id: "web-run-1",
            runbook_id: "audit",
            mode: RunbookMode::ReadOnly,
            phase: "unknown",
            overall_outcome: Some(Outcome::Unknown),
            targets: &targets,
            max_request_bytes: 65_536,
        })
        .unwrap_or_else(|e| unreachable!("body failed: {e}"));
        let text = String::from_utf8(bytes).unwrap_or_else(|e| unreachable!("utf8: {e}"));
        assert!(text.contains(RUNBOOK_SUMMARY_NOTIFICATION_SCHEMA));
        assert!(text.contains("\"has_unknown\":true"));
        assert!(text.contains("\"grants_authority\":false"));
        assert!(!text.contains("env_bindings"));
        assert!(!text.contains("source_env"));
        assert!(!text.contains("bearer_secret_file"));
        assert!(!text.contains("LATTICE_PARAM_"));
    }
}
