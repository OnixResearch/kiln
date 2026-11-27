//! Audience-restricted projection over separate-authority full-candidate
//! verification outcomes (#77, #79).
//!
//! A trusted verifier/reconstructor evaluates the full candidate (including
//! affected downstream projects outside the worker's workspace) under its own
//! authority and produces a [`FullCandidateVerification`]. A restricted worker
//! never receives that full outcome directly; instead [`project_for_audience`],
//! [`project_for_scope`], and [`project_for_definition`] filter it under the
//! holder's [`ViewCeiling`] and current [`EffectiveSourceScope`] before any
//! count, verdict, test, log chunk, diagnostic, or artifact is surfaced.
//!
//! Strict non-interference:
//! - Hidden affected projects, hidden file paths, hidden test names, hidden
//!   diagnostic lines, and hidden artifacts are omitted without leaking names,
//!   paths, digests, or counts.
//! - When any invisible project does not pass, only a bounded two-state
//!   [`ExternalImpact`] indicator (`NoneReported` vs `ExternalChecksFailed`) is
//!   surfaced, revealing neither how many hidden projects exist nor which
//!   failed.
//! - [`ProjectedVerificationResult`] cannot be deserialized; callers cannot
//!   forge an authorized projection.

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::authority::{EffectiveSourceScope, PathScope, SourceViewOperation};
use super::disclosure::{AudienceView, DisclosureRefusal, PublicUnavailable, ViewCeiling};
use super::{AdmittedDefinition, update_bool, update_external};
use crate::monorepo::{finish, update_field, update_usize};
use crate::paths;
use crate::types::{ContentId, DEFAULT_MAX_ID_BYTES, Diagnostic, ExternalRef};

pub const FULL_VERIFICATION_SCHEMA: &str = "kiln.full-candidate-verification.v1";
pub const VERIFIER_PROJECTION_SCHEMA: &str = "kiln.verifier-result-projection.v1";
const VERIFIER_PROJECTION_IDENTITY_SCHEMA: &str = "kiln.verifier-result-projection-identity.v1";

/// Maximum affected projects admitted in one full-candidate verification.
pub const MAX_AFFECTED_PROJECTS: usize = 64;
/// Maximum test cases admitted in one full-candidate verification.
pub const MAX_TEST_CASES: usize = 4_096;
/// Maximum log chunks admitted in one full-candidate verification.
pub const MAX_LOG_CHUNKS: usize = 4_096;
/// Maximum bytes per log chunk.
pub const MAX_LOG_CHUNK_BYTES: usize = 16_384;
/// Maximum tool/compiler diagnostics admitted in one full-candidate verification.
pub const MAX_TOOL_DIAGNOSTICS: usize = 1_024;
/// Maximum bytes per diagnostic or test failure message.
pub const MAX_DIAGNOSTIC_MESSAGE_BYTES: usize = 4_096;
/// Maximum output artifacts admitted in one full-candidate verification.
pub const MAX_OUTPUT_ARTIFACTS: usize = 1_024;
/// Maximum bytes per project identifier.
pub const MAX_PROJECT_ID_BYTES: usize = DEFAULT_MAX_ID_BYTES;
/// Maximum bytes per test case name.
pub const MAX_TEST_NAME_BYTES: usize = 512;
/// Maximum bytes per artifact name.
pub const MAX_ARTIFACT_NAME_BYTES: usize = 256;
/// Maximum paths attached to one test case or log chunk.
pub const MAX_PATHS_PER_ITEM: usize = 64;
/// Maximum bytes per tool diagnostic code.
pub const MAX_DIAGNOSTIC_CODE_BYTES: usize = 128;

/// Terminal verification verdict of a project or test case.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationVerdict {
    Passed,
    Failed,
    Cancelled,
    Unknown,
}

impl VerificationVerdict {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }
}

/// Output stream of a verification log chunk.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    Stdout,
    Stderr,
}

impl LogStream {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// Severity of a compiler or verification tool diagnostic.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
}

impl DiagnosticSeverity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

/// Bounded opaque indicator for checks outside the holder's view.
///
/// Reveals neither the existence of passing hidden projects nor the count,
/// names, paths, or reasons of failing hidden projects.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImpact {
    /// No invisible project failure is reported.
    NoneReported,
    /// One or more checks outside the holder's view did not pass.
    ExternalChecksFailed,
}

impl ExternalImpact {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoneReported => "none_reported",
            Self::ExternalChecksFailed => "external_checks_failed",
        }
    }
}

/// Full-candidate verification outcome of one affected project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectVerification {
    pub project_id: String,
    pub scope: PathScope,
    pub verdict: VerificationVerdict,
}

/// One test case outcome produced during full-candidate verification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestCaseOutcome {
    pub project_id: String,
    pub name: String,
    pub verdict: VerificationVerdict,
    pub duration_ms: u64,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub message: Option<String>,
}

/// One bounded log chunk produced during full-candidate verification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogChunk {
    pub project_id: String,
    pub stream: LogStream,
    pub sequence: u64,
    pub text: String,
    #[serde(default)]
    pub paths: Vec<String>,
}

/// One compiler or tool diagnostic produced during full-candidate verification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDiagnostic {
    pub project_id: String,
    pub severity: DiagnosticSeverity,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub line: Option<u32>,
    pub message: String,
}

/// One output artifact produced during full-candidate verification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputArtifact {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
    pub digest: ContentId,
    pub size_bytes: u64,
}

/// Complete verification outcome recorded by the separate-authority verifier
/// over the reconstructed full candidate. Never sent directly to a restricted
/// holder.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FullCandidateVerification {
    pub schema: String,
    pub repository: ContentId,
    pub base_revision: ExternalRef,
    pub candidate_identity: ContentId,
    #[serde(default)]
    pub candidate_facts_identity: Option<ContentId>,
    pub affected_projects: Vec<ProjectVerification>,
    #[serde(default)]
    pub test_cases: Vec<TestCaseOutcome>,
    #[serde(default)]
    pub log_chunks: Vec<LogChunk>,
    #[serde(default)]
    pub diagnostics: Vec<ToolDiagnostic>,
    #[serde(default)]
    pub artifacts: Vec<OutputArtifact>,
}

