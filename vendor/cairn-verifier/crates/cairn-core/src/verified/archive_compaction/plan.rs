use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::json;

use crate::verified::evidence::{blake3_hex, canonical_json_hash};
use crate::verified::nominal_identity::{
    ArchiveDigestRole, ArchiveId, ArtifactDigestRole, Blake3Digest, ChainDigestRole,
    DigestRoleMarker, InputDigestRole, LedgerDigestRole, PackDigestRole, PlanDigestRole,
    PolicyDigestRole, ReceiptDigestRole, StateDigestRole, RAW_BLAKE3_PROFILE,
};

use super::model::{
    ArchiveInput, ArchiveManifest, ArchiveManifestEntry, ArchivePointer, CompactionIssue,
    CompactionPlan, MutationAction, PackManifest, PackObjectDescriptor, COMPACTION_NON_CLAIM,
    COMPACTION_PACK_SCHEMA_V1, COMPACTION_PLAN_SCHEMA_V1, COMPACTION_POINTER_SCHEMA_V1,
    LOSSLESS_PACK_PROFILE,
};

pub const COMPACTION_ROOT: &str = ".cairn/compaction/v1";
pub const HASH_HEX_LENGTH: usize = 64;
const LOSSINESS_LOSSLESS: &str = "lossless";
const GENESIS_DOMAIN: &[u8] = b"cairn.archive-compaction.genesis.v1\0";
const CHAIN_DOMAIN: &[u8] = b"cairn.archive-compaction.chain.v1\0";

pub fn compaction_genesis_hash(policy_hash: &str) -> String {
    let mut material = Vec::with_capacity(GENESIS_DOMAIN.len() + policy_hash.len());
    material.extend_from_slice(GENESIS_DOMAIN);
    material.extend_from_slice(policy_hash.as_bytes());
    blake3_hex(&material)
}

pub fn compaction_chain_hash(prev_chain_hash: &str, receipt_hash: &str) -> String {
    let separator = [0_u8];
    let capacity =
        CHAIN_DOMAIN.len() + prev_chain_hash.len() + separator.len() + receipt_hash.len();
    let mut material = Vec::with_capacity(capacity);
    material.extend_from_slice(CHAIN_DOMAIN);
    material.extend_from_slice(prev_chain_hash.as_bytes());
    material.extend_from_slice(&separator);
    material.extend_from_slice(receipt_hash.as_bytes());
    blake3_hex(&material)
}

