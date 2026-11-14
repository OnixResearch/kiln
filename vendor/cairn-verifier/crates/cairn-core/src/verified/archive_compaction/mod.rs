// r[impl cairn.nominal_evidence_identities.lifecycle]
// r[verify cairn.nominal_evidence_identities.lifecycle.validation]

mod chain;
mod model;
mod plan;
mod verify;

pub use chain::{
    append_compaction_receipt, build_compaction_receipt, compaction_ledger_hash,
    compaction_receipt_hash, empty_compaction_ledger, validate_compaction_ledger,
    validate_compaction_receipt,
};
pub use model::{
    ArchiveFileInput, ArchiveInput, ArchiveManifest, ArchiveManifestEntry, ArchivePointer,
    CompactionIssue, CompactionLedger, CompactionLedgerEntry, CompactionPlan, CompactionReceipt,
    CompactionVerificationReceipt, MutationAction, PackManifest, PackObjectDescriptor,
    PackObjectInput, UnpackFile, UnpackPlan, COMPACTION_LEDGER_SCHEMA_V1, COMPACTION_NON_CLAIM,
    COMPACTION_PACK_SCHEMA_V1, COMPACTION_PLAN_SCHEMA_V1, COMPACTION_POINTER_SCHEMA_V1,
    COMPACTION_RECEIPT_SCHEMA_V1, COMPACTION_UNPACK_SCHEMA_V1, COMPACTION_VERIFY_SCHEMA_V1,
    LOSSLESS_PACK_PROFILE, LOSSY_SUMMARY_PROFILE,
};
pub use plan::{
    build_archive_manifest, build_compaction_plan, compaction_chain_hash, compaction_genesis_hash,
    compaction_plan_hash, ledger_path, mutation_manifest_path, object_path, pack_manifest_path,
    pointer_path, prepared_pointer_path, receipt_path, safe_archive_id, safe_relative_path,
    validate_compaction_plan, COMPACTION_ROOT, HASH_HEX_LENGTH,
};
pub use verify::{
    build_unpack_plan, build_verification_receipt, unpack_plan_hash, validate_pack_manifest,
    verification_receipt_hash, verify_pack_objects,
};

#[cfg(test)]
mod tests;
