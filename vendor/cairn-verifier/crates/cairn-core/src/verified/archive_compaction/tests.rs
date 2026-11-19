use super::*;

const PROPOSAL_BYTES: &[u8] = b"## Why\n\nExact bytes.\n";
const TASK_BYTES: &[u8] = b"## Tasks\n\n- [x] Done\n";

fn hash(label: &str) -> String {
    crate::verified::evidence::blake3_hex(label.as_bytes())
}

fn archive(id: &str) -> ArchiveInput {
    ArchiveInput {
        archive_id: id.to_string(),
        files: vec![
            ArchiveFileInput {
                path: "proposal.md".to_string(),
                bytes: PROPOSAL_BYTES.to_vec(),
            },
            ArchiveFileInput {
                path: "tasks.md".to_string(),
                bytes: TASK_BYTES.to_vec(),
            },
        ],
    }
}

fn plan_for(id: &str, previous: &str) -> CompactionPlan {
    build_compaction_plan(
        &[archive(id)],
        &[],
        &hash("policy"),
        LOSSLESS_PACK_PROFILE,
        previous,
        &[],
    )
}

fn objects_for(archive: &ArchiveInput) -> Vec<PackObjectInput> {
    archive
        .files
        .iter()
        .map(|file| PackObjectInput {
            content_hash: crate::verified::evidence::blake3_hex(&file.bytes),
            bytes: file.bytes.clone(),
        })
        .collect()
}

#[test]
// r[verify cairn.sync_archive.compaction.plan]
// r[verify cairn.sync_archive.compaction.plan.ordering]
// r[verify cairn.sync_archive.compaction.manifest_roots]
// r[verify cairn.sync_archive.compaction.manifest_roots.entries]
// r[verify cairn.sync_archive.compaction.manifest_roots.stable]
// r[verify cairn.sync_archive.compaction.manifest_roots.deduplicate]
// r[verify cairn.sync_archive.compaction.tests]
// r[verify cairn.sync_archive.compaction.tests.positive]
fn plans_and_manifest_roots_are_deterministic() {
    let policy_hash = hash("policy");
    let genesis = compaction_genesis_hash(&policy_hash);
    let first = plan_for("2026-07-13-example", &genesis);
    let second = plan_for("2026-07-13-example", &genesis);

    assert!(first.blockers.is_empty(), "{:?}", first.blockers);
    assert_eq!(first, second);
    assert_eq!(first.candidates, vec!["2026-07-13-example"]);
    assert!(validate_compaction_plan(&first).is_empty());
    assert_ne!(first.pre_state_root, first.post_state_root);
}

#[test]
// r[verify cairn.sync_archive.compaction.manifest_roots]
// r[verify cairn.sync_archive.compaction.tests]
fn duplicate_and_unsafe_manifest_paths_fail_closed() {
    let input = ArchiveInput {
        archive_id: "2026-07-13-example".to_string(),
        files: vec![
            ArchiveFileInput {
                path: "../escape".to_string(),
                bytes: Vec::new(),
            },
            ArchiveFileInput {
                path: "tasks.md".to_string(),
                bytes: TASK_BYTES.to_vec(),
            },
            ArchiveFileInput {
                path: "tasks.md".to_string(),
                bytes: TASK_BYTES.to_vec(),
            },
        ],
    };
    let issues = build_archive_manifest(&input).expect_err("invalid manifest must fail");
    let codes = issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();

    assert!(codes.contains(&"archive_compaction.path_unsafe"));
    assert!(codes.contains(&"archive_compaction.path_duplicate"));
}