pub fn build_archive_manifest(
    archive: &ArchiveInput,
) -> Result<ArchiveManifest, Vec<CompactionIssue>> {
    // r[impl cairn.sync_archive.compaction.manifest_roots.entries]
    // r[impl cairn.sync_archive.compaction.manifest_roots.stable]
    let mut issues = Vec::new();
    if !safe_archive_id(&archive.archive_id) {
        issues.push(issue(
            "archive_compaction.archive_id_unsafe",
            &archive.archive_id,
            "archive identity must be one safe path component",
        ));
    }
    let mut entries = Vec::with_capacity(archive.files.len());
    let mut seen_paths = BTreeSet::new();
    for file in &archive.files {
        if !safe_relative_path(&file.path) {
            issues.push(issue(
                "archive_compaction.path_unsafe",
                &file.path,
                "archive file path must be normalized and relative",
            ));
            continue;
        }
        if !seen_paths.insert(file.path.clone()) {
            issues.push(issue(
                "archive_compaction.path_duplicate",
                &file.path,
                "archive manifest paths must be unique",
            ));
            continue;
        }
        let byte_length = match u64::try_from(file.bytes.len()) {
            Ok(length) => length,
            Err(_) => {
                issues.push(issue(
                    "archive_compaction.file_too_large",
                    &file.path,
                    "archive file byte length must fit in u64",
                ));
                continue;
            }
        };
        entries.push(ArchiveManifestEntry {
            archive_id: archive.archive_id.clone(),
            path: file.path.clone(),
            role: archive_file_role(&file.path).to_string(),
            byte_length,
            content_hash: blake3_hex(&file.bytes),
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    issues.sort();
    issues.dedup();
    if !issues.is_empty() {
        return Err(issues);
    }
    let archive_root = canonical_json_hash(&json!({
        "archive_id": archive.archive_id,
        "entries": entries,
    }));
    Ok(ArchiveManifest {
        archive_id: archive.archive_id.clone(),
        entries,
        archive_root,
    })
}

pub fn build_compaction_plan(
    archives: &[ArchiveInput],
    retained_pointers: &[ArchivePointer],
    policy_hash: &str,
    profile: &str,
    prev_chain_hash: &str,
    observed_issues: &[CompactionIssue],
) -> CompactionPlan {
    // r[impl cairn.sync_archive.compaction.plan]
    // r[impl cairn.sync_archive.compaction.plan.ordering]
    // r[impl cairn.sync_archive.compaction.manifest_roots]
    // r[impl cairn.sync_archive.compaction.lossiness_policy]
    // r[impl cairn.sync_archive.compaction.lossiness_policy.default_reject]
    // r[impl cairn.sync_archive.compaction.lossiness_policy.non_claims]
    // r[impl cairn.sync_archive.compaction.boundary]
    // r[impl cairn.sync_archive.compaction.boundary.pure_core]
    let mut blockers = observed_issues.to_vec();
    validate_hash(policy_hash, "policy_hash", &mut blockers);
    validate_hash(prev_chain_hash, "prev_chain_hash", &mut blockers);
    if profile != LOSSLESS_PACK_PROFILE {
        blockers.push(issue(
            "archive_compaction.profile_unsupported",
            "profile",
            "only the reviewed lossless-pack profile is executable",
        ));
    }

    let mut pointers = retained_pointers.to_vec();
    pointers.sort_by(|left, right| left.archive_id.cmp(&right.archive_id));
    validate_pointers(&pointers, &mut blockers);
    let pointer_by_archive = pointers
        .iter()
        .map(|pointer| (pointer.archive_id.as_str(), pointer))
        .collect::<BTreeMap<_, _>>();

    let mut all_manifests = Vec::new();
    let mut candidate_manifests = Vec::new();
    let mut restored_manifests = Vec::new();
    let mut seen_archives = BTreeSet::new();
    for archive in archives {
        if !seen_archives.insert(archive.archive_id.clone()) {
            blockers.push(issue(
                "archive_compaction.archive_duplicate",
                &archive.archive_id,
                "expanded archive identities must be unique",
            ));
            continue;
        }
        match build_archive_manifest(archive) {
            Ok(manifest) => {
                if let Some(pointer) = pointer_by_archive.get(manifest.archive_id.as_str()) {
                    if !same_digest::<ArchiveDigestRole>(
                        &pointer.archive_root,
                        &manifest.archive_root,
                    ) {
                        blockers.push(issue(
                            "archive_compaction.restored_archive_mismatch",
                            &manifest.archive_id,
                            "restored archive bytes must match the retained pointer root",
                        ));
                    }
                    restored_manifests.push(manifest.clone());
                } else {
                    candidate_manifests.push(manifest.clone());
                }
                all_manifests.push(manifest);
            }
            Err(mut manifest_issues) => blockers.append(&mut manifest_issues),
        }
    }
    all_manifests.sort_by(|left, right| left.archive_id.cmp(&right.archive_id));
    candidate_manifests.sort_by(|left, right| left.archive_id.cmp(&right.archive_id));
    restored_manifests.sort_by(|left, right| left.archive_id.cmp(&right.archive_id));
    if candidate_manifests.is_empty() {
        blockers.push(issue(
            "archive_compaction.no_candidates",
            ".cairn/archive",
            "at least one expanded archive without a retained pointer is required",
        ));
    }

    let pack = build_pack_manifest(candidate_manifests);
    let pack_manifest_path = pack_manifest_path(&pack.pack_root);
    let mut planned_pointers = pack
        .archives
        .iter()
        .map(|archive| ArchivePointer {
            schema_version: COMPACTION_POINTER_SCHEMA_V1.to_string(),
            archive_id: archive.archive_id.clone(),
            archive_root: archive.archive_root.clone(),
            pack_root: pack.pack_root.clone(),
            pack_manifest_path: pack_manifest_path.clone(),
            lossiness: LOSSINESS_LOSSLESS.to_string(),
        })
        .collect::<Vec<_>>();
    planned_pointers.sort_by(|left, right| left.archive_id.cmp(&right.archive_id));

    let mut post_pointers = pointers.clone();
    post_pointers.extend(planned_pointers.clone());
    post_pointers.sort_by(|left, right| left.archive_id.cmp(&right.archive_id));
    validate_pointers(&post_pointers, &mut blockers);

    let input_hash = canonical_json_hash(&json!({
        "expanded_archives": all_manifests,
        "retained_pointers": pointers,
        "policy_hash": policy_hash,
        "prev_chain_hash": prev_chain_hash,
        "profile": profile,
    }));
    let pre_state_root = canonical_json_hash(&json!({
        "expanded_archives": all_manifests,
        "retained_pointers": pointers,
    }));
    let post_state_root = canonical_json_hash(&json!({
        "expanded_archives": restored_manifests,
        "retained_pointers": post_pointers,
    }));
    let retained_manifest_root = canonical_json_hash(&json!({"pointers": post_pointers}));
    let mutation_manifest = build_mutation_manifest(&pack, &planned_pointers);
    let mutation_manifest_hash = hash_serializable(&mutation_manifest);
    blockers.sort();
    blockers.dedup();
    let candidates = pack
        .archives
        .iter()
        .map(|archive| archive.archive_id.clone())
        .collect();
    let mut plan = CompactionPlan {
        schema_version: COMPACTION_PLAN_SCHEMA_V1.to_string(),
        profile: profile.to_string(),
        lossiness: LOSSINESS_LOSSLESS.to_string(),
        candidates,
        packed_archives_root: pack.packed_archives_root.clone(),
        pack,
        retained_pointers: pointers,
        planned_pointers,
        blockers,
        input_hash,
        policy_hash: policy_hash.to_string(),
        prev_chain_hash: prev_chain_hash.to_string(),
        pre_state_root,
        post_state_root,
        retained_manifest_root,
        mutation_manifest,
        mutation_manifest_hash,
        plan_hash: String::new(),
        non_claims: vec![COMPACTION_NON_CLAIM.to_string()],
    };
    plan.plan_hash = compaction_plan_hash(&plan);
    plan
}

pub fn compaction_plan_hash(plan: &CompactionPlan) -> String {
    let mut material = plan.clone();
    material.plan_hash.clear();
    hash_serializable(&material)
}

pub fn validate_compaction_plan(plan: &CompactionPlan) -> Vec<CompactionIssue> {
    let mut issues = Vec::new();
    if plan.schema_version != COMPACTION_PLAN_SCHEMA_V1 {
        issues.push(issue(
            "archive_compaction.plan_schema_unsupported",
            "schema_version",
            "plan schema must be cairn.archive-compaction-plan.v1",
        ));
    }
    if plan.profile != LOSSLESS_PACK_PROFILE || plan.lossiness != LOSSINESS_LOSSLESS {
        issues.push(issue(
            "archive_compaction.profile_unsupported",
            "profile",
            "only lossless-pack with lossiness=lossless is executable",
        ));
    }
    if !plan
        .non_claims
        .iter()
        .any(|non_claim| non_claim == COMPACTION_NON_CLAIM)
    {
        issues.push(issue(
            "archive_compaction.non_claim_missing",
            "non_claims",
            "compaction plan must preserve the local-integrity-only non-claim",
        ));
    }
    validate_hash(&plan.policy_hash, "policy_hash", &mut issues);
    validate_hash(&plan.prev_chain_hash, "prev_chain_hash", &mut issues);
    validate_hash(&plan.input_hash, "input_hash", &mut issues);
    validate_hash(&plan.pre_state_root, "pre_state_root", &mut issues);
    validate_hash(&plan.post_state_root, "post_state_root", &mut issues);
    validate_hash(
        &plan.packed_archives_root,
        "packed_archives_root",
        &mut issues,
    );
    validate_hash(
        &plan.retained_manifest_root,
        "retained_manifest_root",
        &mut issues,
    );
    validate_hash(
        &plan.mutation_manifest_hash,
        "mutation_manifest_hash",
        &mut issues,
    );
    validate_hash(&plan.plan_hash, "plan_hash", &mut issues);
    issues.extend(super::verify::validate_pack_manifest(&plan.pack));
    if plan.pack.pack_root.is_empty()
        || !same_digest::<PackDigestRole>(&plan.pack.pack_root, &pack_manifest_hash(&plan.pack))
    {
        issues.push(issue(
            "archive_compaction.pack_root_mismatch",
            "pack.pack_root",
            "pack root must match canonical pack material",
        ));
    }
    let expected_candidates = plan
        .pack
        .archives
        .iter()
        .map(|archive| archive.archive_id.clone())
        .collect::<Vec<_>>();
    if plan.candidates != expected_candidates {
        issues.push(issue(
            "archive_compaction.candidates_mismatch",
            "candidates",
            "plan candidates must exactly match the ordered pack archives",
        ));
    }
    if !same_digest::<ArchiveDigestRole>(
        &plan.pack.packed_archives_root,
        &plan.packed_archives_root,
    ) {
        issues.push(issue(
            "archive_compaction.packed_archives_root_mismatch",
            "packed_archives_root",
            "plan and pack archive roots must match",
        ));
    }
    if !same_digest::<PlanDigestRole>(
        &hash_serializable(&plan.mutation_manifest),
        &plan.mutation_manifest_hash,
    ) {
        issues.push(issue(
            "archive_compaction.mutation_manifest_hash_mismatch",
            "mutation_manifest_hash",
            "mutation manifest hash must match canonical actions",
        ));
    }
    if !same_digest::<PlanDigestRole>(&compaction_plan_hash(plan), &plan.plan_hash) {
        issues.push(issue(
            "archive_compaction.plan_hash_mismatch",
            "plan_hash",
            "plan hash must match canonical plan material",
        ));
    }
    validate_pointers(&plan.retained_pointers, &mut issues);
    validate_pointers(&plan.planned_pointers, &mut issues);
    if plan.planned_pointers.len() != plan.pack.archives.len()
        || plan
            .planned_pointers
            .iter()
            .zip(&plan.pack.archives)
            .any(|(pointer, archive)| {
                pointer.archive_id != archive.archive_id
                    || !same_digest::<ArchiveDigestRole>(
                        &pointer.archive_root,
                        &archive.archive_root,
                    )
                    || !same_digest::<PackDigestRole>(&pointer.pack_root, &plan.pack.pack_root)
                    || pointer.pack_manifest_path != pack_manifest_path(&plan.pack.pack_root)
            })
    {
        issues.push(issue(
            "archive_compaction.planned_pointer_mismatch",
            "planned_pointers",
            "planned pointers must exactly bind each ordered pack archive",
        ));
    }
    let mut expected_pointers = plan.retained_pointers.clone();
    expected_pointers.extend(plan.planned_pointers.clone());
    expected_pointers.sort_by(|left, right| left.archive_id.cmp(&right.archive_id));
    validate_pointers(&expected_pointers, &mut issues);
    if !same_digest::<ArchiveDigestRole>(
        &canonical_json_hash(&json!({"pointers": expected_pointers})),
        &plan.retained_manifest_root,
    ) {
        issues.push(issue(
            "archive_compaction.retained_manifest_root_mismatch",
            "retained_manifest_root",
            "retained manifest root must bind all post-compaction pointers",
        ));
    }
    for action in &plan.mutation_manifest {
        if !safe_relative_path(&action.path) {
            issues.push(issue(
                "archive_compaction.mutation_path_unsafe",
                &action.path,
                "mutation paths must be normalized and repository-relative",
            ));
        }
    }
    issues.extend(plan.blockers.clone());
    issues.sort();
    issues.dedup();
    issues
}

pub fn pack_manifest_path(pack_root: &str) -> String {
    format!("{COMPACTION_ROOT}/packs/{pack_root}/manifest.json")
}

pub fn object_path(content_hash: &str) -> String {
    format!("{COMPACTION_ROOT}/objects/blake3/{content_hash}")
}

pub fn pointer_path(archive_id: &str) -> String {
    format!("{COMPACTION_ROOT}/pointers/{archive_id}.json")
}

pub fn prepared_pointer_path(pack_root: &str, archive_id: &str) -> String {
    format!("{COMPACTION_ROOT}/prepared/{pack_root}/{archive_id}.json")
}

pub fn mutation_manifest_path(pack_root: &str) -> String {
    format!("{COMPACTION_ROOT}/mutations/{pack_root}.json")
}

pub fn receipt_path(pack_root: &str) -> String {
    format!("{COMPACTION_ROOT}/receipts/{pack_root}.json")
}

pub fn ledger_path() -> String {
    format!("{COMPACTION_ROOT}/ledger.json")
}

pub fn safe_archive_id(value: &str) -> bool {
    ArchiveId::parse(value, "archive_compaction.archive_id").is_ok()
        && !value.contains('/')
        && !value.starts_with('.')
}

pub fn safe_relative_path(value: &str) -> bool {
    if value.is_empty() || value.starts_with('/') || value.contains('\\') {
        return false;
    }
    value.split('/').all(safe_path_component)
}

fn safe_path_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(crate) fn issue(code: &str, path: &str, message: &str) -> CompactionIssue {
    CompactionIssue {
        code: code.to_string(),
        path: path.to_string(),
        message: message.to_string(),
    }
}

pub(crate) fn hash_serializable<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(json) => canonical_json_hash(&json),
        Err(error) => blake3_hex(format!("serialization_error:{error}").as_bytes()),
    }
}

pub(crate) fn validate_hash(value: &str, path: &str, issues: &mut Vec<CompactionIssue>) {
    let valid = if path == "policy_hash" {
        is_digest::<PolicyDigestRole>(value)
    } else if path == "input_hash" {
        is_digest::<InputDigestRole>(value)
    } else if matches!(path, "plan_hash" | "mutation_manifest_hash") {
        is_digest::<PlanDigestRole>(value)
    } else if path == "receipt_hash" || path.ends_with(".receipt_hash") {
        is_digest::<ReceiptDigestRole>(value)
    } else if matches!(
        path,
        "prev_chain_hash" | "chain_hash" | "genesis_chain_hash" | "latest_chain_hash"
    ) || path.ends_with(".chain_hash")
    {
        is_digest::<ChainDigestRole>(value)
    } else if matches!(path, "pre_state_root" | "post_state_root") {
        is_digest::<StateDigestRole>(value)
    } else if matches!(path, "pack_root" | "pack.pack_root" | "pointer.pack_root") {
        is_digest::<PackDigestRole>(value)
    } else if matches!(
        path,
        "archive_root" | "pointer.archive_root" | "packed_archives_root" | "retained_manifest_root"
    ) {
        is_digest::<ArchiveDigestRole>(value)
    } else if path == "ledger_hash" {
        is_digest::<LedgerDigestRole>(value)
    } else {
        is_digest::<ArtifactDigestRole>(value)
    };
    if !valid {
        issues.push(issue(
            "archive_compaction.hash_malformed",
            path,
            "hash must be lowercase BLAKE3 hex in the declared role",
        ));
    }
}

fn is_digest<Role: DigestRoleMarker>(value: &str) -> bool {
    Blake3Digest::<Role>::parse(value, RAW_BLAKE3_PROFILE, "archive_compaction.digest").is_ok()
}

pub(crate) fn same_digest<Role: DigestRoleMarker>(left: &str, right: &str) -> bool {
    let left =
        Blake3Digest::<Role>::parse(left, RAW_BLAKE3_PROFILE, "archive_compaction.left_digest");
    let right =
        Blake3Digest::<Role>::parse(right, RAW_BLAKE3_PROFILE, "archive_compaction.right_digest");
    matches!(
        (left, right),
        (Ok(left), Ok(right)) if left.as_hex() == right.as_hex()
    )
}

fn archive_file_role(path: &str) -> &'static str {
    if path == "proposal.md" {
        "proposal"
    } else if path == "design.md" {
        "design"
    } else if path == "tasks.md" {
        "tasks"
    } else if path == "metadata.json" {
        "metadata"
    } else if path.starts_with("specs/") && path.ends_with("/spec.md") {
        "delta_spec"
    } else {
        "archive_file"
    }
}

