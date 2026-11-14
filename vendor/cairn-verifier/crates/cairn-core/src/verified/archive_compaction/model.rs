use serde::{Deserialize, Serialize};

pub const COMPACTION_PLAN_SCHEMA_V1: &str = "cairn.archive-compaction-plan.v1";
pub const COMPACTION_PACK_SCHEMA_V1: &str = "cairn.archive-pack.v1";
pub const COMPACTION_POINTER_SCHEMA_V1: &str = "cairn.archive-pointer.v1";
pub const COMPACTION_RECEIPT_SCHEMA_V1: &str = "cairn.archive-compaction-receipt.v1";
pub const COMPACTION_LEDGER_SCHEMA_V1: &str = "cairn.archive-compaction-ledger.v1";
pub const COMPACTION_VERIFY_SCHEMA_V1: &str = "cairn.archive-compaction-verification.v1";
pub const COMPACTION_UNPACK_SCHEMA_V1: &str = "cairn.archive-unpack-plan.v1";
pub const LOSSLESS_PACK_PROFILE: &str = "lossless-pack";
pub const LOSSY_SUMMARY_PROFILE: &str = "lossy-summary";
pub const COMPACTION_NON_CLAIM: &str = "local representation integrity only; archive compaction does not prove archived change correctness, release readiness, remote trust, or semantic review completeness";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveFileInput {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveInput {
    pub archive_id: String,
    pub files: Vec<ArchiveFileInput>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CompactionIssue {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchiveManifestEntry {
    pub archive_id: String,
    pub path: String,
    pub role: String,
    pub byte_length: u64,
    pub content_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub archive_id: String,
    pub entries: Vec<ArchiveManifestEntry>,
    pub archive_root: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackObjectDescriptor {
    pub content_hash: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema_version: String,
    pub archives: Vec<ArchiveManifest>,
    pub objects: Vec<PackObjectDescriptor>,
    pub packed_archives_root: String,
    pub pack_root: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchivePointer {
    pub schema_version: String,
    pub archive_id: String,
    pub archive_root: String,
    pub pack_root: String,
    pub pack_manifest_path: String,
    pub lossiness: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MutationAction {
    pub kind: String,
    pub path: String,
    pub content_hash: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactionPlan {
    pub schema_version: String,
    pub profile: String,
    pub lossiness: String,
    pub candidates: Vec<String>,
    pub pack: PackManifest,
    pub retained_pointers: Vec<ArchivePointer>,
    pub planned_pointers: Vec<ArchivePointer>,
    pub blockers: Vec<CompactionIssue>,
    pub input_hash: String,
    pub policy_hash: String,
    pub prev_chain_hash: String,
    pub pre_state_root: String,
    pub post_state_root: String,
    pub packed_archives_root: String,
    pub retained_manifest_root: String,
    pub mutation_manifest: Vec<MutationAction>,
    pub mutation_manifest_hash: String,
    pub plan_hash: String,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactionReceipt {
    pub schema_version: String,
    pub sequence: u64,
    pub profile: String,
    pub lossiness: String,
    pub pack_root: String,
    pub prev_chain_hash: String,
    pub pre_state_root: String,
    pub post_state_root: String,
    pub packed_archives_root: String,
    pub retained_manifest_root: String,
    pub policy_hash: String,
    pub plan_hash: String,
    pub mutation_manifest_hash: String,
    pub receipt_hash: String,
    pub chain_hash: String,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactionLedgerEntry {
    pub sequence: u64,
    pub receipt_path: String,
    pub receipt_hash: String,
    pub chain_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactionLedger {
    pub schema_version: String,
    pub policy_hash: String,
    pub genesis_chain_hash: String,
    pub entries: Vec<CompactionLedgerEntry>,
    pub latest_chain_hash: String,
    pub ledger_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackObjectInput {
    pub content_hash: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnpackFile {
    pub path: String,
    pub byte_length: u64,
    pub content_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnpackPlan {
    pub schema_version: String,
    pub archive_id: String,
    pub archive_root: String,
    pub pack_root: String,
    pub files: Vec<UnpackFile>,
    pub blockers: Vec<CompactionIssue>,
    pub plan_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactionVerificationReceipt {
    pub schema_version: String,
    pub valid: bool,
    pub policy_hash: String,
    pub ledger_hash: String,
    pub latest_chain_hash: String,
    pub verified_pack_roots: Vec<String>,
    pub issues: Vec<CompactionIssue>,
    pub receipt_hash: String,
    pub non_claims: Vec<String>,
}
