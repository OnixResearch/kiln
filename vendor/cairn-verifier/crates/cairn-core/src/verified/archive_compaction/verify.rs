pub fn validate_pack_manifest(
    pack: &super::model::PackManifest,
) -> Vec<super::model::CompactionIssue> {
    let mut issues = Vec::new();
    if pack.schema_version != super::model::COMPACTION_PACK_SCHEMA_V1 {
        issues.push(super::plan::issue(
            "archive_compaction.pack_schema_unsupported",
            "schema_version",
            "pack schema must be cairn.archive-pack.v1",
        ));
    }
    super::plan::validate_hash(&pack.pack_root, "pack_root", &mut issues);
    super::plan::validate_hash(
        &pack.packed_archives_root,
        "packed_archives_root",
        &mut issues,
    );
    let mut seen_archives = std::collections::BTreeSet::new();
    let mut expected_objects = std::collections::BTreeMap::<String, u64>::new();
    if !pack
        .archives
        .windows(2)
        .all(|pair| pair[0].archive_id < pair[1].archive_id)
    {
        issues.push(super::plan::issue(
            "archive_compaction.pack_archive_order_invalid",
            "archives",
            "pack archives must be strictly sorted by archive identity",
        ));
    }
    if !pack
        .objects
        .windows(2)
        .all(|pair| pair[0].content_hash < pair[1].content_hash)
    {
        issues.push(super::plan::issue(
            "archive_compaction.pack_object_order_invalid",
            "objects",
            "pack objects must be strictly sorted by content hash",
        ));
    }
    let mut archive_roots = Vec::new();
    for archive in &pack.archives {
        if !seen_archives.insert(archive.archive_id.clone()) {
            issues.push(super::plan::issue(
                "archive_compaction.pack_archive_duplicate",
                &archive.archive_id,
                "pack archive identities must be unique",
            ));
        }
        let mut seen_paths = std::collections::BTreeSet::new();
        if !archive
            .entries
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path)
        {
            issues.push(super::plan::issue(
                "archive_compaction.manifest_path_order_invalid",
                &archive.archive_id,
                "archive manifest entries must be strictly sorted by path",
            ));
        }
        for entry in &archive.entries {
            if entry.archive_id != archive.archive_id {
                issues.push(super::plan::issue(
                    "archive_compaction.entry_archive_mismatch",
                    &entry.path,
                    "manifest entry archive identity must match its parent manifest",
                ));
            }
            if !super::plan::safe_relative_path(&entry.path) {
                issues.push(super::plan::issue(
                    "archive_compaction.path_unsafe",
                    &entry.path,
                    "manifest entry path must be normalized and relative",
                ));
            }
            if !seen_paths.insert(entry.path.clone()) {
                issues.push(super::plan::issue(
                    "archive_compaction.path_duplicate",
                    &entry.path,
                    "manifest entry paths must be unique within an archive",
                ));
            }
            super::plan::validate_hash(&entry.content_hash, &entry.path, &mut issues);
            match expected_objects.get(&entry.content_hash) {
                Some(length) if *length != entry.byte_length => issues.push(super::plan::issue(
                    "archive_compaction.object_length_conflict",
                    &entry.content_hash,
                    "identical object hashes must declare identical byte lengths",
                )),
                Some(_) => {}
                None => {
                    expected_objects.insert(entry.content_hash.clone(), entry.byte_length);
                }
            }
        }
        let expected_archive_root =
            crate::verified::evidence::canonical_json_hash(&serde_json::json!({
                "archive_id": archive.archive_id,
                "entries": archive.entries,
            }));
        if !super::plan::same_digest::<crate::verified::nominal_identity::ArchiveDigestRole>(
            &archive.archive_root,
            &expected_archive_root,
        ) {
            issues.push(super::plan::issue(
                "archive_compaction.archive_root_mismatch",
                &archive.archive_id,
                "archive root must match canonical manifest entries",
            ));
        }
        archive_roots.push(serde_json::json!({
            "archive_id": archive.archive_id,
            "archive_root": archive.archive_root,
        }));
    }
    let expected_packed_root = crate::verified::evidence::canonical_json_hash(
        &serde_json::json!({"archives": archive_roots}),
    );
    if !super::plan::same_digest::<crate::verified::nominal_identity::ArchiveDigestRole>(
        &pack.packed_archives_root,
        &expected_packed_root,
    ) {
        issues.push(super::plan::issue(
            "archive_compaction.packed_archives_root_mismatch",
            "packed_archives_root",
            "packed archives root must match canonical archive identities",
        ));
    }
    let declared_objects = pack
        .objects
        .iter()
        .map(|object| (object.content_hash.clone(), object.byte_length))
        .collect::<std::collections::BTreeMap<_, _>>();
    if declared_objects != expected_objects || declared_objects.len() != pack.objects.len() {
        issues.push(super::plan::issue(
            "archive_compaction.object_manifest_mismatch",
            "objects",
            "pack object descriptors must exactly match referenced archive entries",
        ));
    }
    let mut pack_material = pack.clone();
    pack_material.pack_root.clear();
    if !super::plan::same_digest::<crate::verified::nominal_identity::PackDigestRole>(
        &super::plan::hash_serializable(&pack_material),
        &pack.pack_root,
    ) {
        issues.push(super::plan::issue(
            "archive_compaction.pack_root_mismatch",
            "pack_root",
            "pack root must match canonical pack material",
        ));
    }
    issues.sort();
    issues.dedup();
    issues
}