#[test]
// r[verify cairn.sync_archive.compaction.receipt_chain]
// r[verify cairn.sync_archive.compaction.receipt_chain.fields]
// r[verify cairn.sync_archive.compaction.receipt_chain.removed]
// r[verify cairn.sync_archive.compaction.receipt_chain.genesis]
// r[verify cairn.sync_archive.compaction.tests]
// r[verify cairn.sync_archive.compaction.tests.negative]
fn receipt_chain_detects_removed_predecessor() {
    let policy_hash = hash("policy");
    let genesis = compaction_genesis_hash(&policy_hash);
    let first_plan = plan_for("2026-07-13-first", &genesis);
    let first_receipt = build_compaction_receipt(&first_plan, 1);
    let first_ledger = append_compaction_receipt(None, &[], &first_receipt).expect("first append");
    let second_plan = plan_for("2026-07-13-second", &first_receipt.chain_hash);
    let second_receipt = build_compaction_receipt(&second_plan, 2);
    let ledger = append_compaction_receipt(
        Some(&first_ledger),
        std::slice::from_ref(&first_receipt),
        &second_receipt,
    )
    .expect("second append");

    assert!(
        validate_compaction_ledger(&ledger, &[first_receipt.clone(), second_receipt.clone()])
            .is_empty()
    );
    let issues = validate_compaction_ledger(&ledger, &[second_receipt]);
    assert!(!issues.is_empty());
    assert!(issues.iter().any(|issue| {
        issue.code == "archive_compaction.ledger_receipt_count_mismatch"
            || issue.code == "archive_compaction.receipt_predecessor_mismatch"
    }));
}

#[test]
// r[verify cairn.sync_archive.compaction.receipt_chain]
// r[verify cairn.sync_archive.compaction.tests]
fn receipt_chain_preserves_history_across_policy_updates() {
    let first_policy_hash = hash("first-policy");
    let first_genesis = compaction_genesis_hash(&first_policy_hash);
    let first_plan = build_compaction_plan(
        &[archive("2026-07-13-first")],
        &[],
        &first_policy_hash,
        LOSSLESS_PACK_PROFILE,
        &first_genesis,
        &[],
    );
    let first_receipt = build_compaction_receipt(&first_plan, 1);
    let first_ledger = append_compaction_receipt(None, &[], &first_receipt).expect("first append");
    let second_policy_hash = hash("second-policy");
    let second_plan = build_compaction_plan(
        &[archive("2026-07-13-second")],
        &[],
        &second_policy_hash,
        LOSSLESS_PACK_PROFILE,
        &first_receipt.chain_hash,
        &[],
    );
    let second_receipt = build_compaction_receipt(&second_plan, 2);

    let ledger = append_compaction_receipt(
        Some(&first_ledger),
        std::slice::from_ref(&first_receipt),
        &second_receipt,
    )
    .expect("policy update append");

    assert_eq!(ledger.policy_hash, first_policy_hash);
    assert!(validate_compaction_ledger(&ledger, &[first_receipt, second_receipt]).is_empty());
}

#[test]
// r[verify cairn.sync_archive.compaction.verify]
// r[verify cairn.sync_archive.compaction.verify.tampered_object]
// r[verify cairn.sync_archive.compaction.tests]
// r[verify cairn.sync_archive.compaction.tests.positive]
// r[verify cairn.sync_archive.compaction.tests.negative]
fn pack_verification_accepts_exact_objects_and_rejects_tampering() {
    let policy_hash = hash("policy");
    let genesis = compaction_genesis_hash(&policy_hash);
    let source = archive("2026-07-13-example");
    let plan = build_compaction_plan(
        std::slice::from_ref(&source),
        &[],
        &policy_hash,
        LOSSLESS_PACK_PROFILE,
        &genesis,
        &[],
    );
    let objects = objects_for(&source);

    assert!(verify_pack_objects(&plan.pack, &objects).is_empty());
    let mut tampered = objects;
    tampered[0].bytes.push(b'!');
    let issues = verify_pack_objects(&plan.pack, &tampered);
    assert!(issues
        .iter()
        .any(|issue| issue.code == "archive_compaction.object_hash_mismatch"));
}

#[test]
// r[verify cairn.sync_archive.compaction.lossiness_policy]
// r[verify cairn.sync_archive.compaction.lossiness_policy.default_reject]
// r[verify cairn.sync_archive.compaction.lossiness_policy.non_claims]
// r[verify cairn.sync_archive.compaction.tests]
// r[verify cairn.sync_archive.compaction.tests.negative]
fn lossy_profile_is_rejected_by_default() {
    let policy_hash = hash("policy");
    let genesis = compaction_genesis_hash(&policy_hash);
    let plan = build_compaction_plan(
        &[archive("2026-07-13-example")],
        &[],
        &policy_hash,
        LOSSY_SUMMARY_PROFILE,
        &genesis,
        &[],
    );

    assert!(plan
        .blockers
        .iter()
        .any(|issue| issue.code == "archive_compaction.profile_unsupported"));
    assert!(!validate_compaction_plan(&plan).is_empty());
}