// r[impl cairn.sync_archive.compaction.manifest_roots.deduplicate]
fn build_pack_manifest(mut archives: Vec<ArchiveManifest>) -> PackManifest {
    archives.sort_by(|left, right| left.archive_id.cmp(&right.archive_id));
    let mut objects = BTreeMap::<String, u64>::new();
    for archive in &archives {
        for entry in &archive.entries {
            objects
                .entry(entry.content_hash.clone())
                .and_modify(|length| debug_assert_eq!(*length, entry.byte_length))
                .or_insert(entry.byte_length);
        }
    }
    let objects = objects
        .into_iter()
        .map(|(content_hash, byte_length)| PackObjectDescriptor {
            content_hash,
            byte_length,
        })
        .collect::<Vec<_>>();
    let packed_archives_root = canonical_json_hash(&json!({
        "archives": archives
            .iter()
            .map(|archive| json!({
                "archive_id": archive.archive_id,
                "archive_root": archive.archive_root,
            }))
            .collect::<Vec<_>>(),
    }));
    let mut pack = PackManifest {
        schema_version: COMPACTION_PACK_SCHEMA_V1.to_string(),
        archives,
        objects,
        packed_archives_root,
        pack_root: String::new(),
    };
    pack.pack_root = pack_manifest_hash(&pack);
    pack
}

