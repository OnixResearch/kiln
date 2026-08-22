use alloc::string::{String, ToString};
use serde::{Deserialize, Serialize};

pub const CONTRACT_SCHEMA_VERSION: u16 = 1;

pub const SOURCE_NON_CLAIMS: &[&str] = &[
    "receipt admission does not prove repository trust",
    "receipt admission does not prove global freshness",
    "receipt admission does not prove complete history",
    "receipt admission does not prove peer convergence",
    "receipt admission does not prove remote availability",
    "equal trees do not prove equal history or authority",
    "receipt admission does not prove build correctness",
    "receipt admission does not prove release readiness",
];

const FIXTURE_PROFILE: &str = "radicle-source-v1";
const FIXTURE_RID: &str = "rad:zFixtureRepository";
const FIXTURE_SIGNER: &str = "z6MkFixtureSigner";
const FIXTURE_REFERENCE: &str = "refs/heads/main";
const FIXTURE_SIGREFS: &str = "1111111111111111111111111111111111111111";
const FIXTURE_PARENT: &str = "2222222222222222222222222222222222222222";
const FIXTURE_IDENTITY_ROOT: &str = "3333333333333333333333333333333333333333";
const FIXTURE_REVISION: &str = "4444444444444444444444444444444444444444";
const FIXTURE_TREE: &str = "5555555555555555555555555555555555555555";
const FIXTURE_BLAKE3: &str = "6666666666666666666666666666666666666666666666666666666666666666";
const FIXTURE_MAX_IDENTITY_BYTES: u64 = 256;
const FIXTURE_MAX_REFERENCE_BYTES: u64 = 256;
const FIXTURE_MAX_ENTRIES: u64 = 1024;
const FIXTURE_MAX_PATH_BYTES: u64 = 4096;
const FIXTURE_MAX_FILE_BYTES: u64 = 1_048_576;
const FIXTURE_MAX_TOTAL_BYTES: u64 = 16_777_216;
const FIXTURE_ENTRY_COUNT: u64 = 2;
const FIXTURE_TOTAL_BYTES: u64 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureLevel {
    None,
    Root,
    Parent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    NotVerified,
    Verified,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotLimits {
    pub max_identity_bytes: u64,
    pub max_reference_bytes: u64,
    pub max_entries: u64,
    pub max_path_bytes: u64,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
}

// r[impl kiln.radicle.source_contract.request]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRequest {
    pub schema_version: u16,
    pub profile_id: String,
    pub repository_id: String,
    pub signer_node_id: String,
    pub signed_reference: String,
    pub expected_sigrefs_commit: String,
    pub expected_revision: String,
    pub limits: SnapshotLimits,
}

// r[impl kiln.radicle.source_contract.sigrefs]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedRefsObservation {
    pub schema_version: u16,
    pub profile_id: String,
    pub repository_id: String,
    pub signer_node_id: String,
    pub signed_reference: String,
    pub namespaced_sigrefs_reference: String,
    pub sigrefs_commit_before: String,
    pub sigrefs_commit_after: String,
    pub sigrefs_parent: Option<String>,
    pub identity_root_commit: String,
    pub signed_revision: String,
    pub namespaced_revision: String,
    pub feature_level: FeatureLevel,
    pub signature: Verification,
    pub repository_identity: Verification,
    pub namespace_target: Verification,
}

// r[impl kiln.radicle.source_contract.snapshot]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotEvidence {
    pub schema_version: u16,
    pub revision: String,
    pub native_tree: String,
    pub canonical_tree_blake3: String,
    pub entry_count: u64,
    pub total_bytes: u64,
}

// r[impl kiln.radicle.source_contract.linkage]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReceipt {
    pub schema_version: u16,
    pub request: SourceRequest,
    pub observation: SignedRefsObservation,
    pub snapshot: SnapshotEvidence,
}

#[must_use]
pub fn fixture_request() -> SourceRequest {
    SourceRequest {
        schema_version: CONTRACT_SCHEMA_VERSION,
        profile_id: FIXTURE_PROFILE.to_string(),
        repository_id: FIXTURE_RID.to_string(),
        signer_node_id: FIXTURE_SIGNER.to_string(),
        signed_reference: FIXTURE_REFERENCE.to_string(),
        expected_sigrefs_commit: FIXTURE_SIGREFS.to_string(),
        expected_revision: FIXTURE_REVISION.to_string(),
        limits: SnapshotLimits {
            max_identity_bytes: FIXTURE_MAX_IDENTITY_BYTES,
            max_reference_bytes: FIXTURE_MAX_REFERENCE_BYTES,
            max_entries: FIXTURE_MAX_ENTRIES,
            max_path_bytes: FIXTURE_MAX_PATH_BYTES,
            max_file_bytes: FIXTURE_MAX_FILE_BYTES,
            max_total_bytes: FIXTURE_MAX_TOTAL_BYTES,
        },
    }
}

#[must_use]
pub fn fixture_receipt() -> SourceReceipt {
    let request = fixture_request();
    SourceReceipt {
        schema_version: CONTRACT_SCHEMA_VERSION,
        observation: SignedRefsObservation {
            schema_version: CONTRACT_SCHEMA_VERSION,
            profile_id: request.profile_id.clone(),
            repository_id: request.repository_id.clone(),
            signer_node_id: request.signer_node_id.clone(),
            signed_reference: request.signed_reference.clone(),
            namespaced_sigrefs_reference: alloc::format!(
                "refs/namespaces/{}/refs/rad/sigrefs",
                request.signer_node_id
            ),
            sigrefs_commit_before: request.expected_sigrefs_commit.clone(),
            sigrefs_commit_after: request.expected_sigrefs_commit.clone(),
            sigrefs_parent: Some(FIXTURE_PARENT.to_string()),
            identity_root_commit: FIXTURE_IDENTITY_ROOT.to_string(),
            signed_revision: request.expected_revision.clone(),
            namespaced_revision: request.expected_revision.clone(),
            feature_level: FeatureLevel::Parent,
            signature: Verification::Verified,
            repository_identity: Verification::Verified,
            namespace_target: Verification::Verified,
        },
        snapshot: SnapshotEvidence {
            schema_version: CONTRACT_SCHEMA_VERSION,
            revision: request.expected_revision.clone(),
            native_tree: FIXTURE_TREE.to_string(),
            canonical_tree_blake3: FIXTURE_BLAKE3.to_string(),
            entry_count: FIXTURE_ENTRY_COUNT,
            total_bytes: FIXTURE_TOTAL_BYTES,
        },
        request,
    }
}
