//! Controller-owned verification. No worker manifest is an execution observation.
//!
//! Invoke this API in an operator-owned process/UID, with scratch and profile
//! inaccessible to the editing worker. This module never switches credentials.
//! r[impl kiln.implementation.verification.scenario.independent]
//! r[impl kiln.implementation.verification.scenario.forged]
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use kiln_core::change_review::{CiCheck, CiFact};
use kiln_core::implementation::proof_evidence::{
    ProofBackend, ProofEvidenceManifest, ProofVerificationPolicy, RetainedProofArtifact,
    expected_binding, validate_manifest,
};
use kiln_core::implementation::{
    CandidateHandoff, CandidateVerification, ImplementationAttempt, ImplementationWork, SourceRef,
    TerminalPolicy, VERIFICATION_SCHEMA, WorkSourceKind, admit_candidate_handoff,
    admit_candidate_verification,
};
use kiln_core::types::ContentId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod capture;
pub use capture::{
    HERDR_CANDIDATE_RECEIPT_SCHEMA, HerdrCandidateDisposition, HerdrCandidateReceipt,
    materialize_candidate,
};
mod process;
mod source;

const PROFILE_SCHEMA: &str = "kiln.verification-profile.v1";
const MAX_CHECKS: usize = 64;
const MAX_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenProgram {
    pub path: PathBuf,
    pub blake3: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCheck {
    pub id: String,
    pub kind: String,
    pub program: FrozenProgram,
    pub arguments: Vec<String>,
    pub prerequisites: Vec<String>,
    /// Every acceptance obligation must be covered by at least one executed check.
    pub acceptance: Vec<String>,
    /// Repository-relative outputs, absent before execution and required afterwards.
    pub artifacts: Vec<String>,
    pub timeout_ms: u64,
}
impl VerificationCheck {
    /// Stable identity of the selected protected invocation; callers can freeze
    /// this into a proof policy without accepting worker-authored command data.
    pub fn invocation_identity(&self) -> Result<String, String> {
        Ok(digest(
            &serde_json::to_vec(&json!({
                "id": self.id.as_str(),
                "kind": self.kind.as_str(),
                "program_path": self.program.path.to_string_lossy(),
                "program_blake3": self.program.blake3.as_str(),
                "arguments": &self.arguments,
                "prerequisites": &self.prerequisites,
                "acceptance": &self.acceptance,
                "artifacts": &self.artifacts,
                "timeout_ms": self.timeout_ms,
            }))
            .map_err(text)?,
        ))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnAcceptance {
    /// Exact change/task aliases and source/spec versions admitted with work.
    pub aliases: Vec<SourceRef>,
    /// Pinned AgentVerificationContract template. final_subject_hash must be empty;
    /// the controller fills it from independently resolved source and outer work.
    pub contract: Value,
    /// Immutable reviewed JSON material; content is re-read by the adapter.
    pub registry: FrozenProgram,
    pub policy: FrozenProgram,
    /// Exact `.cairn` Git tree prepared and admitted by the trusted controller
    /// before review. None requires baseline equality. This is source
    /// authorization, not evidence that a lifecycle command executed.
    #[serde(default)]
    pub prepared_lifecycle_tree: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationProfile {
    pub schema: String,
    pub repository: String,
    pub toolchain: ContentId,
    pub environment_identity: ContentId,
    pub environment: BTreeMap<String, String>,
    pub checks: Vec<VerificationCheck>,
    /// For bug profiles, selected check ID; identical argv runs baseline/candidate.
    pub reproducer: Option<String>,
    /// Frozen requirements, verifier definitions and policy. Every entry must
    /// exist at baseline and have identical Git object identity in candidate.
    pub protected_paths: Vec<String>,
    pub allow_already_satisfied: bool,
    pub cairn: Option<CairnAcceptance>,
    /// Independently approved proof policy; never sourced from the candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<ProofVerificationPolicy>,
}

impl VerificationProfile {
    pub fn identity(&self) -> Result<ContentId, String> {
        ContentId::from_blake3_hex(digest(&serde_json::to_vec(self).map_err(text)?)).map_err(text)
    }

    pub fn derived_environment_identity(&self) -> Result<ContentId, String> {
        ContentId::from_blake3_hex(digest(
            &serde_json::to_vec(&self.environment).map_err(text)?,
        ))
        .map_err(text)
    }

    /// Identity of the actually hash-checked verifier program cohort. Complete
    /// runtime closures must be supplied as immutable Nix-store programs.
    pub fn derived_toolchain_identity(&self) -> Result<ContentId, String> {
        let programs: BTreeMap<_, _> = self
            .checks
            .iter()
            .map(|check| (&check.program.path, &check.program.blake3))
            .collect();
        ContentId::from_blake3_hex(digest(&serde_json::to_vec(&programs).map_err(text)?))
            .map_err(text)
    }
}

/// Trusted enrollment, not input supplied by a worker. The repository directory
/// is operator-owned acquisition; repository labels alone are not enrollment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifierAuthority {
    pub repository: String,
    pub repository_root: PathBuf,
    pub scratch_root: PathBuf,
    pub git: FrozenProgram,
    pub worker_uid: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtifactObservation {
    pub path: String,
    pub hash: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckObservation {
    pub check_id: String,
    pub command: Vec<String>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub stdout_hash: String,
    pub stderr_hash: String,
    pub artifacts: Vec<ArtifactObservation>,
    pub evidence_hash: String,
    pub execution_receipt_hash: String,
    pub baseline: bool,
}

/// Constructible only by real execution, deliberately not Deserialize.
#[derive(Debug, Serialize)]
pub struct VerificationRun {
    profile: VerificationProfile,
    handoff: CandidateHandoff,
    outer_binding: ContentId,
    subject_hash: String,
    run: CiFact,
    observations: Vec<CheckObservation>,
    already_satisfied: bool,
    checks_executed: bool,
    evidence_directory: PathBuf,
    authority: VerifierAuthority,
}

impl VerificationRun {
    pub fn profile(&self) -> &VerificationProfile {
        &self.profile
    }
    pub fn handoff(&self) -> &CandidateHandoff {
        &self.handoff
    }
    pub fn run(&self) -> &CiFact {
        &self.run
    }
    pub fn observations(&self) -> &[CheckObservation] {
        &self.observations
    }
    pub fn subject_hash(&self) -> &str {
        &self.subject_hash
    }
    pub fn checks_executed(&self) -> bool {
        self.checks_executed
    }
    pub fn evidence_directory(&self) -> &Path {
        &self.evidence_directory
    }
    pub fn authority(&self) -> &VerifierAuthority {
        &self.authority
    }

    /// Actual retained artifact bytes for canonical Engine ingestion.
    pub fn check_artifact(&self, check_id: &str) -> Result<Vec<u8>, String> {
        let row = self
            .observations
            .iter()
            .find(|row| !row.baseline && row.check_id == check_id)
            .ok_or("check absent from trusted execution")?;
        let bytes = if row.status == "blocked" {
            b"blocked".to_vec()
        } else {
            bounded_file(
                &self
                    .evidence_directory
                    .join(format!("candidate-{check_id}/evidence.json")),
                MAX_ARTIFACT_BYTES,
            )?
        };
        if digest(&bytes) != row.evidence_hash {
            return Err("retained check evidence material differs".into());
        }
        Ok(bytes)
    }

    /// Non-Cairn admission only. Cairn callers must use the authoritative adapter.
    pub fn candidate_verification(
        &self,
        work: &ImplementationWork,
        attempt: &ImplementationAttempt,
        handoff: &CandidateHandoff,
    ) -> Result<CandidateVerification, String> {
        if work
            .aliases
            .iter()
            .any(|alias| alias.kind == WorkSourceKind::CairnTask)
        {
            return Err("Cairn work requires authoritative Cairn completion".into());
        }
        self.bound_candidate_verification(work, attempt, handoff)
    }

    fn bound_candidate_verification(
        &self,
        work: &ImplementationWork,
        attempt: &ImplementationAttempt,
        handoff: &CandidateHandoff,
    ) -> Result<CandidateVerification, String> {
        self.validate_retained_evidence()?;
        let binding = admit_candidate_handoff(work, attempt, handoff).map_err(text)?;
        if binding != self.outer_binding || handoff != &self.handoff || !self.checks_executed {
            return Err(
                "verification cannot transfer to another work/version/attempt/candidate".into(),
            );
        }
        let result = CandidateVerification {
            schema: VERIFICATION_SCHEMA.into(),
            handoff: binding,
            work_identity: handoff.work_identity.clone(),
            work_revision: handoff.work_revision,
            attempt: handoff.attempt_identity.clone(),
            fence: handoff.fence,
            candidate: handoff.candidate.clone(),
            tree: handoff.tree.clone(),
            base: handoff.base.clone(),
            verification_profile: handoff.verification_profile.clone(),
            run: self.run.clone(),
        };
        admit_candidate_verification(work, attempt, handoff, &result).map_err(text)?;
        Ok(result)
    }

    /// Recheck only the outer binding. This does not confer completion authority.
    pub fn assert_binding(
        &self,
        work: &ImplementationWork,
        attempt: &ImplementationAttempt,
        handoff: &CandidateHandoff,
    ) -> Result<(), String> {
        self.validate_retained_evidence()?;
        let binding = admit_candidate_handoff(work, attempt, handoff).map_err(text)?;
        if binding != self.outer_binding || handoff != &self.handoff || !self.checks_executed {
            return Err("stale outer verification binding".into());
        }
        Ok(())
    }

    /// Re-read controller-retained bytes before granting evidence authority.
    pub fn validate_retained_evidence(&self) -> Result<(), String> {
        for row in &self.observations {
            if row.status == "blocked" {
                continue;
            }
            let directory = self.evidence_directory.join(format!(
                "{}-{}",
                if row.baseline {
                    "baseline"
                } else {
                    "candidate"
                },
                row.check_id
            ));
            if bounded_file(&directory.join("receipt.json"), MAX_ARTIFACT_BYTES)?
                != serde_json::to_vec_pretty(row).map_err(text)?
                || digest(&bounded_file(
                    &directory.join("stdout"),
                    MAX_ARTIFACT_BYTES,
                )?) != row.stdout_hash
                || digest(&bounded_file(
                    &directory.join("stderr"),
                    MAX_ARTIFACT_BYTES,
                )?) != row.stderr_hash
            {
                return Err("retained execution receipt or output was substituted".into());
            }
            for artifact in &row.artifacts {
                let bytes = retained_artifact(&directory.join("artifacts"), &artifact.path)?;
                if digest(&bytes) != artifact.hash || bytes.len() as u64 != artifact.bytes {
                    return Err("retained artifact digest differs".into());
                }
            }
        }
        Ok(())
    }
}

/// Execute, retain and project a frozen admitted profile. A successful return may
/// contain failed/unknown observations; only admission grants verification.
// r[impl kiln.implementation.verification]
pub fn verify_candidate(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    handoff: &CandidateHandoff,
    profile: &VerificationProfile,
    authority: &VerifierAuthority,
) -> Result<VerificationRun, String> {
    let outer_binding = admit_candidate_handoff(work, attempt, handoff).map_err(text)?;
    validate_profile(work, handoff, profile)?;
    source::admit_authority(authority, work)?;
    let facts = source::resolve(authority, handoff)?;
    if facts.paths
        != handoff
            .changed_paths
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
        || facts.tree != handoff.tree.native_id.as_str()
    {
        return Err("independent candidate tree or complete changed paths differ".into());
    }
    source::protect(
        authority,
        handoff,
        &profile.protected_paths,
        profile
            .cairn
            .as_ref()
            .and_then(|cairn| cairn.prepared_lifecycle_tree.as_deref()),
    )?;
    let already_satisfied = facts.paths.is_empty();
    if already_satisfied
        && (!profile.allow_already_satisfied
            || work.terminal_policy != TerminalPolicy::VerifiedOrAlreadySatisfied
            || handoff.base.as_ref() != Some(&handoff.candidate))
    {
        return Err("empty patch has no explicit already-satisfied authority".into());
    }
    let subject_hash = digest(
        &serde_json::to_vec(&json!({
            "handoff": outer_binding, "aliases": work.aliases, "tree": facts.tree,
        }))
        .map_err(text)?,
    );
    // create_dir is an atomic replay barrier. Never overwrite or rerun an uncertain run.
    let evidence_directory = authority.scratch_root.join(outer_binding.as_str());
    fs::create_dir(&evidence_directory).map_err(|error| {
        format!("verification already reserved or cannot create evidence: {error}")
    })?;
    fs::write(
        evidence_directory.join("reservation.json"),
        serde_json::to_vec(&json!({
            "handoff": outer_binding, "subject": subject_hash, "state": "unknown_until_observed",
        }))
        .map_err(text)?,
    )
    .map_err(text)?;
    let mut observations = Vec::new();
    if let Some(reproducer) = &profile.reproducer {
        let check = profile
            .checks
            .iter()
            .find(|check| &check.id == reproducer)
            .ok_or("missing reproducer")?;
        observations.push(execute_check(
            authority,
            handoff,
            profile,
            check,
            true,
            &evidence_directory,
        )?);
    }
    let mut pending: BTreeSet<_> = profile
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect();
    while !pending.is_empty() {
        let next = profile
            .checks
            .iter()
            .find(|check| {
                pending.contains(check.id.as_str())
                    && check
                        .prerequisites
                        .iter()
                        .all(|id| !pending.contains(id.as_str()))
            })
            .ok_or("verification prerequisite cycle")?;
        let mut observation = if next.prerequisites.iter().all(|id| {
            observations
                .iter()
                .any(|row| !row.baseline && &row.check_id == id && row.status == "passed")
        }) {
            execute_check(
                authority,
                handoff,
                profile,
                next,
                false,
                &evidence_directory,
            )?
        } else {
            blocked_observation(next)
        };
        if let Some(policy) = &profile.proof
            && !observation.baseline
            && observation.status == "passed"
            && next.id == policy.adapter_check_id
        {
            if validate_proof_result(
                work,
                attempt,
                handoff,
                profile,
                policy,
                &observation,
                &evidence_directory,
            )
            .is_err()
            {
                observation.status = if !policy.qualified || policy.backend == ProofBackend::Mm0 {
                    "unsupported"
                } else {
                    "failed"
                }
                .into();
            }
            let manifest_hash = observation
                .artifacts
                .iter()
                .find(|artifact| artifact.path == policy.manifest_artifact)
                .map(|artifact| artifact.hash.as_str())
                .ok_or("proof manifest artifact absent from retained observation")?;
            observation.execution_receipt_hash = digest(
                &serde_json::to_vec(&json!({
                    "execution": observation.execution_receipt_hash,
                    "profile": profile.identity()?,
                    "manifest": manifest_hash,
                    "status": observation.status.as_str(),
                }))
                .map_err(text)?,
            );
            fs::write(
                evidence_directory.join(format!("candidate-{}/receipt.json", observation.check_id)),
                serde_json::to_vec_pretty(&observation).map_err(text)?,
            )
            .map_err(text)?;
        }
        pending.remove(next.id.as_str());
        observations.push(observation);
    }
    let baseline_ok = profile.reproducer.as_ref().is_none_or(|id| {
        observations.iter().any(|row| {
            row.baseline
                && &row.check_id == id
                && if already_satisfied {
                    row.status == "passed" && row.exit_code == Some(0)
                } else {
                    row.status == "failed" && row.exit_code.is_some_and(|exit| exit > 0)
                }
        })
    });
    let checks: Vec<_> = observations
        .iter()
        .filter(|row| !row.baseline)
        .map(|row| CiCheck {
            name: row.check_id.clone(),
            kind: profile
                .checks
                .iter()
                .find(|check| check.id == row.check_id)
                .map(|check| check.kind.clone())
                .unwrap_or_default(),
            outcome: if row.status == "passed" && baseline_ok {
                "pass"
            } else if row.status == "unknown" || row.status == "timed_out" {
                "unknown"
            } else {
                "fail"
            }
            .into(),
            artifact_digest: Some(row.evidence_hash.clone()),
        })
        .collect();
    let run = CiFact {
        project: work.project.clone(),
        repository: work.repository.clone(),
        revision: handoff.candidate.native_id.as_str().into(),
        run_id: format!("implementation-verification:{}", outer_binding.as_str()),
        ordinal: attempt.ordinal.into(),
        terminal: true,
        success: baseline_ok && checks.iter().all(|check| check.outcome == "pass"),
        full_run: true,
        clean: true,
        scope_identity: Some(outer_binding.as_str().into()),
        untrusted: None,
        checks,
    };
    let report = VerificationRun {
        profile: profile.clone(),
        handoff: handoff.clone(),
        outer_binding,
        subject_hash,
        run,
        observations,
        already_satisfied,
        checks_executed: true,
        evidence_directory,
        authority: authority.clone(),
    };
    fs::write(
        report.evidence_directory.join("observations.json"),
        serde_json::to_vec_pretty(&report).map_err(text)?,
    )
    .map_err(text)?;
    Ok(report)
}

fn validate_proof_result(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    handoff: &CandidateHandoff,
    profile: &VerificationProfile,
    policy: &ProofVerificationPolicy,
    observation: &CheckObservation,
    evidence_directory: &Path,
) -> Result<(), String> {
    if observation.status != "passed" || observation.exit_code != Some(0) {
        return Err("proof adapter did not complete successfully".into());
    }
    let output_root =
        evidence_directory.join(format!("candidate-{}/artifacts", observation.check_id));
    let source_root = evidence_directory.join(format!("candidate-{}/source", observation.check_id));
    for input in &policy.approved_inputs {
        let bytes = retained_artifact(&source_root, &input.path)?;
        if digest(&bytes) != input.blake3 {
            return Err(format!(
                "approved proof input {} differs from captured candidate",
                input.id
            ));
        }
    }
    let manifest_bytes = retained_artifact(&output_root, &policy.manifest_artifact)?;
    let adapter_check = profile
        .checks
        .iter()
        .find(|check| check.id == policy.adapter_check_id)
        .ok_or("named proof adapter check disappeared from trusted profile")?;
    let invocation = adapter_check.invocation_identity()?;
    if invocation != policy.adapter_invocation_blake3 {
        return Err("proof adapter invocation differs from trusted profile identity".into());
    }
    if manifest_bytes.len() as u64 > MAX_ARTIFACT_BYTES {
        return Err("proof manifest exceeds retained artifact bound".into());
    }
    let proof_bytes = retained_artifact(&output_root, &policy.proof_artifact)?;
    let proof_observation = observation
        .artifacts
        .iter()
        .find(|artifact| artifact.path == policy.proof_artifact)
        .ok_or("proof bytes are absent from actual check observation")?;
    let manifest_observation = observation
        .artifacts
        .iter()
        .find(|artifact| artifact.path == policy.manifest_artifact)
        .ok_or("proof manifest is absent from actual check observation")?;
    if digest(&manifest_bytes) != manifest_observation.hash
        || manifest_bytes.len() as u64 != manifest_observation.bytes
    {
        return Err("manifest bytes differ from actual retained check artifact".into());
    }
    let manifest: ProofEvidenceManifest = serde_json::from_slice(&manifest_bytes).map_err(text)?;
    if manifest.proof_artifact.path != policy.proof_artifact
        || manifest.proof_artifact.blake3 != digest(&proof_bytes)
        || manifest.proof_artifact.bytes != proof_bytes.len() as u64
        || proof_observation.hash != digest(&proof_bytes)
        || proof_observation.bytes != proof_bytes.len() as u64
    {
        return Err("manifest proof artifact differs from controller-retained bytes".into());
    }
    let expected = expected_binding(
        work,
        attempt,
        handoff,
        profile.identity()?,
        RetainedProofArtifact {
            path: policy.proof_artifact.clone(),
            blake3: proof_observation.hash.clone(),
            bytes: proof_observation.bytes,
        },
    )
    .map_err(text)?;
    validate_manifest(policy, &expected, &manifest).map_err(text)
}

fn proof_policy_error(
    policy: &ProofVerificationPolicy,
    profile: &VerificationProfile,
) -> Result<(), String> {
    if policy.manifest_artifact == policy.proof_artifact
        || !safe_relative(&policy.manifest_artifact)
        || !safe_relative(&policy.proof_artifact)
        || policy.required_checks.is_empty()
        || !kiln_core::implementation::proof_evidence::valid_blake3(
            &policy.adapter_invocation_blake3,
        )
        || policy.required_checks.len()
            > kiln_core::implementation::proof_evidence::MAX_PROOF_CHECKS
        || policy.approved_inputs.is_empty()
        || policy.approved_inputs.len()
            > kiln_core::implementation::proof_evidence::MAX_PROOF_IDENTITIES
        || policy.approved_inputs.iter().any(|input| {
            !safe_relative(&input.path) || !profile.protected_paths.contains(&input.path)
        })
    {
        return Err("malformed frozen proof policy".into());
    }
    let check = profile
        .checks
        .iter()
        .find(|check| {
            check.id == policy.adapter_check_id
                && check.artifacts.contains(&policy.manifest_artifact)
        })
        .ok_or("proof manifest artifact is not produced by the named frozen adapter check")?;
    if !check.artifacts.contains(&policy.proof_artifact)
        || check.acceptance.is_empty()
        || check.kind != "proof"
    {
        return Err(
            "proof artifacts must be retained by the named acceptance-covering proof check".into(),
        );
    }
    if check.invocation_identity()? != policy.adapter_invocation_blake3 {
        return Err("proof adapter invocation identity differs from frozen program command".into());
    }
    Ok(())
}

fn validate_profile(
    work: &ImplementationWork,
    handoff: &CandidateHandoff,
    profile: &VerificationProfile,
) -> Result<(), String> {
    if profile.schema != PROFILE_SCHEMA
        || profile.identity()? != work.verification_profile
        || profile.repository != work.repository
        || profile.toolchain != handoff.toolchain
        || profile.environment_identity != handoff.environment
        || profile.checks.is_empty()
        || profile.environment_identity != profile.derived_environment_identity()?
        || profile.toolchain != profile.derived_toolchain_identity()?
        || profile.checks.len() > MAX_CHECKS
        || profile.protected_paths.is_empty()
        || (profile.cairn.is_some() && profile.checks.len() == MAX_CHECKS)
    {
        return Err("profile differs from frozen work/toolchain/environment".into());
    }
    let ids: BTreeSet<_> = profile
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect();
    if ids.len() != profile.checks.len() {
        return Err("duplicate profile checks".into());
    }
    let obligations: BTreeSet<_> = work
        .acceptance
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let mut covered = BTreeSet::new();
    for check in &profile.checks {
        if !safe_label(&check.id)
            || !safe_label(&check.kind)
            || check.timeout_ms == 0
            || check.id == "cairn-authoritative-completion"
            || check.timeout_ms > 86_400_000
            || check.artifacts.is_empty()
            || check.artifacts.len() > MAX_CHECKS
            || check.arguments.len() > 64
            || check.arguments.iter().any(|arg| arg.contains('\0'))
            || check
                .prerequisites
                .iter()
                .any(|id| id == &check.id || !ids.contains(id.as_str()))
        {
            return Err("malformed verification check".into());
        }
        let unique: BTreeSet<_> = check.artifacts.iter().collect();
        if unique.len() != check.artifacts.len()
            || check.artifacts.iter().any(|path| !safe_relative(path))
        {
            return Err("unsafe or duplicate required artifacts".into());
        }
        for obligation in &check.acceptance {
            if !obligations.contains(obligation.as_str()) {
                return Err("unknown acceptance case".into());
            }
            covered.insert(obligation.as_str());
        }
        process::admit_program(&check.program)?;
    }
    if covered != obligations {
        return Err("profile leaves acceptance cases uncovered".into());
    }
    if profile
        .reproducer
        .as_ref()
        .is_some_and(|id| !ids.contains(id.as_str()))
    {
        return Err("bug profile has no selected reproducer".into());
    }
    if profile
        .protected_paths
        .iter()
        .any(|path| !safe_relative(path))
    {
        return Err("unsafe protected path".into());
    }
    if work
        .aliases
        .iter()
        .any(|alias| alias.kind == WorkSourceKind::CairnTask)
    {
        let cairn = profile
            .cairn
            .as_ref()
            .ok_or("Cairn work lacks trusted authoritative contract")?;
        if cairn.aliases != work.aliases
            || !profile.protected_paths.iter().any(|path| path == ".cairn")
        {
            return Err("Cairn exact aliases/spec versions or lifecycle protection missing".into());
        }
    }
    if let Some(policy) = &profile.proof {
        proof_policy_error(policy, profile)?;
    }
    Ok(())
}

fn execute_check(
    authority: &VerifierAuthority,
    handoff: &CandidateHandoff,
    profile: &VerificationProfile,
    check: &VerificationCheck,
    baseline: bool,
    evidence: &Path,
) -> Result<CheckObservation, String> {
    let label = format!(
        "{}-{}",
        if baseline { "baseline" } else { "candidate" },
        check.id
    );
    let directory = evidence.join(&label);
    fs::create_dir(&directory).map_err(text)?;
    let snapshot = directory.join("source");
    let commit = if baseline {
        handoff
            .base
            .as_ref()
            .ok_or("reproducer requires baseline")?
    } else {
        &handoff.candidate
    };
    source::snapshot(authority, commit.native_id.as_str(), &snapshot)?;
    for path in &check.artifacts {
        if fs::symlink_metadata(snapshot.join(path)).is_ok() {
            return Err("required artifact existed before check; stale evidence refused".into());
        }
    }
    process::admit_program(&check.program)?;
    let output = process::execute(
        &check.program.path,
        &check.arguments,
        &snapshot,
        &profile.environment,
        check.timeout_ms,
    )?;
    fs::write(directory.join("stdout"), &output.stdout).map_err(text)?;
    fs::write(directory.join("stderr"), &output.stderr).map_err(text)?;
    let mut status = output.status;
    let mut artifacts = Vec::new();
    for path in &check.artifacts {
        match retained_artifact(&snapshot, path) {
            Ok(bytes) => {
                let artifact = ArtifactObservation {
                    path: path.clone(),
                    hash: digest(&bytes),
                    bytes: bytes.len() as u64,
                };
                let destination = directory.join("artifacts").join(path);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(text)?;
                }
                fs::write(destination, bytes).map_err(text)?;
                artifacts.push(artifact);
            }
            Err(_) if status == "passed" => status = "incomplete".into(),
            Err(_) => {}
        }
    }
    // Commands may not leave source edits or substitute a different subject.
    if !source::clean(authority, &snapshot)? {
        status = "subject_changed".into();
    }
    let evidence_bytes = serde_json::to_vec(&json!({
        "stdout": digest(&output.stdout), "stderr": digest(&output.stderr), "artifacts": artifacts,
    }))
    .map_err(text)?;
    let evidence_hash = digest(&evidence_bytes);
    fs::write(directory.join("evidence.json"), evidence_bytes).map_err(text)?;
    let receipt_hash = digest(
        &serde_json::to_vec(&json!({
            "handoff": handoff, "profile": profile.identity()?, "baseline": baseline,
            "check": check, "status": status, "exit": output.exit_code, "evidence": evidence_hash,
        }))
        .map_err(text)?,
    );
    let observation = CheckObservation {
        check_id: check.id.clone(),
        command: command(check),
        status,
        exit_code: output.exit_code,
        stdout_hash: digest(&output.stdout),
        stderr_hash: digest(&output.stderr),
        artifacts,
        evidence_hash,
        execution_receipt_hash: receipt_hash,
        baseline,
    };
    fs::write(
        directory.join("receipt.json"),
        serde_json::to_vec_pretty(&observation).map_err(text)?,
    )
    .map_err(text)?;
    Ok(observation)
}

fn blocked_observation(check: &VerificationCheck) -> CheckObservation {
    CheckObservation {
        check_id: check.id.clone(),
        command: command(check),
        status: "blocked".into(),
        exit_code: None,
        stdout_hash: digest(&[]),
        stderr_hash: digest(&[]),
        artifacts: vec![],
        evidence_hash: digest(b"blocked"),
        execution_receipt_hash: digest(b"not-executed"),
        baseline: false,
    }
}

pub fn command(check: &VerificationCheck) -> Vec<String> {
    std::iter::once(check.program.path.to_string_lossy().into_owned())
        .chain(check.arguments.iter().cloned())
        .collect()
}

fn retained_artifact(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        path.push(component);
        if fs::symlink_metadata(&path)
            .map_err(text)?
            .file_type()
            .is_symlink()
        {
            return Err("symlink artifact refused".into());
        }
    }
    bounded_file(&path, MAX_ARTIFACT_BYTES).and_then(|bytes| {
        if bytes.is_empty() {
            Err("empty artifact refused".into())
        } else {
            Ok(bytes)
        }
    })
}

fn bounded_file(path: &Path, bound: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(text)?;
    let metadata = file.metadata().map_err(text)?;
    if !metadata.is_file() || metadata.len() > bound {
        return Err("file is not regular or exceeds bound".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(bound + 1)
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() as u64 > bound {
        return Err("file grew beyond bound".into());
    }
    Ok(bytes)
}
fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
fn text(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
fn safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
}
fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".." | ".git"))
}

#[cfg(test)]
mod tests;