fn pack_manifest_hash(pack: &PackManifest) -> String {
    let mut material = pack.clone();
    material.pack_root.clear();
    hash_serializable(&material)
}

fn validate_pointers(pointers: &[ArchivePointer], issues: &mut Vec<CompactionIssue>) {
    let mut seen = BTreeSet::new();
    for pointer in pointers {
        if pointer.schema_version != COMPACTION_POINTER_SCHEMA_V1 {
            issues.push(issue(
                "archive_compaction.pointer_schema_unsupported",
                &pointer.archive_id,
                "pointer schema must be cairn.archive-pointer.v1",
            ));
        }
        if !safe_archive_id(&pointer.archive_id) {
            issues.push(issue(
                "archive_compaction.pointer_archive_unsafe",
                &pointer.archive_id,
                "pointer archive identity must be one safe path component",
            ));
        }
        if !seen.insert(pointer.archive_id.clone()) {
            issues.push(issue(
                "archive_compaction.pointer_duplicate",
                &pointer.archive_id,
                "retained pointer archive identities must be unique",
            ));
        }
        validate_hash(&pointer.archive_root, "pointer.archive_root", issues);
        validate_hash(&pointer.pack_root, "pointer.pack_root", issues);
        if pointer.lossiness != LOSSINESS_LOSSLESS {
            issues.push(issue(
                "archive_compaction.pointer_lossiness_unsupported",
                &pointer.archive_id,
                "only lossless retained pointers are supported",
            ));
        }
        if !safe_relative_path(&pointer.pack_manifest_path) {
            issues.push(issue(
                "archive_compaction.pointer_path_unsafe",
                &pointer.pack_manifest_path,
                "pointer pack manifest path must be repository-relative",
            ));
        }
    }
}

