pub fn build_compaction_receipt(
    plan: &super::model::CompactionPlan,
    sequence: u64,
) -> super::model::CompactionReceipt {
    // r[impl cairn.sync_archive.compaction.receipt_chain]
    // r[impl cairn.sync_archive.compaction.receipt_chain.fields]
    let mut receipt = super::model::CompactionReceipt {
        schema_version: super::model::COMPACTION_RECEIPT_SCHEMA_V1.to_string(),
        sequence,
        profile: plan.profile.clone(),
        lossiness: plan.lossiness.clone(),
        pack_root: plan.pack.pack_root.clone(),
        prev_chain_hash: plan.prev_chain_hash.clone(),
        pre_state_root: plan.pre_state_root.clone(),
        post_state_root: plan.post_state_root.clone(),
        packed_archives_root: plan.packed_archives_root.clone(),
        retained_manifest_root: plan.retained_manifest_root.clone(),
        policy_hash: plan.policy_hash.clone(),
        plan_hash: plan.plan_hash.clone(),
        mutation_manifest_hash: plan.mutation_manifest_hash.clone(),
        receipt_hash: String::new(),
        chain_hash: String::new(),
        non_claims: vec![super::model::COMPACTION_NON_CLAIM.to_string()],
    };
    receipt.receipt_hash = compaction_receipt_hash(&receipt);
    receipt.chain_hash =
        super::plan::compaction_chain_hash(&receipt.prev_chain_hash, &receipt.receipt_hash);
    receipt
}

pub fn compaction_receipt_hash(receipt: &super::model::CompactionReceipt) -> String {
    let mut material = receipt.clone();
    material.receipt_hash.clear();
    material.chain_hash.clear();
    super::plan::hash_serializable(&material)
}

pub fn validate_compaction_receipt(
    receipt: &super::model::CompactionReceipt,
) -> Vec<super::model::CompactionIssue> {
    let mut issues = Vec::new();
    if receipt.schema_version != super::model::COMPACTION_RECEIPT_SCHEMA_V1 {
        issues.push(super::plan::issue(
            "archive_compaction.receipt_schema_unsupported",
            "schema_version",
            "receipt schema must be cairn.archive-compaction-receipt.v1",
        ));
    }
    for (path, value) in [
        ("pack_root", receipt.pack_root.as_str()),
        ("prev_chain_hash", receipt.prev_chain_hash.as_str()),
        ("pre_state_root", receipt.pre_state_root.as_str()),
        ("post_state_root", receipt.post_state_root.as_str()),
        (
            "packed_archives_root",
            receipt.packed_archives_root.as_str(),
        ),
        (
            "retained_manifest_root",
            receipt.retained_manifest_root.as_str(),
        ),
        ("policy_hash", receipt.policy_hash.as_str()),
        ("plan_hash", receipt.plan_hash.as_str()),
        (
            "mutation_manifest_hash",
            receipt.mutation_manifest_hash.as_str(),
        ),
        ("receipt_hash", receipt.receipt_hash.as_str()),
        ("chain_hash", receipt.chain_hash.as_str()),
    ] {
        super::plan::validate_hash(value, path, &mut issues);
    }
    if !receipt
        .non_claims
        .iter()
        .any(|non_claim| non_claim == super::model::COMPACTION_NON_CLAIM)
    {
        issues.push(super::plan::issue(
            "archive_compaction.non_claim_missing",
            "non_claims",
            "compaction receipt must preserve the local-integrity-only non-claim",
        ));
    }
    if !super::plan::same_digest::<crate::verified::nominal_identity::ReceiptDigestRole>(
        &compaction_receipt_hash(receipt),
        &receipt.receipt_hash,
    ) {
        issues.push(super::plan::issue(
            "archive_compaction.receipt_hash_mismatch",
            "receipt_hash",
            "receipt hash must match canonical receipt material",
        ));
    }
    let expected_chain =
        super::plan::compaction_chain_hash(&receipt.prev_chain_hash, &receipt.receipt_hash);
    if !super::plan::same_digest::<crate::verified::nominal_identity::ChainDigestRole>(
        &expected_chain,
        &receipt.chain_hash,
    ) {
        issues.push(super::plan::issue(
            "archive_compaction.chain_hash_mismatch",
            "chain_hash",
            "chain hash must bind the previous chain and current receipt hashes",
        ));
    }
    issues.sort();
    issues.dedup();
    issues
}