pub fn verify_pack_objects(
    pack: &super::model::PackManifest,
    objects: &[super::model::PackObjectInput],
) -> Vec<super::model::CompactionIssue> {
    // r[impl cairn.sync_archive.compaction.verify]
    // r[impl cairn.sync_archive.compaction.verify.tampered_object]
    let mut issues = validate_pack_manifest(pack);
    let mut observed = std::collections::BTreeMap::<String, &[u8]>::new();
    for object in objects {
        if observed
            .insert(object.content_hash.clone(), &object.bytes)
            .is_some()
        {
            issues.push(super::plan::issue(
                "archive_compaction.object_duplicate",
                &object.content_hash,
                "each observed object hash must be supplied once",
            ));
        }
    }
    for descriptor in &pack.objects {
        match observed.get(&descriptor.content_hash) {
            None => issues.push(super::plan::issue(
                "archive_compaction.object_missing",
                &descriptor.content_hash,
                "every declared pack object must be present",
            )),
            Some(bytes) => {
                let observed_length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                if observed_length != descriptor.byte_length {
                    issues.push(super::plan::issue(
                        "archive_compaction.object_length_mismatch",
                        &descriptor.content_hash,
                        "object byte length must match its descriptor",
                    ));
                }
                if !super::plan::same_digest::<crate::verified::nominal_identity::ArtifactDigestRole>(
                    &crate::verified::evidence::blake3_hex(bytes),
                    &descriptor.content_hash,
                ) {
                    issues.push(super::plan::issue(
                        "archive_compaction.object_hash_mismatch",
                        &descriptor.content_hash,
                        "object bytes must match their BLAKE3 identity",
                    ));
                }
            }
        }
    }
    for content_hash in observed.keys() {
        if !pack
            .objects
            .iter()
            .any(|descriptor| descriptor.content_hash == *content_hash)
        {
            issues.push(super::plan::issue(
                "archive_compaction.object_unreferenced",
                content_hash,
                "observed objects must be referenced by the pack manifest",
            ));
        }
    }
    issues.sort();
    issues.dedup();
    issues
}

pub fn build_unpack_plan(
    pointer: &super::model::ArchivePointer,
    pack: &super::model::PackManifest,
    objects: &[super::model::PackObjectInput],
) -> super::model::UnpackPlan {
    // r[impl cairn.sync_archive.compaction.lossless_pack]
    // r[impl cairn.sync_archive.compaction.lossless_pack.unpack]
    let mut blockers = verify_pack_objects(pack, objects);
    if !super::plan::same_digest::<crate::verified::nominal_identity::PackDigestRole>(
        &pointer.pack_root,
        &pack.pack_root,
    ) {
        blockers.push(super::plan::issue(
            "archive_compaction.pointer_pack_mismatch",
            &pointer.archive_id,
            "pointer pack root must match the loaded pack",
        ));
    }
    let archive = pack
        .archives
        .iter()
        .find(|archive| archive.archive_id == pointer.archive_id);
    let mut files = Vec::new();
    let archive_root = match archive {
        Some(archive) => {
            if !super::plan::same_digest::<crate::verified::nominal_identity::ArchiveDigestRole>(
                &archive.archive_root,
                &pointer.archive_root,
            ) {
                blockers.push(super::plan::issue(
                    "archive_compaction.pointer_archive_root_mismatch",
                    &pointer.archive_id,
                    "pointer archive root must match its pack manifest",
                ));
            }
            files = archive
                .entries
                .iter()
                .map(|entry| super::model::UnpackFile {
                    path: entry.path.clone(),
                    byte_length: entry.byte_length,
                    content_hash: entry.content_hash.clone(),
                })
                .collect();
            files.sort_by(|left, right| left.path.cmp(&right.path));
            archive.archive_root.clone()
        }
        None => {
            blockers.push(super::plan::issue(
                "archive_compaction.pointer_archive_missing",
                &pointer.archive_id,
                "pointer archive identity must exist in the pack manifest",
            ));
            String::new()
        }
    };
    blockers.sort();
    blockers.dedup();
    let mut plan = super::model::UnpackPlan {
        schema_version: super::model::COMPACTION_UNPACK_SCHEMA_V1.to_string(),
        archive_id: pointer.archive_id.clone(),
        archive_root,
        pack_root: pack.pack_root.clone(),
        files,
        blockers,
        plan_hash: String::new(),
    };
    plan.plan_hash = unpack_plan_hash(&plan);
    plan
}

pub fn unpack_plan_hash(plan: &super::model::UnpackPlan) -> String {
    let mut material = plan.clone();
    material.plan_hash.clear();
    super::plan::hash_serializable(&material)
}

pub fn build_verification_receipt(
    policy_hash: &str,
    ledger: &super::model::CompactionLedger,
    mut verified_pack_roots: Vec<String>,
    mut issues: Vec<super::model::CompactionIssue>,
) -> super::model::CompactionVerificationReceipt {
    verified_pack_roots.sort();
    verified_pack_roots.dedup();
    issues.sort();
    issues.dedup();
    let valid = issues.is_empty();
    let mut receipt = super::model::CompactionVerificationReceipt {
        schema_version: super::model::COMPACTION_VERIFY_SCHEMA_V1.to_string(),
        valid,
        policy_hash: policy_hash.to_string(),
        ledger_hash: ledger.ledger_hash.clone(),
        latest_chain_hash: ledger.latest_chain_hash.clone(),
        verified_pack_roots,
        issues,
        receipt_hash: String::new(),
        non_claims: vec![super::model::COMPACTION_NON_CLAIM.to_string()],
    };
    receipt.receipt_hash = verification_receipt_hash(&receipt);
    receipt
}

pub fn verification_receipt_hash(receipt: &super::model::CompactionVerificationReceipt) -> String {
    let mut material = receipt.clone();
    material.receipt_hash.clear();
    super::plan::hash_serializable(&material)
}