fn build_mutation_manifest(
    pack: &PackManifest,
    planned_pointers: &[ArchivePointer],
) -> Vec<MutationAction> {
    let mut actions = pack
        .objects
        .iter()
        .map(|object| MutationAction {
            kind: "write_object".to_string(),
            path: object_path(&object.content_hash),
            content_hash: Some(object.content_hash.clone()),
        })
        .collect::<Vec<_>>();
    actions.push(MutationAction {
        kind: "write_pack_manifest".to_string(),
        path: pack_manifest_path(&pack.pack_root),
        content_hash: Some(hash_serializable(pack)),
    });
    for pointer in planned_pointers {
        actions.push(MutationAction {
            kind: "write_prepared_pointer".to_string(),
            path: prepared_pointer_path(&pack.pack_root, &pointer.archive_id),
            content_hash: Some(hash_serializable(pointer)),
        });
    }
    actions.push(MutationAction {
        kind: "write_mutation_manifest".to_string(),
        path: mutation_manifest_path(&pack.pack_root),
        content_hash: None,
    });
    actions.push(MutationAction {
        kind: "write_receipt".to_string(),
        path: receipt_path(&pack.pack_root),
        content_hash: None,
    });
    actions.push(MutationAction {
        kind: "append_ledger".to_string(),
        path: ledger_path(),
        content_hash: None,
    });
    for pointer in planned_pointers {
        actions.push(MutationAction {
            kind: "prune_expanded_archive".to_string(),
            path: format!(".cairn/archive/{}", pointer.archive_id),
            content_hash: Some(pointer.archive_root.clone()),
        });
        actions.push(MutationAction {
            kind: "finalize_pointer".to_string(),
            path: pointer_path(&pointer.archive_id),
            content_hash: Some(hash_serializable(pointer)),
        });
    }
    actions.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    actions
}
