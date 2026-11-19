// r[impl cairn.global_registry.query]
// r[impl cairn.global_registry.query.failure]
pub fn query_registry_exact_id(
    snapshot: &super::GlobalRegistrySnapshot,
    stable_id: &str,
) -> super::RegistryQueryReport {
    let mut selected_ids = snapshot
        .nodes
        .iter()
        .filter(|node| node.stable_id == stable_id)
        .map(|node| node.stable_id.clone())
        .chain(
            snapshot
                .edges
                .iter()
                .filter(|edge| edge.stable_id == stable_id)
                .map(|edge| edge.stable_id.clone()),
        )
        .collect::<Vec<_>>();
    selected_ids.sort();
    selected_ids.dedup();
    let issues = if selected_ids.is_empty() {
        vec![super::issue(
            "registry.query_id_unknown",
            "query",
            stable_id,
            "stable_id",
            Some("present node or edge ID"),
            Some(stable_id),
            "exact-ID query did not match the selected snapshot",
        )]
    } else {
        Vec::new()
    };
    finalize_query(snapshot, format!("id={stable_id}"), selected_ids, issues)
}

// r[impl cairn.global_registry.query.tags]
pub fn query_registry_tags(
    snapshot: &super::GlobalRegistrySnapshot,
    selectors: &[(String, String, String)],
) -> super::RegistryQueryReport {
    let mut normalized = selectors.to_vec();
    normalized.sort();
    normalized.dedup();
    let mut issues = Vec::new();
    if normalized.is_empty()
        || normalized.iter().any(|(namespace, key, value)| {
            namespace.is_empty() || key.is_empty() || value.is_empty()
        })
    {
        issues.push(super::issue(
            "registry.query_selector_invalid",
            "query",
            "tags",
            "selectors",
            Some("one or more non-empty namespace/key/value selectors"),
            None,
            "tag query selector is empty or malformed",
        ));
    }
    let selected_ids = if issues.is_empty() {
        let by_selector = normalized
            .iter()
            .map(|(namespace, key, value)| {
                snapshot
                    .tags
                    .iter()
                    .filter(|tag| {
                        tag.namespace == *namespace && tag.key == *key && tag.value == *value
                    })
                    .map(|tag| tag.subject_id.clone())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .collect::<Vec<_>>();
        by_selector
            .into_iter()
            .reduce(|left, right| left.intersection(&right).cloned().collect())
            .unwrap_or_default()
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };
    finalize_query(
        snapshot,
        format!(
            "tags={}",
            normalized
                .iter()
                .map(|(namespace, key, value)| format!("{namespace}:{key}={value}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        selected_ids,
        issues,
    )
}

fn finalize_query(
    snapshot: &super::GlobalRegistrySnapshot,
    selector: String,
    mut selected_ids: Vec<String>,
    mut issues: Vec<super::RegistryIssue>,
) -> super::RegistryQueryReport {
    selected_ids.sort();
    selected_ids.dedup();
    issues.sort_by(super::issue_order);
    let valid = issues.is_empty();
    let result_hash = crate::verified::evidence::canonical_json_hash(&serde_json::json!({
        "snapshot_hash": snapshot.snapshot_hash,
        "selector": selector,
        "selected_ids": selected_ids,
    }));
    let mut report = super::RegistryQueryReport {
        valid,
        verdict: if valid { "PASS" } else { "FAIL" }.to_string(),
        snapshot_hash: snapshot.snapshot_hash.clone(),
        selector,
        selected_ids,
        issues,
        result_hash,
        receipt_hash: String::new(),
        non_claims: super::registry_non_claims(snapshot),
    };
    report.receipt_hash = super::report_hash(&report);
    report
}