impl FullCandidateVerification {
    /// Validate schema, bounds, identifiers, and canonical paths.
    ///
    /// # Errors
    /// Returns sorted diagnostics for every violation.
    pub fn validate(&self) -> Result<(), Vec<Diagnostic>> {
        let mut findings = super::Findings::default();
        if self.schema != FULL_VERIFICATION_SCHEMA {
            findings.push(
                "unsupported_schema",
                "schema",
                "full-candidate verification schema is unsupported",
            );
        }
        if self.affected_projects.len() > MAX_AFFECTED_PROJECTS {
            findings.push(
                "bound_exceeded",
                "affected_projects",
                "more affected projects than the named bound",
            );
        }
        if self.test_cases.len() > MAX_TEST_CASES {
            findings.push(
                "bound_exceeded",
                "test_cases",
                "more test cases than the named bound",
            );
        }
        if self.log_chunks.len() > MAX_LOG_CHUNKS {
            findings.push(
                "bound_exceeded",
                "log_chunks",
                "more log chunks than the named bound",
            );
        }
        if self.diagnostics.len() > MAX_TOOL_DIAGNOSTICS {
            findings.push(
                "bound_exceeded",
                "diagnostics",
                "more tool diagnostics than the named bound",
            );
        }
        if self.artifacts.len() > MAX_OUTPUT_ARTIFACTS {
            findings.push(
                "bound_exceeded",
                "artifacts",
                "more output artifacts than the named bound",
            );
        }
        let mut seen_projects = BTreeSet::new();
        for project in &self.affected_projects {
            if !valid_id(&project.project_id, MAX_PROJECT_ID_BYTES) {
                findings.push(
                    "invalid_project_id",
                    "affected_projects",
                    "project_id must be bounded printable text without control characters",
                );
            } else if !seen_projects.insert(project.project_id.as_str()) {
                findings.push(
                    "duplicate_project_id",
                    "affected_projects",
                    "affected project_id must be unique",
                );
            }
            if project.scope.validate().is_err() {
                findings.push(
                    "invalid_project_scope",
                    "affected_projects.scope",
                    "affected project scope must hold safe canonical components",
                );
            }
        }
        for test_case in &self.test_cases {
            if !valid_id(&test_case.project_id, MAX_PROJECT_ID_BYTES)
                || !valid_id(&test_case.name, MAX_TEST_NAME_BYTES)
            {
                findings.push(
                    "invalid_test_case",
                    "test_cases",
                    "test case project_id and name must be non-empty bounded text",
                );
            }
            if test_case.paths.len() > MAX_PATHS_PER_ITEM
                || test_case.paths.iter().any(|path| !paths::canonical(path))
            {
                findings.push(
                    "invalid_test_path",
                    "test_cases.paths",
                    "test case paths must be canonical and within the named bound",
                );
            }
            if test_case
                .message
                .as_deref()
                .is_some_and(|msg| !valid_multiline(msg, MAX_DIAGNOSTIC_MESSAGE_BYTES))
            {
                findings.push(
                    "invalid_test_message",
                    "test_cases.message",
                    "test case message exceeds bound or contains NUL/control characters",
                );
            }
        }
        for chunk in &self.log_chunks {
            if !valid_id(&chunk.project_id, MAX_PROJECT_ID_BYTES)
                || !valid_multiline(&chunk.text, MAX_LOG_CHUNK_BYTES)
            {
                findings.push(
                    "invalid_log_chunk",
                    "log_chunks",
                    "log chunk project_id or text is invalid or exceeds bound",
                );
            }
            if chunk.paths.len() > MAX_PATHS_PER_ITEM
                || chunk.paths.iter().any(|path| !paths::canonical(path))
            {
                findings.push(
                    "invalid_log_path",
                    "log_chunks.paths",
                    "log chunk paths must be canonical and within the named bound",
                );
            }
        }
        for diag in &self.diagnostics {
            if !valid_id(&diag.project_id, MAX_PROJECT_ID_BYTES)
                || !valid_multiline(&diag.message, MAX_DIAGNOSTIC_MESSAGE_BYTES)
            {
                findings.push(
                    "invalid_diagnostic",
                    "diagnostics",
                    "tool diagnostic project_id or message is invalid or exceeds bound",
                );
            }
            if diag
                .code
                .as_deref()
                .is_some_and(|code| !valid_id(code, MAX_DIAGNOSTIC_CODE_BYTES))
            {
                findings.push(
                    "invalid_diagnostic_code",
                    "diagnostics.code",
                    "tool diagnostic code must be non-empty bounded text",
                );
            }
            if diag
                .file_path
                .as_deref()
                .is_some_and(|path| !paths::canonical(path))
            {
                findings.push(
                    "invalid_diagnostic_path",
                    "diagnostics.file_path",
                    "tool diagnostic file_path must be canonical",
                );
            }
        }
        for artifact in &self.artifacts {
            if !valid_id(&artifact.project_id, MAX_PROJECT_ID_BYTES)
                || !valid_id(&artifact.name, MAX_ARTIFACT_NAME_BYTES)
            {
                findings.push(
                    "invalid_artifact",
                    "artifacts",
                    "output artifact project_id or name is invalid or exceeds bound",
                );
            }
            if artifact
                .path
                .as_deref()
                .is_some_and(|path| !paths::canonical(path))
            {
                findings.push(
                    "invalid_artifact_path",
                    "artifacts.path",
                    "output artifact path must be canonical",
                );
            }
        }
        findings.finish(())
    }
}

/// Why verification result projection was refused, for the trusted owner's
/// audit. External callers receive only [`PublicUnavailable`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionRefusal {
    Expired,
    RepositoryMismatch,
    SnapshotMismatch,
    NotAuthorized,
    Stale,
    InvalidInput,
}

impl ProjectionRefusal {
    #[must_use]
    pub const fn public(&self) -> PublicUnavailable {
        DisclosureRefusal::NotAuthorized.public()
    }
}

/// Authorized project verification status disclosed to the holder.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProjectedProjectVerification {
    pub project_id: String,
    pub verdict: VerificationVerdict,
}

/// Authorized test case outcome disclosed to the holder.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProjectedTestCase {
    pub project_id: String,
    pub name: String,
    pub verdict: VerificationVerdict,
    pub duration_ms: u64,
    pub paths: Vec<String>,
    pub message: Option<String>,
}