// r[impl cairn.sync_archive.compaction.receipt_chain.genesis]
pub fn empty_compaction_ledger(policy_hash: &str) -> super::model::CompactionLedger {
    let genesis_chain_hash = super::plan::compaction_genesis_hash(policy_hash);
    let mut ledger = super::model::CompactionLedger {
        schema_version: super::model::COMPACTION_LEDGER_SCHEMA_V1.to_string(),
        policy_hash: policy_hash.to_string(),
        genesis_chain_hash: genesis_chain_hash.clone(),
        entries: Vec::new(),
        latest_chain_hash: genesis_chain_hash,
        ledger_hash: String::new(),
    };
    ledger.ledger_hash = compaction_ledger_hash(&ledger);
    ledger
}

pub fn append_compaction_receipt(
    existing: Option<&super::model::CompactionLedger>,
    existing_receipts: &[super::model::CompactionReceipt],
    receipt: &super::model::CompactionReceipt,
) -> Result<super::model::CompactionLedger, Vec<super::model::CompactionIssue>> {
    let mut ledger = existing
        .cloned()
        .unwrap_or_else(|| empty_compaction_ledger(&receipt.policy_hash));
    let mut issues = validate_compaction_ledger(&ledger, existing_receipts);
    issues.extend(validate_compaction_receipt(receipt));
    let expected_sequence = match u64::try_from(ledger.entries.len()) {
        Ok(length) => length.saturating_add(1),
        Err(_) => {
            issues.push(super::plan::issue(
                "archive_compaction.ledger_too_large",
                "entries",
                "ledger entry count must fit in u64",
            ));
            0
        }
    };
    if receipt.sequence != expected_sequence {
        issues.push(super::plan::issue(
            "archive_compaction.sequence_mismatch",
            "sequence",
            "receipt sequence must append exactly one ledger entry",
        ));
    }
    if !super::plan::same_digest::<crate::verified::nominal_identity::ChainDigestRole>(
        &receipt.prev_chain_hash,
        &ledger.latest_chain_hash,
    ) {
        issues.push(super::plan::issue(
            "archive_compaction.previous_chain_mismatch",
            "prev_chain_hash",
            "receipt predecessor must match the ledger tip",
        ));
    }
    issues.sort();
    issues.dedup();
    if !issues.is_empty() {
        return Err(issues);
    }
    ledger.entries.push(super::model::CompactionLedgerEntry {
        sequence: receipt.sequence,
        receipt_path: super::plan::receipt_path(&receipt.pack_root),
        receipt_hash: receipt.receipt_hash.clone(),
        chain_hash: receipt.chain_hash.clone(),
    });
    ledger.latest_chain_hash = receipt.chain_hash.clone();
    ledger.ledger_hash = compaction_ledger_hash(&ledger);
    Ok(ledger)
}

pub fn compaction_ledger_hash(ledger: &super::model::CompactionLedger) -> String {
    let mut material = ledger.clone();
    material.ledger_hash.clear();
    super::plan::hash_serializable(&material)
}

