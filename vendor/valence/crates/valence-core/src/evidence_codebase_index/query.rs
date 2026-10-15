use std::collections::BTreeSet;

use super::model::EvidenceCodebaseIndex;
use super::model::EvidenceIndexQueryReport;
use super::model::EvidenceIndexRecord;
use super::model::HASH_PREFIX;
use super::model::INDEX_BOUNDARY;
use super::model::INDEX_SCHEMA_VERSION;

#[must_use]
pub fn query_evidence_index(index: &EvidenceCodebaseIndex, query: &str) -> EvidenceIndexQueryReport {
    let mut selected_ids = BTreeSet::<String>::new();
    for record in &index.records {
        if record_matches(record, query) {
            selected_ids.insert(record.record_id.clone());
            for linked in linked_records(index, record) {
                selected_ids.insert(linked.record_id.clone());
            }
        }
    }

    let records = index
        .records
        .iter()
        .filter(|record| selected_ids.contains(&record.record_id))
        .cloned()
        .collect::<Vec<_>>();

    EvidenceIndexQueryReport {
        schema_version: INDEX_SCHEMA_VERSION.to_owned(),
        query: query.to_owned(),
        match_count: records.len(),
        boundary: INDEX_BOUNDARY.to_owned(),
        records,
    }
}
fn record_matches(record: &EvidenceIndexRecord, query: &str) -> bool {
    record.record_id == query
        || record.stable_id == query
        || record.artifact_locator == query
        || record.artifact_blake3 == query
        || record.artifact_blake3 == query.trim_start_matches(HASH_PREFIX)
        || contains_query(&record.source_ids, query)
        || contains_query(&record.requirement_ids, query)
        || contains_query(&record.lint_ids, query)
        || contains_query(&record.policy_ids, query)
        || contains_query(&record.fingerprints, query)
        || contains_query(&record.artifact_hashes, query)
        || contains_query(&record.provenance_source_ids, query)
        || contains_query(&record.parent_links, query)
}

fn linked_records<'a>(index: &'a EvidenceCodebaseIndex, record: &EvidenceIndexRecord) -> Vec<&'a EvidenceIndexRecord> {
    index
        .records
        .iter()
        .filter(|candidate| candidate.record_id != record.record_id)
        .filter(|candidate| {
            record.parent_links.iter().any(|link| link_matches_record(link, candidate))
                || candidate.parent_links.iter().any(|link| link_matches_record(link, record))
                || shared_identity(&record.policy_ids, &candidate.policy_ids)
                || shared_identity(&record.requirement_ids, &candidate.requirement_ids)
                || shared_identity(&record.lint_ids, &candidate.lint_ids)
                || cross_record_hash_link(record, candidate)
        })
        .collect()
}

fn link_matches_record(link: &str, record: &EvidenceIndexRecord) -> bool {
    link == record.artifact_locator || link == record.stable_id || link == record.record_id
}

fn shared_identity(left: &[String], right: &[String]) -> bool {
    left.iter().any(|value| right.contains(value))
}

fn cross_record_hash_link(left: &EvidenceIndexRecord, right: &EvidenceIndexRecord) -> bool {
    right.artifact_hashes.contains(&left.stable_id)
        || left.artifact_hashes.contains(&right.stable_id)
        || right.fingerprints.contains(&left.stable_id)
        || left.fingerprints.contains(&right.stable_id)
        || shared_identity(&left.artifact_hashes, &right.artifact_hashes)
        || shared_identity(&left.fingerprints, &right.fingerprints)
        || shared_identity(&left.artifact_hashes, &right.fingerprints)
        || shared_identity(&left.fingerprints, &right.artifact_hashes)
}

fn contains_query(values: &[String], query: &str) -> bool {
    values.iter().any(|value| value == query || value == query.trim_start_matches(HASH_PREFIX))
}