/// Authorized log chunk disclosed to the holder.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProjectedLogChunk {
    pub project_id: String,
    pub stream: LogStream,
    pub sequence: u64,
    pub text: String,
    pub paths: Vec<String>,
}

/// Authorized compiler or tool diagnostic disclosed to the holder.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProjectedToolDiagnostic {
    pub project_id: String,
    pub severity: DiagnosticSeverity,
    pub code: Option<String>,
    pub file_path: Option<String>,
    pub line: Option<u32>,
    pub message: String,
}

/// Authorized output artifact disclosed to the holder.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProjectedOutputArtifact {
    pub project_id: String,
    pub name: String,
    pub path: Option<String>,
    pub digest: ContentId,
    pub size_bytes: u64,
}

/// Audience-authorized projection of a full-candidate verification result.
///
/// Only [`project_for_audience`], [`project_for_scope`], or
/// [`project_for_definition`] constructs one; it cannot be deserialized, and
/// its BLAKE3 [`digest`](Self::digest) binds all disclosed fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectedVerificationResult {
    schema: &'static str,
    repository: ContentId,
    base_revision: ExternalRef,
    candidate_identity: ContentId,
    candidate_facts_identity: Option<ContentId>,
    ceiling: ContentId,
    audience: String,
    effective_scope: ContentId,
    visible_verdict: VerificationVerdict,
    external_impact: ExternalImpact,
    full_candidate_passed: bool,
    affected_projects: Vec<ProjectedProjectVerification>,
    test_cases: Vec<ProjectedTestCase>,
    log_chunks: Vec<ProjectedLogChunk>,
    diagnostics: Vec<ProjectedToolDiagnostic>,
    artifacts: Vec<ProjectedOutputArtifact>,
    digest: ContentId,
}

impl ProjectedVerificationResult {
    #[must_use]
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    #[must_use]
    pub fn repository(&self) -> &ContentId {
        &self.repository
    }
    #[must_use]
    pub fn base_revision(&self) -> &ExternalRef {
        &self.base_revision
    }
    #[must_use]
    pub fn candidate_identity(&self) -> &ContentId {
        &self.candidate_identity
    }
    #[must_use]
    pub fn candidate_facts_identity(&self) -> Option<&ContentId> {
        self.candidate_facts_identity.as_ref()
    }
    #[must_use]
    pub fn ceiling(&self) -> &ContentId {
        &self.ceiling
    }
    #[must_use]
    pub fn audience(&self) -> &str {
        &self.audience
    }
    #[must_use]
    pub fn effective_scope(&self) -> &ContentId {
        &self.effective_scope
    }
    #[must_use]
    pub const fn visible_verdict(&self) -> VerificationVerdict {
        self.visible_verdict
    }
    #[must_use]
    pub const fn external_impact(&self) -> ExternalImpact {
        self.external_impact
    }
    #[must_use]
    pub const fn full_candidate_passed(&self) -> bool {
        self.full_candidate_passed
    }
    #[must_use]
    pub fn affected_projects(&self) -> &[ProjectedProjectVerification] {
        &self.affected_projects
    }
    #[must_use]
    pub fn test_cases(&self) -> &[ProjectedTestCase] {
        &self.test_cases
    }
    #[must_use]
    pub fn log_chunks(&self) -> &[ProjectedLogChunk] {
        &self.log_chunks
    }
    #[must_use]
    pub fn diagnostics(&self) -> &[ProjectedToolDiagnostic] {
        &self.diagnostics
    }
    #[must_use]
    pub fn artifacts(&self) -> &[ProjectedOutputArtifact] {
        &self.artifacts
    }
    /// Deterministic BLAKE3 digest over the projected fields.
    #[must_use]
    pub fn digest(&self) -> &ContentId {
        &self.digest
    }
}

/// Project a full-candidate verification outcome to the holder of a current
/// [`AudienceView`] and [`EffectiveSourceScope`].
///
/// # Errors
/// Returns [`ProjectionRefusal`] if the scope expired, is stale, lacks
/// `RetrieveResult`, mismatches repository/snapshot, or the full outcome is
/// malformed.
pub fn project_for_audience(
    full: &FullCandidateVerification,
    view: &AudienceView,
    scope: &EffectiveSourceScope,
    now_unix_s: u64,
) -> Result<ProjectedVerificationResult, ProjectionRefusal> {
    view.check_current(scope, now_unix_s)
        .map_err(|_| ProjectionRefusal::Stale)?;
    if full.repository != *view.binding().repository() {
        return Err(ProjectionRefusal::RepositoryMismatch);
    }
    if full.base_revision != *view.binding().snapshot() {
        return Err(ProjectionRefusal::SnapshotMismatch);
    }
    project_internal(full, None, view.ceiling(), scope, now_unix_s)
}

/// Project a full-candidate verification outcome using a [`ViewCeiling`] and
/// current [`EffectiveSourceScope`].
///
/// # Errors
/// Returns [`ProjectionRefusal`] on expired/stale/unauthorized scope,
/// repository/revision mismatch, or malformed verification input.
pub fn project_for_scope(
    full: &FullCandidateVerification,
    ceiling: &ViewCeiling,
    scope: &EffectiveSourceScope,
    now_unix_s: u64,
) -> Result<ProjectedVerificationResult, ProjectionRefusal> {
    project_internal(full, None, ceiling, scope, now_unix_s)
}

/// Project a full-candidate verification outcome using an [`AdmittedDefinition`],
/// its [`ViewCeiling`], and current [`EffectiveSourceScope`]. In addition to
/// ceiling and scope checks, paths excluded by `definition` are omitted.
///
/// # Errors
/// Returns [`ProjectionRefusal`] on mismatched definition/ceiling,
/// expired/unauthorized scope, repository/revision mismatch, or malformed input.
pub fn project_for_definition(
    full: &FullCandidateVerification,
    definition: &AdmittedDefinition,
    ceiling: &ViewCeiling,
    scope: &EffectiveSourceScope,
    now_unix_s: u64,
) -> Result<ProjectedVerificationResult, ProjectionRefusal> {
    if definition.identity() != ceiling.definition() {
        return Err(ProjectionRefusal::Stale);
    }
    project_internal(full, Some(definition), ceiling, scope, now_unix_s)
}

