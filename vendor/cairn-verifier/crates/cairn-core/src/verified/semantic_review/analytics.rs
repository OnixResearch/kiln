pub fn review_ledger_analytics(receipts: &[serde_json::Value]) -> serde_json::Value {
    // r[impl cairn.semantic_review.review_ledger_analytics.aggregate]
    // r[impl cairn.semantic_review.review_ledger_analytics.aggregate.aggregate.counts]
    // r[impl cairn.semantic_review.review_ledger_analytics.aggregate.aggregate.hashes]
    let mut valid_receipts = Vec::new();
    let mut invalid_inputs = Vec::new();
    for (index, receipt) in receipts.iter().enumerate() {
        let validation = super::validate_review_receipt(receipt, &[]);
        if validation.valid {
            valid_receipts.push(receipt.clone());
        } else {
            invalid_inputs.push(serde_json::json!({
                "index": index,
                "issues": super::common::issues_json(&validation.issues),
            }));
        }
    }
    let by_kind = super::common::count_string_field(&valid_receipts, "review_kind");
    let by_decision = super::common::count_string_field(&valid_receipts, "decision");
    let mut by_category: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for receipt in &valid_receipts {
        if let Some(findings) = receipt
            .get("findings")
            .and_then(serde_json::Value::as_array)
        {
            for finding in findings {
                if let Some(category) = finding.get("category").and_then(serde_json::Value::as_str)
                {
                    super::common::increment_count(&mut by_category, category);
                }
            }
        }
    }
    let by_category = super::common::sort_count_map(&mut by_category);
    let mut input_receipt_hashes = valid_receipts
        .iter()
        .map(super::review_receipt_hash)
        .collect::<Vec<_>>();
    input_receipt_hashes.sort();
    let mut warnings = Vec::new();
    if valid_receipts.len() < 5 {
        warnings.push(
            serde_json::json!({"code": "small_sample", "message": "fewer than five valid review receipts"}),
        );
    }
    let mut receipt = serde_json::json!({
        "schema_version": super::REVIEW_ANALYTICS_SCHEMA_V1,
        "valid": invalid_inputs.is_empty(),
        "verdict": if invalid_inputs.is_empty() {"PASS"} else {"FAIL"},
        "input_count": receipts.len(),
        "valid_input_count": valid_receipts.len(),
        "invalid_inputs": invalid_inputs,
        "input_receipt_hashes": input_receipt_hashes,
        "counts": {
            "by_review_kind": by_kind,
            "by_decision": by_decision,
            "by_category": by_category,
        },
        "warnings": warnings,
    });
    let hash = crate::verified::evidence::canonical_json_hash(&receipt);
    receipt["receipt_hash"] = serde_json::json!(hash);
    receipt
}