pub fn validate_compaction_ledger(
    ledger: &super::model::CompactionLedger,
    receipts: &[super::model::CompactionReceipt],
) -> Vec<super::model::CompactionIssue> {
    // r[impl cairn.sync_archive.compaction.receipt_chain]
    // r[impl cairn.sync_archive.compaction.receipt_chain.removed]
    // r[impl cairn.sync_archive.compaction.verify]
    let mut issues = Vec::new();
    if ledger.schema_version != super::model::COMPACTION_LEDGER_SCHEMA_V1 {
        issues.push(super::plan::issue(
            "archive_compaction.ledger_schema_unsupported",
            "schema_version",
            "ledger schema must be cairn.archive-compaction-ledger.v1",
        ));
    }
    super::plan::validate_hash(&ledger.policy_hash, "policy_hash", &mut issues);
    super::plan::validate_hash(
        &ledger.genesis_chain_hash,
        "genesis_chain_hash",
        &mut issues,
    );
    super::plan::validate_hash(&ledger.latest_chain_hash, "latest_chain_hash", &mut issues);
    super::plan::validate_hash(&ledger.ledger_hash, "ledger_hash", &mut issues);
    if !super::plan::same_digest::<crate::verified::nominal_identity::ChainDigestRole>(
        &super::plan::compaction_genesis_hash(&ledger.policy_hash),
        &ledger.genesis_chain_hash,
    ) {
        issues.push(super::plan::issue(
            "archive_compaction.genesis_hash_mismatch",
            "genesis_chain_hash",
            "ledger genesis must be derived from the policy-owned domain and policy hash",
        ));
    }
    if !super::plan::same_digest::<crate::verified::nominal_identity::LedgerDigestRole>(
        &compaction_ledger_hash(ledger),
        &ledger.ledger_hash,
    ) {
        issues.push(super::plan::issue(
            "archive_compaction.ledger_hash_mismatch",
            "ledger_hash",
            "ledger hash must match canonical ledger material",
        ));
    }
    if !receipts.is_empty() && receipts.len() != ledger.entries.len() {
        issues.push(super::plan::issue(
            "archive_compaction.ledger_receipt_count_mismatch",
            "entries",
            "every ledger entry must have one observed receipt",
        ));
    }
    let mut expected_prev = ledger.genesis_chain_hash.as_str();
    for (index, entry) in ledger.entries.iter().enumerate() {
        let expected_sequence = match u64::try_from(index) {
            Ok(value) => value.saturating_add(1),
            Err(_) => 0,
        };
        if entry.sequence != expected_sequence {
            issues.push(super::plan::issue(
                "archive_compaction.ledger_sequence_mismatch",
                &format!("entries[{index}].sequence"),
                "ledger sequences must be contiguous and one-based",
            ));
        }
        super::plan::validate_hash(
            &entry.receipt_hash,
            &format!("entries[{index}].receipt_hash"),
            &mut issues,
        );
        super::plan::validate_hash(
            &entry.chain_hash,
            &format!("entries[{index}].chain_hash"),
            &mut issues,
        );
        if let Some(receipt) = receipts.get(index) {
            issues.extend(validate_compaction_receipt(receipt));
            if receipt.sequence != entry.sequence
                || !super::plan::same_digest::<crate::verified::nominal_identity::ReceiptDigestRole>(
                    &receipt.receipt_hash,
                    &entry.receipt_hash,
                )
                || !super::plan::same_digest::<crate::verified::nominal_identity::ChainDigestRole>(
                    &receipt.chain_hash,
                    &entry.chain_hash,
                )
                || super::plan::receipt_path(&receipt.pack_root) != entry.receipt_path
            {
                issues.push(super::plan::issue(
                    "archive_compaction.ledger_entry_mismatch",
                    &format!("entries[{index}]"),
                    "ledger entry must bind its exact receipt identity",
                ));
            }
            if !super::plan::same_digest::<crate::verified::nominal_identity::ChainDigestRole>(
                &receipt.prev_chain_hash,
                expected_prev,
            ) {
                issues.push(super::plan::issue(
                    "archive_compaction.receipt_predecessor_mismatch",
                    &format!("entries[{index}]"),
                    "receipt predecessor must match the prior ledger chain hash",
                ));
            }
        }
        expected_prev = &entry.chain_hash;
    }
    if !super::plan::same_digest::<crate::verified::nominal_identity::ChainDigestRole>(
        &ledger.latest_chain_hash,
        expected_prev,
    ) {
        issues.push(super::plan::issue(
            "archive_compaction.ledger_tip_mismatch",
            "latest_chain_hash",
            "ledger tip must match the final chain entry or genesis",
        ));
    }
    issues.sort();
    issues.dedup();
    issues
}