fn project_internal(
    full: &FullCandidateVerification,
    definition: Option<&AdmittedDefinition>,
    ceiling: &ViewCeiling,
    scope: &EffectiveSourceScope,
    now_unix_s: u64,
) -> Result<ProjectedVerificationResult, ProjectionRefusal> {
    if !scope.live_at(now_unix_s) {
        return Err(ProjectionRefusal::Expired);
    }
    if scope.repository() != &full.repository {
        return Err(ProjectionRefusal::RepositoryMismatch);
    }
    if scope.revision() != &full.base_revision {
        return Err(ProjectionRefusal::SnapshotMismatch);
    }
    if !ceiling
        .operations()
        .contains(&SourceViewOperation::RetrieveResult)
        || !scope.names(SourceViewOperation::RetrieveResult)
    {
        return Err(ProjectionRefusal::NotAuthorized);
    }
    if full.validate().is_err() {
        return Err(ProjectionRefusal::InvalidInput);
    }

    let path_authorized = |path: &str| -> bool {
        ceiling.allows(SourceViewOperation::RetrieveResult, path)
            && scope.permits(SourceViewOperation::RetrieveResult, path)
            && definition.is_none_or(|def| def.selects(path))
    };
    let project_authorized = |project: &ProjectVerification| -> bool {
        ceiling.contains(&project.scope)
            && scope.permits_scope(SourceViewOperation::RetrieveResult, &project.scope)
    };

    let mut affected_projects = Vec::new();
    let mut visible_project_ids = BTreeSet::new();
    let mut hidden_failed = false;

    for project in &full.affected_projects {
        if project_authorized(project) {
            visible_project_ids.insert(project.project_id.as_str());
            affected_projects.push(ProjectedProjectVerification {
                project_id: project.project_id.clone(),
                verdict: project.verdict,
            });
        } else if project.verdict != VerificationVerdict::Passed {
            hidden_failed = true;
        }
    }
    affected_projects.sort();

    let external_impact = if hidden_failed {
        ExternalImpact::ExternalChecksFailed
    } else {
        ExternalImpact::NoneReported
    };

    let mut test_cases: Vec<ProjectedTestCase> = full
        .test_cases
        .iter()
        .filter(|tc| {
            visible_project_ids.contains(tc.project_id.as_str())
                && tc.paths.iter().all(|path| path_authorized(path))
        })
        .map(|tc| {
            let mut paths = tc.paths.clone();
            paths.sort();
            ProjectedTestCase {
                project_id: tc.project_id.clone(),
                name: tc.name.clone(),
                verdict: tc.verdict,
                duration_ms: tc.duration_ms,
                paths,
                message: tc.message.clone(),
            }
        })
        .collect();
    test_cases.sort();

    let mut log_chunks: Vec<ProjectedLogChunk> = full
        .log_chunks
        .iter()
        .filter(|chunk| {
            visible_project_ids.contains(chunk.project_id.as_str())
                && chunk.paths.iter().all(|path| path_authorized(path))
        })
        .map(|chunk| {
            let mut paths = chunk.paths.clone();
            paths.sort();
            ProjectedLogChunk {
                project_id: chunk.project_id.clone(),
                stream: chunk.stream,
                sequence: chunk.sequence,
                text: chunk.text.clone(),
                paths,
            }
        })
        .collect();
    log_chunks.sort_by(|a, b| {
        (&a.project_id, a.sequence, a.stream, &a.text, &a.paths).cmp(&(
            &b.project_id,
            b.sequence,
            b.stream,
            &b.text,
            &b.paths,
        ))
    });

    let mut diagnostics: Vec<ProjectedToolDiagnostic> = full
        .diagnostics
        .iter()
        .filter(|diag| {
            visible_project_ids.contains(diag.project_id.as_str())
                && diag
                    .file_path
                    .as_deref()
                    .is_none_or(|path| path_authorized(path))
        })
        .map(|diag| ProjectedToolDiagnostic {
            project_id: diag.project_id.clone(),
            severity: diag.severity,
            code: diag.code.clone(),
            file_path: diag.file_path.clone(),
            line: diag.line,
            message: diag.message.clone(),
        })
        .collect();
    diagnostics.sort();

    let mut artifacts: Vec<ProjectedOutputArtifact> = full
        .artifacts
        .iter()
        .filter(|artifact| {
            visible_project_ids.contains(artifact.project_id.as_str())
                && artifact
                    .path
                    .as_deref()
                    .is_none_or(|path| path_authorized(path))
        })
        .map(|artifact| ProjectedOutputArtifact {
            project_id: artifact.project_id.clone(),
            name: artifact.name.clone(),
            path: artifact.path.clone(),
            digest: artifact.digest.clone(),
            size_bytes: artifact.size_bytes,
        })
        .collect();
    artifacts.sort();

    let visible_verdict = aggregate_visible_verdict(&affected_projects, &test_cases);
    let full_candidate_passed = visible_verdict == VerificationVerdict::Passed
        && external_impact == ExternalImpact::NoneReported;

    let digest = projection_digest(
        full,
        ceiling,
        scope,
        visible_verdict,
        external_impact,
        full_candidate_passed,
        &affected_projects,
        &test_cases,
        &log_chunks,
        &diagnostics,
        &artifacts,
    );

    Ok(ProjectedVerificationResult {
        schema: VERIFIER_PROJECTION_SCHEMA,
        repository: full.repository.clone(),
        base_revision: full.base_revision.clone(),
        candidate_identity: full.candidate_identity.clone(),
        candidate_facts_identity: full.candidate_facts_identity.clone(),
        ceiling: ceiling.identity().clone(),
        audience: scope.audience().to_string(),
        effective_scope: scope.identity().clone(),
        visible_verdict,
        external_impact,
        full_candidate_passed,
        affected_projects,
        test_cases,
        log_chunks,
        diagnostics,
        artifacts,
        digest,
    })
}

fn aggregate_visible_verdict(
    projects: &[ProjectedProjectVerification],
    test_cases: &[ProjectedTestCase],
) -> VerificationVerdict {
    let mut verdicts = projects
        .iter()
        .map(|p| p.verdict)
        .chain(test_cases.iter().map(|t| t.verdict));
    let mut has_cancelled = false;
    let mut has_unknown = false;
    for verdict in &mut verdicts {
        match verdict {
            VerificationVerdict::Failed => return VerificationVerdict::Failed,
            VerificationVerdict::Cancelled => has_cancelled = true,
            VerificationVerdict::Unknown => has_unknown = true,
            VerificationVerdict::Passed => {}
        }
    }
    if has_cancelled {
        VerificationVerdict::Cancelled
    } else if has_unknown {
        VerificationVerdict::Unknown
    } else {
        VerificationVerdict::Passed
    }
}

