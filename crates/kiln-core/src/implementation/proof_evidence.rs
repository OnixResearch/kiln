//! Backend-neutral proof evidence contracts. These records are proposals until
//! the protected implementer validates them against the frozen profile and
//! actual retained execution observations.

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use super::{CandidateHandoff, ImplementationAttempt, ImplementationWork, acceptance_digest};
use crate::types::{ContentId, CoreResult, Diagnostic};

pub const PROOF_EVIDENCE_SCHEMA: &str = "kiln.proof-evidence.v1";
pub const MAX_PROOF_CLAIMS: usize = 256;
pub const MAX_PROOF_CHECKS: usize = 64;
pub const MAX_PROOF_IDENTITIES: usize = 64;
pub const MAX_PROOF_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_PROOF_LABEL_BYTES: usize = 256;
pub const MAX_PROOF_TEXT_BYTES: usize = 4096;
pub const MAX_PROOF_RAW_OUTPUT_BYTES: usize = 1024 * 1024;
pub const LEAN_INVALID_CONTROL_COUNT: usize = 9;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofBackend {
    Lean,
    Mm0,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofDisposition {
    Passed,
    Failed,
    Skipped,
    NoClaim,
    Approximate,
    Dead,
    TimedOut,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofClaim {
    pub id: String,
    pub theorem: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofCheckRequirement {
    pub id: String,
    pub role: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofArtifact {
    pub path: String,
    pub blake3: String,
    pub bytes: u64,
}

/// Artifact facts retained by the verifier, not deserializable manifest claims.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedProofArtifact {
    pub path: String,
    pub blake3: String,
    pub bytes: u64,
}

/// Raw semantic/process outcome emitted by the frozen proof adapter. It is
/// accepted only when the complete role inventory matches the trusted profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofCheckResult {
    pub id: String,
    pub role: String,
    pub stdout: String,
    pub stderr: String,
    pub disposition: ProofDisposition,
    pub exit_code: Option<i32>,
    pub stdout_blake3: String,
    pub stderr_blake3: String,
}

/// Bounded, versioned proof evidence. Digests in this document do not establish
/// execution; the implementer must compare it with independently approved
/// policy and actual retained artifact/process observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofEvidenceManifest {
    pub schema: String,
    pub backend: ProofBackend,
    pub work_id: String,
    pub work_version: u64,
    pub attempt_id: String,
    pub fence: u64,
    pub repository: String,
    pub candidate: String,
    pub tree: String,
    pub acceptance_digest: ContentId,
    pub profile_digest: ContentId,
    pub module: String,
    pub toolchain: String,
    pub adapter_invocation_blake3: String,
    pub claims: Vec<ProofClaim>,
    pub specification_blake3: String,
    pub semantic_dependencies: Vec<String>,
    pub assumption_policy_blake3: String,
    pub assumptions: Vec<String>,
    pub proof_artifact: ProofArtifact,
    /// Provider-native identities remain explicit, e.g. Lean's SHA-256 pins.
    pub producer_source_revision: String,
    pub producer_profile_sha256: String,
    pub producer_executable_sha256: String,
    pub checker_source_revision: String,
    pub checker_executable_sha256: String,
    pub build_configuration_blake3: String,
    pub producer_runtime_identity_blake3: String,
    pub replay_runtime_identity_blake3: String,
    pub limit_identity_blake3: String,
    pub tools: Vec<ProofToolIdentity>,
    pub checks: Vec<ProofCheckResult>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofToolIdentity {
    pub name: String,
    pub path: String,
    pub source_revision: String,
    /// SHA-256 is preserved for upstream Compiler Lab tool pins.
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofApprovedInput {
    pub id: String,
    pub path: String,
    pub blake3: String,
}

/// Frozen proof policy belongs to the trusted verification profile, never to
/// candidate-authored files. `qualified` is an operator qualification fact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofVerificationPolicy {
    pub backend: ProofBackend,
    pub qualified: bool,
    pub manifest_artifact: String,
    pub adapter_check_id: String,
    pub adapter_invocation_blake3: String,
    pub module: String,
    pub toolchain: String,
    pub proof_artifact: String,
    pub claims: Vec<ProofClaim>,
    pub specification_blake3: String,
    pub semantic_dependencies: Vec<String>,
    pub assumption_policy_blake3: String,
    pub allowed_assumptions: Vec<String>,
    pub required_checks: Vec<ProofCheckRequirement>,
    pub approved_inputs: Vec<ProofApprovedInput>,
    pub producer_source_revision: String,
    pub producer_profile_sha256: String,
    pub producer_executable_sha256: String,
    pub checker_source_revision: String,
    pub checker_executable_sha256: String,
    pub build_configuration_blake3: String,
    pub producer_runtime_identity_blake3: String,
    pub replay_runtime_identity_blake3: String,
    pub limit_identity_blake3: String,
    pub tools: Vec<ProofToolIdentity>,
}

/// Exact verifier-side context, assembled from admitted Kiln records and actual
/// execution, not copied from the submitted manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofExpectedBinding {
    pub work_id: String,
    pub work_version: u64,
    pub attempt_id: String,
    pub fence: u64,
    pub repository: String,
    pub candidate: String,
    pub tree: String,
    pub acceptance_digest: ContentId,
    pub profile_digest: ContentId,
    pub artifact: RetainedProofArtifact,
}

/// Derive the expected outer subject directly from admitted records and actual
/// execution; caller-provided manifest subject fields are never authoritative.
pub fn expected_binding(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    handoff: &CandidateHandoff,
    profile_digest: ContentId,
    artifact: RetainedProofArtifact,
) -> CoreResult<ProofExpectedBinding> {
    Ok(ProofExpectedBinding {
        work_id: work.identity()?.as_str().into(),
        work_version: work.revision,
        attempt_id: attempt.identity.as_str().into(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        candidate: handoff.candidate.native_id.as_str().into(),
        tree: handoff.tree.native_id.as_str().into(),
        acceptance_digest: acceptance_digest(work)?,
        profile_digest,
        artifact,
    })
}

/// Validate the complete proof claim against frozen policy and the actual
/// retained adapter artifact. This is contract validation, not a proof checker.
pub fn validate_manifest(
    policy: &ProofVerificationPolicy,
    expected: &ProofExpectedBinding,
    manifest: &ProofEvidenceManifest,
) -> CoreResult<()> {
    if !policy.qualified {
        return Err(refusal(
            "proof_unsupported",
            "profile.qualified",
            "proof backend has no qualified trusted profile",
        ));
    }
    if policy.backend == ProofBackend::Mm0 {
        return Err(refusal(
            "proof_unsupported",
            "profile.backend",
            "MM0 remains unsupported until a parser-enabled qualified profile and replay fixtures exist",
        ));
    }
    if !valid_blake3(&policy.specification_blake3)
        || !valid_blake3(&policy.assumption_policy_blake3)
        || !valid_blake3(&policy.build_configuration_blake3)
        || !valid_blake3(&policy.producer_runtime_identity_blake3)
        || !valid_blake3(&policy.replay_runtime_identity_blake3)
        || !valid_blake3(&policy.limit_identity_blake3)
        || !valid_sha256(&policy.producer_profile_sha256)
        || !valid_sha256(&policy.producer_executable_sha256)
        || !valid_sha256(&policy.checker_executable_sha256)
        || !valid_blake3(&policy.adapter_invocation_blake3)
        || policy.tools.is_empty()
        || policy.tools.len() > MAX_PROOF_IDENTITIES
        || policy.tools.iter().any(|tool| {
            tool.name.is_empty()
                || tool.name.len() > MAX_PROOF_LABEL_BYTES
                || tool.path.is_empty()
                || tool.path.len() > MAX_PROOF_TEXT_BYTES
                || tool.source_revision.is_empty()
                || tool.source_revision.len() > MAX_PROOF_LABEL_BYTES
                || !valid_sha256(&tool.sha256)
        })
    {
        return Err(refusal(
            "proof_profile_malformed",
            "profile.identities",
            "trusted proof profile contains malformed identities",
        ));
    }
    if policy.module.is_empty()
        || policy.module.len() > MAX_PROOF_TEXT_BYTES
        || policy.toolchain.is_empty()
        || policy.toolchain.len() > MAX_PROOF_LABEL_BYTES
        || policy.module.chars().any(char::is_control)
        || policy.toolchain.chars().any(char::is_control)
    {
        return Err(refusal(
            "proof_profile_malformed",
            "profile.module",
            "module and exact toolchain must be declared",
        ));
    }
    let mut claim_ids = BTreeSet::new();
    if policy.claims.is_empty()
        || policy.claims.len() > MAX_PROOF_CLAIMS
        || policy.claims.iter().any(|claim| {
            claim.id.is_empty() || claim.theorem.is_empty() || !claim_ids.insert(claim.id.as_str())
        })
        || policy
            .semantic_dependencies
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != policy.semantic_dependencies.len()
        || policy
            .allowed_assumptions
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != policy.allowed_assumptions.len()
        || policy.semantic_dependencies.len() > MAX_PROOF_IDENTITIES
        || policy.allowed_assumptions.len() > MAX_PROOF_IDENTITIES
        || !identity_list_is_bounded(&policy.semantic_dependencies)
        || !identity_list_is_bounded(&policy.allowed_assumptions)
        || policy.producer_source_revision.is_empty()
        || policy.checker_source_revision.is_empty()
        || policy
            .producer_source_revision
            .chars()
            .any(char::is_control)
        || policy.checker_source_revision.chars().any(char::is_control)
    {
        return Err(refusal(
            "proof_profile_malformed",
            "profile.inventory",
            "trusted proof profile contains malformed or oversized inventory",
        ));
    }
    let mut input_ids = BTreeSet::new();
    if policy.approved_inputs.len() != policy.semantic_dependencies.len() + 2
        || policy.approved_inputs.iter().any(|input| {
            input.id.is_empty()
                || input.id.len() > MAX_PROOF_LABEL_BYTES
                || input.path.is_empty()
                || input.path.len() > MAX_PROOF_TEXT_BYTES
                || input.id.chars().any(char::is_control)
                || !valid_blake3(&input.blake3)
                || !input_ids.insert(input.id.as_str())
        })
        || !policy
            .approved_inputs
            .iter()
            .any(|input| input.id == "specification" && input.blake3 == policy.specification_blake3)
        || !policy.approved_inputs.iter().any(|input| {
            input.id == "assumption_policy" && input.blake3 == policy.assumption_policy_blake3
        })
        || policy
            .semantic_dependencies
            .iter()
            .any(|id| !policy.approved_inputs.iter().any(|input| &input.id == id))
    {
        return Err(refusal(
            "proof_policy_mismatch",
            "profile.approved_inputs",
            "frozen spec, dependency, or assumption inputs are incomplete or mismatched",
        ));
    }

    if manifest.schema != PROOF_EVIDENCE_SCHEMA
        || manifest.backend != policy.backend
        || manifest.module != policy.module
        || manifest.toolchain != policy.toolchain
        || manifest.adapter_invocation_blake3 != policy.adapter_invocation_blake3
        || manifest.work_id != expected.work_id
        || manifest.work_version != expected.work_version
        || manifest.attempt_id != expected.attempt_id
        || manifest.fence != expected.fence
        || manifest.repository != expected.repository
        || manifest.candidate != expected.candidate
        || manifest.tree != expected.tree
        || manifest.acceptance_digest != expected.acceptance_digest
        || manifest.profile_digest != expected.profile_digest
        || manifest.proof_artifact.path != expected.artifact.path
        || manifest.proof_artifact.blake3 != expected.artifact.blake3
        || manifest.proof_artifact.bytes != expected.artifact.bytes
        || manifest.proof_artifact.bytes > MAX_PROOF_ARTIFACT_BYTES
    {
        return Err(refusal(
            "proof_binding_mismatch",
            "manifest.subject",
            "manifest does not match exact admitted subject and retained artifact",
        ));
    }
    if manifest
        .claims
        .iter()
        .any(|claim| claim.id.is_empty() || claim.theorem.is_empty())
        || manifest.assumptions.iter().collect::<BTreeSet<_>>().len() != manifest.assumptions.len()
        || !identity_list_is_bounded(&manifest.semantic_dependencies)
        || !identity_list_is_bounded(&manifest.assumptions)
    {
        return Err(refusal(
            "proof_manifest_malformed",
            "manifest",
            "proof inventory contains malformed or duplicate entries",
        ));
    }
    let mut claims = manifest.claims.clone();
    let mut expected_claims = policy.claims.clone();
    claims.sort_by(|a, b| (&a.id, &a.theorem).cmp(&(&b.id, &b.theorem)));
    expected_claims.sort_by(|a, b| (&a.id, &a.theorem).cmp(&(&b.id, &b.theorem)));
    if claims != expected_claims || claims.is_empty() || claims.len() > MAX_PROOF_CLAIMS {
        return Err(refusal(
            "proof_claim_mismatch",
            "manifest.claims",
            "required theorem inventory differs from trusted policy",
        ));
    }
    if manifest.specification_blake3 != policy.specification_blake3
        || manifest.semantic_dependencies != policy.semantic_dependencies
        || manifest.assumption_policy_blake3 != policy.assumption_policy_blake3
        || manifest
            .assumptions
            .iter()
            .any(|item| !policy.allowed_assumptions.contains(item))
    {
        return Err(refusal(
            "proof_policy_mismatch",
            "manifest.assumptions",
            "specification, dependencies, or assumptions differ from trusted policy",
        ));
    }
    if manifest.producer_source_revision != policy.producer_source_revision
        || manifest.producer_profile_sha256 != policy.producer_profile_sha256
        || manifest.producer_executable_sha256 != policy.producer_executable_sha256
        || manifest.checker_source_revision != policy.checker_source_revision
        || manifest.checker_executable_sha256 != policy.checker_executable_sha256
        || manifest.build_configuration_blake3 != policy.build_configuration_blake3
        || manifest.producer_runtime_identity_blake3 != policy.producer_runtime_identity_blake3
        || manifest.replay_runtime_identity_blake3 != policy.replay_runtime_identity_blake3
        || manifest.limit_identity_blake3 != policy.limit_identity_blake3
        || manifest.tools != policy.tools
    {
        return Err(refusal(
            "proof_tool_mismatch",
            "manifest.tool_identity",
            "producer, checker, build, runtime, or limits differ from trusted profile",
        ));
    }
    if manifest.checks.len() != policy.required_checks.len()
        || manifest.checks.len() > MAX_PROOF_CHECKS
    {
        return Err(refusal(
            "proof_check_inventory",
            "manifest.checks",
            "required replay/control inventory is incomplete or has extras",
        ));
    }
    let mut required_ids = BTreeSet::new();
    let mut roles = BTreeSet::new();
    for required in &policy.required_checks {
        if required.id.is_empty()
            || required.role.is_empty()
            || !required_ids.insert(required.id.as_str())
        {
            return Err(refusal(
                "proof_check_inventory",
                "profile.required_checks",
                "trusted replay/control inventory is malformed",
            ));
        }
        roles.insert(required.role.as_str());
    }
    if policy.backend == ProofBackend::Lean
        && (![
            "official_replay",
            "producer_replay",
            "nanoda",
            "invalid_control",
        ]
        .iter()
        .all(|role| roles.contains(role))
            || policy
                .required_checks
                .iter()
                .filter(|check| check.role == "invalid_control")
                .count()
                != LEAN_INVALID_CONTROL_COUNT
            || policy.allowed_assumptions.len() != 3
            || !["propext", "Classical.choice", "Quot.sound"]
                .iter()
                .all(|name| {
                    policy
                        .allowed_assumptions
                        .iter()
                        .any(|allowed| allowed == name)
                }))
    {
        return Err(refusal(
            "proof_check_inventory",
            "profile.required_checks",
            "Lean requires official replay, fresh producer replay, Nanoda, all nine frozen invalid controls, and the pinned axiom allowlist",
        ));
    }
    let mut seen = BTreeSet::new();
    for required in &policy.required_checks {
        let Some(actual) = manifest
            .checks
            .iter()
            .find(|row| row.id == required.id && row.role == required.role)
        else {
            return Err(refusal(
                "proof_check_missing",
                "manifest.checks",
                "required replay or control result is omitted",
            ));
        };
        if !seen.insert(actual.id.as_str())
            || actual.disposition != ProofDisposition::Passed
            || actual.exit_code != Some(0)
            || actual.stdout.len() > MAX_PROOF_RAW_OUTPUT_BYTES
            || actual.stderr.len() > MAX_PROOF_RAW_OUTPUT_BYTES
            || blake3::hash(actual.stdout.as_bytes()).to_hex().to_string() != actual.stdout_blake3
            || blake3::hash(actual.stderr.as_bytes()).to_hex().to_string() != actual.stderr_blake3
        {
            return Err(refusal(
                "proof_check_nonpassing",
                "manifest.checks",
                "required proof check did not pass with bound raw outputs",
            ));
        }
    }
    Ok(())
}

fn refusal(code: &'static str, field: &'static str, message: &'static str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

/// Small shared helpers used by adapters while retaining Kiln BLAKE3 identity.
pub fn valid_blake3(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
pub fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn identity_list_is_bounded(values: &[String]) -> bool {
    values.len() <= MAX_PROOF_IDENTITIES
        && values.iter().all(|value| {
            !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn cid(byte: char) -> ContentId {
        ContentId::from_blake3_hex(byte.to_string().repeat(64))
            .unwrap_or_else(|error| unreachable!("fixture identity: {error:?}"))
    }

    fn check_mut(manifest: &mut ProofEvidenceManifest, index: usize) -> &mut ProofCheckResult {
        manifest
            .checks
            .get_mut(index)
            .unwrap_or_else(|| unreachable!("fixture check {index}"))
    }

    fn h(byte: char) -> String {
        byte.to_string().repeat(64)
    }

    fn fixture() -> (
        ProofVerificationPolicy,
        ProofExpectedBinding,
        ProofEvidenceManifest,
    ) {
        let required_checks = [
            ("official", "official_replay"),
            ("producer", "producer_replay"),
            ("nanoda", "nanoda"),
            ("control-1", "invalid_control"),
            ("control-2", "invalid_control"),
            ("control-3", "invalid_control"),
            ("control-4", "invalid_control"),
            ("control-5", "invalid_control"),
            ("control-6", "invalid_control"),
            ("control-7", "invalid_control"),
            ("control-8", "invalid_control"),
            ("control-9", "invalid_control"),
        ]
        .into_iter()
        .map(|(id, role)| ProofCheckRequirement {
            id: id.into(),
            role: role.into(),
        })
        .collect::<Vec<_>>();
        let tools = vec![ProofToolIdentity {
            name: "lean".into(),
            path: "/nix/store/lean/bin/lean".into(),
            source_revision: "rev-a".into(),
            sha256: h('a'),
        }];
        let policy = ProofVerificationPolicy {
            backend: ProofBackend::Lean,
            qualified: true,
            manifest_artifact: "proof/manifest.json".into(),
            adapter_check_id: "lean-proof".into(),
            module: "Example".into(),
            toolchain: "leanprover/lean4:v4.20.0".into(),
            proof_artifact: "proof/module.olean".into(),
            adapter_invocation_blake3: h('7'),
            claims: vec![ProofClaim {
                id: "claim-1".into(),
                theorem: "Example.sound".into(),
            }],
            specification_blake3: h('b'),
            semantic_dependencies: vec!["dep-sha256-a".into()],
            assumption_policy_blake3: h('c'),
            allowed_assumptions: vec![
                "propext".into(),
                "Classical.choice".into(),
                "Quot.sound".into(),
            ],
            required_checks,
            approved_inputs: vec![
                ProofApprovedInput {
                    id: "specification".into(),
                    path: "proof/spec.lean".into(),
                    blake3: h('b'),
                },
                ProofApprovedInput {
                    id: "assumption_policy".into(),
                    path: "proof/axioms.json".into(),
                    blake3: h('c'),
                },
                ProofApprovedInput {
                    id: "dep-sha256-a".into(),
                    path: "proof/dep.lean".into(),
                    blake3: h('d'),
                },
            ],
            producer_source_revision: "producer-rev".into(),
            producer_profile_sha256: h('a'),
            producer_executable_sha256: h('a'),
            checker_source_revision: "checker-rev".into(),
            checker_executable_sha256: h('a'),
            build_configuration_blake3: h('d'),
            producer_runtime_identity_blake3: h('e'),
            replay_runtime_identity_blake3: h('f'),
            limit_identity_blake3: h('1'),
            tools: tools.clone(),
        };
        let expected = ProofExpectedBinding {
            work_id: "work-identity".into(),
            work_version: 7,
            attempt_id: "attempt".into(),
            fence: 9,
            repository: "repo".into(),
            candidate: "commit".into(),
            tree: "tree".into(),
            acceptance_digest: cid('2'),
            profile_digest: cid('3'),
            artifact: RetainedProofArtifact {
                path: "proof/module.olean".into(),
                blake3: h('4'),
                bytes: 32,
            },
        };
        let checks = policy
            .required_checks
            .iter()
            .map(|check| ProofCheckResult {
                id: check.id.clone(),
                role: check.role.clone(),
                disposition: ProofDisposition::Passed,
                exit_code: Some(0),
                stdout: "fixture output".into(),
                stderr: String::new(),
                stdout_blake3: blake3::hash(b"fixture output").to_hex().to_string(),
                stderr_blake3: blake3::hash(b"").to_hex().to_string(),
            })
            .collect();
        let manifest = ProofEvidenceManifest {
            schema: PROOF_EVIDENCE_SCHEMA.into(),
            backend: ProofBackend::Lean,
            module: policy.module.clone(),
            toolchain: policy.toolchain.clone(),
            adapter_invocation_blake3: policy.adapter_invocation_blake3.clone(),
            work_id: expected.work_id.clone(),
            work_version: expected.work_version,
            attempt_id: expected.attempt_id.clone(),
            fence: expected.fence,
            repository: expected.repository.clone(),
            candidate: expected.candidate.clone(),
            tree: expected.tree.clone(),
            acceptance_digest: expected.acceptance_digest.clone(),
            profile_digest: expected.profile_digest.clone(),
            claims: policy.claims.clone(),
            specification_blake3: policy.specification_blake3.clone(),
            semantic_dependencies: policy.semantic_dependencies.clone(),
            assumption_policy_blake3: policy.assumption_policy_blake3.clone(),
            assumptions: vec!["propext".into()],
            proof_artifact: ProofArtifact {
                path: expected.artifact.path.clone(),
                blake3: expected.artifact.blake3.clone(),
                bytes: expected.artifact.bytes,
            },
            producer_source_revision: policy.producer_source_revision.clone(),
            producer_profile_sha256: policy.producer_profile_sha256.clone(),
            producer_executable_sha256: policy.producer_executable_sha256.clone(),
            checker_source_revision: policy.checker_source_revision.clone(),
            checker_executable_sha256: policy.checker_executable_sha256.clone(),
            build_configuration_blake3: policy.build_configuration_blake3.clone(),
            producer_runtime_identity_blake3: policy.producer_runtime_identity_blake3.clone(),
            replay_runtime_identity_blake3: policy.replay_runtime_identity_blake3.clone(),
            limit_identity_blake3: policy.limit_identity_blake3.clone(),
            tools,
            checks,
        };
        (policy, expected, manifest)
    }

    #[test]
    fn accepts_only_exact_frozen_lean_evidence_contract() {
        let (policy, expected, manifest) = fixture();
        assert!(validate_manifest(&policy, &expected, &manifest).is_ok());
    }

    #[test]
    fn only_assumptions_declared_by_frozen_policy_are_accepted() {
        let (policy, expected, mut manifest) = fixture();
        manifest.assumptions = policy.allowed_assumptions.clone();
        assert!(validate_manifest(&policy, &expected, &manifest).is_ok());
        manifest.assumptions.push("unapproved_axiom".into());
        assert!(validate_manifest(&policy, &expected, &manifest).is_err());
    }

    #[test]
    fn rejects_subject_profile_policy_claim_tool_and_control_substitution() {
        let (policy, expected, mut manifest) = fixture();
        manifest.fence += 1;
        assert!(validate_manifest(&policy, &expected, &manifest).is_err());
        let (policy, expected, mut manifest) = fixture();
        manifest.profile_digest = cid('8');
        assert!(validate_manifest(&policy, &expected, &manifest).is_err());
        let (policy, expected, mut manifest) = fixture();
        manifest.claims.clear();
        assert!(validate_manifest(&policy, &expected, &manifest).is_err());
        let (policy, expected, mut manifest) = fixture();
        manifest
            .tools
            .first_mut()
            .unwrap_or_else(|| unreachable!("fixture tool"))
            .sha256 = h('9');
        assert!(validate_manifest(&policy, &expected, &manifest).is_err());
        let (policy, expected, mut manifest) = fixture();
        manifest.checks.pop();
        assert!(validate_manifest(&policy, &expected, &manifest).is_err());
        let (policy, expected, mut manifest) = fixture();
        check_mut(&mut manifest, 3).exit_code = Some(0);
        check_mut(&mut manifest, 3).disposition = ProofDisposition::Failed;
        assert!(validate_manifest(&policy, &expected, &manifest).is_err());
        for mutate in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] {
            let (policy, expected, mut manifest) = fixture();
            match mutate {
                0 => manifest.work_version += 1,
                1 => manifest.attempt_id = "stale-attempt".into(),
                2 => manifest.repository = "other-repo".into(),
                3 => manifest.candidate = "other-candidate".into(),
                4 => manifest.tree = "other-tree".into(),
                5 => manifest.proof_artifact.blake3 = h('8'),
                6 => {
                    check_mut(&mut manifest, 0).disposition = ProofDisposition::TimedOut;
                    check_mut(&mut manifest, 0).exit_code = Some(0);
                }
                7 => {
                    check_mut(&mut manifest, 1).disposition = ProofDisposition::Cancelled;
                    check_mut(&mut manifest, 1).exit_code = Some(0);
                }
                8 => manifest.module = "WrongModule".into(),
                9 => manifest.toolchain = "leanprover/lean4:stale".into(),
                10 => manifest.work_id = "other-work".into(),
                _ => manifest.acceptance_digest = cid('8'),
            }
            assert!(validate_manifest(&policy, &expected, &manifest).is_err());
        }
    }

    #[test]
    fn mm0_is_unsupported_even_with_a_well_formed_manifest() {
        let (mut policy, expected, mut manifest) = fixture();
        policy.backend = ProofBackend::Mm0;
        manifest.backend = ProofBackend::Mm0;
        assert!(validate_manifest(&policy, &expected, &manifest).is_err());
    }
}