#[allow(clippy::too_many_arguments)]
fn projection_digest(
    full: &FullCandidateVerification,
    ceiling: &ViewCeiling,
    scope: &EffectiveSourceScope,
    visible_verdict: VerificationVerdict,
    external_impact: ExternalImpact,
    full_candidate_passed: bool,
    affected_projects: &[ProjectedProjectVerification],
    test_cases: &[ProjectedTestCase],
    log_chunks: &[ProjectedLogChunk],
    diagnostics: &[ProjectedToolDiagnostic],
    artifacts: &[ProjectedOutputArtifact],
) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(
        &mut hasher,
        "schema",
        VERIFIER_PROJECTION_IDENTITY_SCHEMA.as_bytes(),
    );
    update_field(
        &mut hasher,
        "projection.schema",
        VERIFIER_PROJECTION_SCHEMA.as_bytes(),
    );
    update_field(&mut hasher, "repository", full.repository.as_str().as_bytes());
    update_external(&mut hasher, "base_revision", &full.base_revision);
    update_field(
        &mut hasher,
        "candidate_identity",
        full.candidate_identity.as_str().as_bytes(),
    );
    match &full.candidate_facts_identity {
        Some(id) => update_field(&mut hasher, "candidate_facts_identity", id.as_str().as_bytes()),
        None => update_field(&mut hasher, "candidate_facts_identity.none", b""),
    }
    update_field(&mut hasher, "ceiling", ceiling.identity().as_str().as_bytes());
    update_field(&mut hasher, "audience", scope.audience().as_bytes());
    update_field(
        &mut hasher,
        "effective_scope",
        scope.identity().as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "visible_verdict",
        visible_verdict.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "external_impact",
        external_impact.as_str().as_bytes(),
    );
    update_bool(&mut hasher, "full_candidate_passed", full_candidate_passed);

    update_usize(&mut hasher, "affected_projects", affected_projects.len());
    for project in affected_projects {
        update_field(&mut hasher, "project.id", project.project_id.as_bytes());
        update_field(
            &mut hasher,
            "project.verdict",
            project.verdict.as_str().as_bytes(),
        );
    }

    update_usize(&mut hasher, "test_cases", test_cases.len());
    for tc in test_cases {
        update_field(&mut hasher, "tc.project_id", tc.project_id.as_bytes());
        update_field(&mut hasher, "tc.name", tc.name.as_bytes());
        update_field(&mut hasher, "tc.verdict", tc.verdict.as_str().as_bytes());
        hasher.update(&tc.duration_ms.to_le_bytes());
        update_usize(&mut hasher, "tc.paths", tc.paths.len());
        for path in &tc.paths {
            update_field(&mut hasher, "tc.path", path.as_bytes());
        }
        match &tc.message {
            Some(msg) => update_field(&mut hasher, "tc.message", msg.as_bytes()),
            None => update_field(&mut hasher, "tc.message.none", b""),
        }
    }

    update_usize(&mut hasher, "log_chunks", log_chunks.len());
    for chunk in log_chunks {
        update_field(&mut hasher, "log.project_id", chunk.project_id.as_bytes());
        update_field(&mut hasher, "log.stream", chunk.stream.as_str().as_bytes());
        hasher.update(&chunk.sequence.to_le_bytes());
        update_field(&mut hasher, "log.text", chunk.text.as_bytes());
        update_usize(&mut hasher, "log.paths", chunk.paths.len());
        for path in &chunk.paths {
            update_field(&mut hasher, "log.path", path.as_bytes());
        }
    }

    update_usize(&mut hasher, "diagnostics", diagnostics.len());
    for diag in diagnostics {
        update_field(&mut hasher, "diag.project_id", diag.project_id.as_bytes());
        update_field(
            &mut hasher,
            "diag.severity",
            diag.severity.as_str().as_bytes(),
        );
        match &diag.code {
            Some(code) => update_field(&mut hasher, "diag.code", code.as_bytes()),
            None => update_field(&mut hasher, "diag.code.none", b""),
        }
        match &diag.file_path {
            Some(path) => update_field(&mut hasher, "diag.file_path", path.as_bytes()),
            None => update_field(&mut hasher, "diag.file_path.none", b""),
        }
        match diag.line {
            Some(line) => {
                update_bool(&mut hasher, "diag.has_line", true);
                hasher.update(&line.to_le_bytes());
            }
            None => update_bool(&mut hasher, "diag.has_line", false),
        }
        update_field(&mut hasher, "diag.message", diag.message.as_bytes());
    }

    update_usize(&mut hasher, "artifacts", artifacts.len());
    for artifact in artifacts {
        update_field(
            &mut hasher,
            "artifact.project_id",
            artifact.project_id.as_bytes(),
        );
        update_field(&mut hasher, "artifact.name", artifact.name.as_bytes());
        match &artifact.path {
            Some(path) => update_field(&mut hasher, "artifact.path", path.as_bytes()),
            None => update_field(&mut hasher, "artifact.path.none", b""),
        }
        update_field(
            &mut hasher,
            "artifact.digest",
            artifact.digest.as_str().as_bytes(),
        );
        hasher.update(&artifact.size_bytes.to_le_bytes());
    }

    finish(hasher)
}

fn valid_id(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn valid_multiline(value: &str, max_bytes: usize) -> bool {
    value.len() <= max_bytes
        && !value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementation::authority::AuthorityProvenance;
    use crate::projection::authority::{
        CapabilityContext, EffectiveScopeFacts, SessionCapability, SnapshotRule, SourceGrant,
    };
    use crate::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories, resolve,
    };
    use crate::projection::disclosure::{HandleKey, ScopedEnumeration, ScopedSelection, restrict};
    use crate::types::ProviderProfileId;
    use alloc::vec;

    fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|error| unreachable!("{error:?}"))
    }

    fn content(ch: char) -> ContentId {
        ok(ContentId::from_blake3_hex(ch.to_string().repeat(64)))
    }

    fn repo_and_revision() -> (ContentId, ExternalRef) {
        let enrolled = [RepositoryEnrollment {
            repository_id: "mono".into(),
            binding: ProviderBinding {
                source_provider: ok(ProviderProfileId::new("git-source-v1")),
                native_repository: ok(ExternalRef::new("git", "repository", "mono")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/srv/mono".into(),
            }],
        }];
        let admitted = ok(admit_repositories(&enrolled, &[]))
            .pop()
            .unwrap_or_else(|| unreachable!());
        let rev = ok(ExternalRef::new("git", "commit", &"a".repeat(40)));
        (admitted.identity().clone(), rev)
    }

    fn alpha_definition() -> AdmittedDefinition {
        ok(admit_definition(&ProjectionDefinition {
            schema: crate::projection::DEFINITION_SCHEMA.into(),
            id: "alpha-view".into(),
            profile: ProjectionProfile::Writable,
            selection: Selection {
                root: String::new(),
                include: vec!["projects/alpha/**".into(), "shared/**".into()],
                exclude: vec!["projects/alpha/src/secret.rs".into()],
            },
            mounts: vec![Mount {
                source_prefix: String::new(),
                destination_prefix: String::new(),
            }],
            dependencies: Vec::new(),
            creation_destinations: vec!["projects/alpha/src".into()],
            required_capabilities: vec![SourceCapability::ExactTree],
            generated_inputs: GeneratedInputs::Refused,
            target: TargetProfile::POSIX,
            compatibility: None,
        }))
    }

    fn scope_for(
        repo: &ContentId,
        rev: &ExternalRef,
        ceiling: &ViewCeiling,
        grants: Vec<SourceGrant>,
        not_after_s: u64,
    ) -> EffectiveSourceScope {
        let ctx = CapabilityContext {
            audience: "worker-alpha".into(),
            repository: repo.clone(),
            snapshot: SnapshotRule::Exact {
                revision: rev.clone(),
            },
            view_binding: ceiling.definition().clone(),
            generation: 1,
            revocation_generation: 0,
        };
        let capabilities = SessionCapability::under(&ctx, &grants);
        ok(EffectiveSourceScope::new(EffectiveScopeFacts {
            audience: "worker-alpha".into(),
            repository: repo.clone(),
            snapshot: SnapshotRule::Exact {
                revision: rev.clone(),
            },
            revision: rev.clone(),
            view_binding: ceiling.definition().clone(),
            admitted: grants,
            capabilities,
            not_after_s,
            depth: 1,
            generation: 1,
            revocation_generation: 0,
            provenance: AuthorityProvenance::VerifiedUcan,
        }))
    }

    fn alpha_scope(
        repo: &ContentId,
        rev: &ExternalRef,
        ceiling: &ViewCeiling,
    ) -> EffectiveSourceScope {
        scope_for(
            repo,
            rev,
            ceiling,
            vec![
                SourceGrant {
                    path: ok(PathScope::from_path("projects/alpha", true)
                        .ok_or("path")),
                    operations: BTreeSet::from([
                        SourceViewOperation::Discover,
                        SourceViewOperation::ReadContent,
                        SourceViewOperation::RetrieveResult,
                    ]),
                },
                SourceGrant {
                    path: ok(PathScope::from_path("shared", true).ok_or("path")),
                    operations: BTreeSet::from([
                        SourceViewOperation::Discover,
                        SourceViewOperation::ReadContent,
                    ]),
                },
            ],
            2_000,
        )
    }

    fn base_full_verification(repo: &ContentId, rev: &ExternalRef) -> FullCandidateVerification {
        FullCandidateVerification {
            schema: FULL_VERIFICATION_SCHEMA.into(),
            repository: repo.clone(),
            base_revision: rev.clone(),
            candidate_identity: content('1'),
            candidate_facts_identity: Some(content('2')),
            affected_projects: vec![
                ProjectVerification {
                    project_id: "alpha".into(),
                    scope: PathScope::from_path("projects/alpha", true)
                        .unwrap_or_else(|| unreachable!()),
                    verdict: VerificationVerdict::Passed,
                },
                ProjectVerification {
                    project_id: "beta".into(),
                    scope: PathScope::from_path("projects/beta", true)
                        .unwrap_or_else(|| unreachable!()),
                    verdict: VerificationVerdict::Passed,
                },
            ],
            test_cases: vec![
                TestCaseOutcome {
                    project_id: "alpha".into(),
                    name: "alpha::unit_ok".into(),
                    verdict: VerificationVerdict::Passed,
                    duration_ms: 12,
                    paths: vec!["projects/alpha/src/lib.rs".into()],
                    message: None,
                },
                TestCaseOutcome {
                    project_id: "alpha".into(),
                    name: "alpha::excluded_file_test".into(),
                    verdict: VerificationVerdict::Failed,
                    duration_ms: 4,
                    paths: vec!["projects/alpha/src/secret.rs".into()],
                    message: Some("excluded path leaked".into()),
                },
                TestCaseOutcome {
                    project_id: "alpha".into(),
                    name: "alpha::cross_project_test".into(),
                    verdict: VerificationVerdict::Failed,
                    duration_ms: 9,
                    paths: vec!["projects/beta/src/lib.rs".into()],
                    message: Some("beta secret".into()),
                },
                TestCaseOutcome {
                    project_id: "beta".into(),
                    name: "beta::hidden_test".into(),
                    verdict: VerificationVerdict::Passed,
                    duration_ms: 25,
                    paths: vec!["projects/beta/src/lib.rs".into()],
                    message: None,
                },
            ],
            log_chunks: vec![
                LogChunk {
                    project_id: "alpha".into(),
                    stream: LogStream::Stdout,
                    sequence: 1,
                    text: "alpha compiled cleanly\n".into(),
                    paths: vec!["projects/alpha/src/lib.rs".into()],
                },
                LogChunk {
                    project_id: "alpha".into(),
                    stream: LogStream::Stderr,
                    sequence: 2,
                    text: "shared read-only log without RetrieveResult\n".into(),
                    paths: vec!["shared/proto.rs".into()],
                },
                LogChunk {
                    project_id: "beta".into(),
                    stream: LogStream::Stderr,
                    sequence: 1,
                    text: "beta secret compiler output\n".into(),
                    paths: vec!["projects/beta/src/lib.rs".into()],
                },
            ],
            diagnostics: vec![
                ToolDiagnostic {
                    project_id: "alpha".into(),
                    severity: DiagnosticSeverity::Warning,
                    code: Some("W001".into()),
                    file_path: Some("projects/alpha/src/lib.rs".into()),
                    line: Some(14),
                    message: "unused variable".into(),
                },
                ToolDiagnostic {
                    project_id: "alpha".into(),
                    severity: DiagnosticSeverity::Error,
                    code: Some("E002".into()),
                    file_path: Some("projects/beta/src/lib.rs".into()),
                    line: Some(7),
                    message: "hidden file path diagnostic".into(),
                },
                ToolDiagnostic {
                    project_id: "beta".into(),
                    severity: DiagnosticSeverity::Error,
                    code: Some("E999".into()),
                    file_path: Some("projects/beta/src/lib.rs".into()),
                    line: Some(3),
                    message: "beta internal error".into(),
                },
            ],
            artifacts: vec![
                OutputArtifact {
                    project_id: "alpha".into(),
                    name: "alpha-test-report.json".into(),
                    path: Some("projects/alpha/src/lib.rs".into()),
                    digest: content('3'),
                    size_bytes: 512,
                },
                OutputArtifact {
                    project_id: "beta".into(),
                    name: "beta-binary".into(),
                    path: Some("projects/beta/src/lib.rs".into()),
                    digest: content('4'),
                    size_bytes: 4096,
                },
            ],
        }
    }

    #[test]
    fn projects_only_authorized_items_and_enforces_strict_non_interference() {
        let (repo, rev) = repo_and_revision();
        let def = alpha_definition();
        let ceiling = ViewCeiling::from_definition(&def);
        let scope = alpha_scope(&repo, &rev, &ceiling);

        let full_one_hidden_pass = base_full_verification(&repo, &rev);
        let projected_one = ok(project_for_definition(
            &full_one_hidden_pass,
            &def,
            &ceiling,
            &scope,
            1_000,
        ));

        assert_eq!(projected_one.affected_projects().len(), 1);
        assert_eq!(projected_one.affected_projects()[0].project_id, "alpha");
        assert_eq!(projected_one.test_cases().len(), 1);
        assert_eq!(projected_one.test_cases()[0].name, "alpha::unit_ok");
        assert_eq!(projected_one.log_chunks().len(), 1);
        assert_eq!(
            projected_one.log_chunks()[0].text,
            "alpha compiled cleanly\n"
        );
        assert_eq!(projected_one.diagnostics().len(), 1);
        assert_eq!(projected_one.diagnostics()[0].message, "unused variable");
        assert_eq!(projected_one.artifacts().len(), 1);
        assert_eq!(projected_one.artifacts()[0].name, "alpha-test-report.json");
        assert_eq!(projected_one.visible_verdict(), VerificationVerdict::Passed);
        assert_eq!(
            projected_one.external_impact(),
            ExternalImpact::NoneReported
        );
        assert!(projected_one.full_candidate_passed());

        // Non-interference: add many more hidden projects, tests, logs, diagnostics, artifacts (all passing)
        // and mutate hidden project names and paths. Output MUST be byte-identical.
        let mut full_many_hidden_pass = full_one_hidden_pass.clone();
        full_many_hidden_pass.affected_projects[1].project_id = "zeta-renamed".into();
        full_many_hidden_pass.affected_projects.push(ProjectVerification {
            project_id: "gamma".into(),
            scope: PathScope::from_path("projects/gamma", true).unwrap_or_else(|| unreachable!()),
            verdict: VerificationVerdict::Passed,
        });
        full_many_hidden_pass.affected_projects.push(ProjectVerification {
            project_id: "delta".into(),
            scope: PathScope::root(),
            verdict: VerificationVerdict::Passed,
        });
        for idx in 0..20 {
            full_many_hidden_pass.test_cases.push(TestCaseOutcome {
                project_id: "gamma".into(),
                name: alloc::format!("gamma::secret_test_{idx}"),
                verdict: VerificationVerdict::Passed,
                duration_ms: 99,
                paths: vec![alloc::format!("projects/gamma/file_{idx}.rs")],
                message: Some("hidden message".into()),
            });
            full_many_hidden_pass.log_chunks.push(LogChunk {
                project_id: "zeta-renamed".into(),
                stream: LogStream::Stderr,
                sequence: idx as u64 + 10,
                text: alloc::format!("hidden log line {idx}"),
                paths: vec![],
            });
            full_many_hidden_pass.diagnostics.push(ToolDiagnostic {
                project_id: "delta".into(),
                severity: DiagnosticSeverity::Warning,
                code: Some("W999".into()),
                file_path: Some("projects/delta/secret.rs".into()),
                line: Some(idx as u32 + 1),
                message: "hidden diagnostic".into(),
            });
            full_many_hidden_pass.artifacts.push(OutputArtifact {
                project_id: "gamma".into(),
                name: alloc::format!("gamma-art-{idx}"),
                path: None,
                digest: content('9'),
                size_bytes: 1234,
            });
        }

        let projected_many = ok(project_for_definition(
            &full_many_hidden_pass,
            &def,
            &ceiling,
            &scope,
            1_000,
        ));
        assert_eq!(projected_one.digest(), projected_many.digest());
        assert_eq!(
            ok(serde_json::to_vec(&projected_one)),
            ok(serde_json::to_vec(&projected_many))
        );

        // Now fail 1 hidden project vs 3 hidden projects -> opaque ExternalChecksFailed,
        // and both 1-failing and 3-failing must be byte-identical to each other!
        let mut full_one_fail = full_one_hidden_pass.clone();
        full_one_fail.affected_projects[1].verdict = VerificationVerdict::Failed;
        let projected_one_fail = ok(project_for_definition(
            &full_one_fail,
            &def,
            &ceiling,
            &scope,
            1_000,
        ));

        let mut full_many_fail = full_many_hidden_pass;
        full_many_fail.affected_projects[1].verdict = VerificationVerdict::Failed;
        full_many_fail.affected_projects[2].verdict = VerificationVerdict::Cancelled;
        full_many_fail.affected_projects[3].verdict = VerificationVerdict::Unknown;
        let projected_many_fail = ok(project_for_definition(
            &full_many_fail,
            &def,
            &ceiling,
            &scope,
            1_000,
        ));

        assert_eq!(
            projected_one_fail.visible_verdict(),
            VerificationVerdict::Passed
        );
        assert_eq!(
            projected_one_fail.external_impact(),
            ExternalImpact::ExternalChecksFailed
        );
        assert!(!projected_one_fail.full_candidate_passed());
        assert_eq!(projected_one_fail.digest(), projected_many_fail.digest());
        assert_eq!(
            ok(serde_json::to_vec(&projected_one_fail)),
            ok(serde_json::to_vec(&projected_many_fail))
        );

        // Visible items remain unchanged across external pass vs external fail!
        assert_eq!(
            projected_one.affected_projects(),
            projected_one_fail.affected_projects()
        );
        assert_eq!(projected_one.test_cases(), projected_one_fail.test_cases());
        assert_eq!(projected_one.log_chunks(), projected_one_fail.log_chunks());
        assert_eq!(
            projected_one.diagnostics(),
            projected_one_fail.diagnostics()
        );
        assert_eq!(projected_one.artifacts(), projected_one_fail.artifacts());
    }

    #[test]
    fn audience_view_projection_and_negative_authority_and_bound_refusals() {
        let (repo, rev) = repo_and_revision();
        let def = alpha_definition();
        let ceiling = ViewCeiling::from_definition(&def);
        let scope = alpha_scope(&repo, &rev, &ceiling);
        let facts = ProjectionFacts {
            repository: repo.clone(),
            exact_revision: rev.clone(),
            canonical_tree: content('7'),
            evaluator: ComponentVersion {
                name: "kiln".into(),
                version: "1".into(),
            },
            adapter: ComponentVersion {
                name: "git".into(),
                version: "1".into(),
            },
            source_capabilities: vec![SourceCapability::ExactTree],
            coverage: Coverage::Complete,
            entries: vec![SourceEntry {
                path: "projects/alpha/src/lib.rs".into(),
                object: ok(ExternalRef::new("git", "blob", &"b".repeat(40))),
                kind: EntryKind::File,
            }],
            dependencies: Vec::new(),
            generated: Vec::new(),
        };
        let enrolled = [RepositoryEnrollment {
            repository_id: "mono".into(),
            binding: ProviderBinding {
                source_provider: ok(ProviderProfileId::new("git-source-v1")),
                native_repository: ok(ExternalRef::new("git", "repository", "mono")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/srv/mono".into(),
            }],
        }];
        let admitted_repo = ok(admit_repositories(&enrolled, &[]))
            .pop()
            .unwrap_or_else(|| unreachable!());
        let selection = ScopedSelection::for_holder(&scope, &ceiling);
        let filtered = ok(selection.filter_facts(&facts));
        let partial = ok(resolve(&def, &admitted_repo, &filtered));
        let enumeration = ok(ScopedEnumeration::new(&selection, partial));
        let key = HandleKey::from_bytes([0x11; 32]);
        let view = ok(restrict(&enumeration, &scope, &ceiling, &key, 1_000));

        let mut full = base_full_verification(&repo, &rev);
        // Remove excluded path test so project_for_audience (without definition exclude) matches only authorized paths.
        full.test_cases
            .retain(|tc| tc.name != "alpha::excluded_file_test");

        let via_audience = ok(project_for_audience(&full, &view, &scope, 1_000));
        assert_eq!(via_audience.affected_projects().len(), 1);
        assert_eq!(via_audience.test_cases().len(), 1);

        // Expired scope fails closed
        assert_eq!(
            project_for_audience(&full, &view, &scope, 2_000),
            Err(ProjectionRefusal::Stale)
        );
        assert_eq!(
            project_for_scope(&full, &ceiling, &scope, 2_000),
            Err(ProjectionRefusal::Expired)
        );

        // Navigation ceiling lacks RetrieveResult
        let nav_def = ok(admit_definition(&ProjectionDefinition {
            schema: crate::projection::DEFINITION_SCHEMA.into(),
            id: "alpha-nav".into(),
            profile: ProjectionProfile::Navigation,
            selection: Selection {
                root: String::new(),
                include: vec!["projects/alpha/**".into()],
                exclude: Vec::new(),
            },
            mounts: vec![Mount {
                source_prefix: String::new(),
                destination_prefix: String::new(),
            }],
            dependencies: Vec::new(),
            creation_destinations: Vec::new(),
            required_capabilities: Vec::new(),
            generated_inputs: GeneratedInputs::Refused,
            target: TargetProfile::POSIX,
            compatibility: None,
        }));
        let nav_ceiling = ViewCeiling::from_definition(&nav_def);
        assert_eq!(
            project_for_scope(&full, &nav_ceiling, &scope, 1_000),
            Err(ProjectionRefusal::NotAuthorized)
        );

        // Discover-only scope without RetrieveResult fails closed
        let discover_only = scope_for(
            &repo,
            &rev,
            &ceiling,
            vec![SourceGrant {
                path: PathScope::from_path("projects/alpha", true)
                    .unwrap_or_else(|| unreachable!()),
                operations: BTreeSet::from([SourceViewOperation::Discover]),
            }],
            2_000,
        );
        assert_eq!(
            project_for_scope(&full, &ceiling, &discover_only, 1_000),
            Err(ProjectionRefusal::NotAuthorized)
        );

        // Repository / snapshot mismatch fails closed
        let mut wrong_repo = full.clone();
        wrong_repo.repository = content('f');
        assert_eq!(
            project_for_scope(&wrong_repo, &ceiling, &scope, 1_000),
            Err(ProjectionRefusal::RepositoryMismatch)
        );
        let mut wrong_rev = full.clone();
        wrong_rev.base_revision = ok(ExternalRef::new("git", "commit", &"c".repeat(40)));
        assert_eq!(
            project_for_scope(&wrong_rev, &ceiling, &scope, 1_000),
            Err(ProjectionRefusal::SnapshotMismatch)
        );

        // Path traversal / malformed input fails closed
        let mut bad_path = full.clone();
        bad_path.diagnostics[0].file_path = Some("../secret.rs".into());
        assert_eq!(
            project_for_scope(&bad_path, &ceiling, &scope, 1_000),
            Err(ProjectionRefusal::InvalidInput)
        );

        // Cannot deserialize ProjectedVerificationResult (non-forgeable)
        let json = ok(serde_json::to_string(&via_audience));
        assert!(json.contains(VERIFIER_PROJECTION_SCHEMA));
        assert!(!json.contains("beta"));
    }
}
