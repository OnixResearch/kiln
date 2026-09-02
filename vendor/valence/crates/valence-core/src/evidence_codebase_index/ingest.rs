use iddqd::IdOrdMap;
use serde_json::Value;

use super::model::*;

const DUPLICATE_SOURCE_LOCATOR_CODE: &str = "source_locator.duplicate_conflict";

#[must_use]
pub fn build_evidence_codebase_index(
    inputs: &[EvidenceArtifactInput],
    mode: EvidenceIndexBuildMode,
) -> EvidenceCodebaseIndex {
    let sources = normalized_sources(inputs, mode);
    let source_index = source_identity_index(&sources);
    let mut builder = IndexBuilder::new(mode);

    for source in &source_index.duplicate_conflicts {
        builder.issue(
            source,
            DUPLICATE_SOURCE_LOCATOR_CODE,
            "duplicate normalized source locator has conflicting BLAKE3 identity",
        );
    }

    for source in &sources {
        ingest_source(source, &source_index, &mut builder);
    }

    builder.records.sort_by(|left, right| {
        left.artifact_family
            .cmp(&right.artifact_family)
            .then(left.stable_id.cmp(&right.stable_id))
            .then(left.artifact_locator.cmp(&right.artifact_locator))
            .then(left.record_id.cmp(&right.record_id))
    });
    builder.issues.sort_by(|left, right| {
        left.severity
            .cmp(&right.severity)
            .then(left.code.cmp(&right.code))
            .then(left.artifact_locator.cmp(&right.artifact_locator))
            .then(left.detail.cmp(&right.detail))
    });
    builder.issues.dedup();

    let record_count = builder.records.len();
    let mut index = EvidenceCodebaseIndex {
        schema_version: INDEX_SCHEMA_VERSION.to_owned(),
        normalization_version: NORMALIZATION_VERSION.to_owned(),
        index_hash: String::new(),
        boundary: INDEX_BOUNDARY.to_owned(),
        source_artifact_count: sources.len(),
        record_count,
        records: builder.records,
        issues: builder.issues,
    };
    index.index_hash = index_hash(&index);
    index
}
fn normalized_sources(inputs: &[EvidenceArtifactInput], mode: EvidenceIndexBuildMode) -> Vec<NormalizedSource> {
    let mut sources = inputs
        .iter()
        .map(|input| NormalizedSource {
            locator: normalize_locator(&input.locator),
            bytes: input.bytes.clone(),
            blake3: blake3_hex(&input.bytes),
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.locator.cmp(&right.locator).then(left.blake3.cmp(&right.blake3)));

    if mode == EvidenceIndexBuildMode::Advisory {
        sources.dedup_by(|left, right| left.locator == right.locator && left.blake3 == right.blake3);
    }
    sources
}

struct SourceIdentityIndex {
    by_locator: IdOrdMap<NormalizedSource>,
    duplicate_conflicts: Vec<NormalizedSource>,
}

impl SourceIdentityIndex {
    fn get(&self, locator: &str) -> Option<&NormalizedSource> {
        self.by_locator.get(locator)
    }
}

fn source_identity_index(sources: &[NormalizedSource]) -> SourceIdentityIndex {
    let mut by_locator = IdOrdMap::<NormalizedSource>::new();
    let mut duplicate_conflicts = Vec::new();
    for source in sources {
        if let Some(existing) = by_locator.get(source.locator.as_str()) {
            if existing.blake3 != source.blake3 {
                duplicate_conflicts.push(source.clone());
            }
            continue;
        }
        by_locator.insert_unique(source.clone()).expect("source locator was checked before unique insert");
    }
    SourceIdentityIndex {
        by_locator,
        duplicate_conflicts,
    }
}

fn ingest_source(source: &NormalizedSource, source_index: &SourceIdentityIndex, builder: &mut IndexBuilder) {
    if let Some(objects) = parse_jsonl_objects(source, builder) {
        if objects.iter().all(is_provenance_record) {
            ingest_provenance_records(source, &objects, source_index, builder);
            return;
        }
        if objects.iter().all(is_finding_record) {
            ingest_finding_records(source, &objects, builder);
            return;
        }
        builder.issue(source, UNSUPPORTED_FAMILY_CODE, "JSONL artifact has no supported Octet evidence family");
        return;
    }

    let text = String::from_utf8_lossy(&source.bytes);
    if let Ok(value) = serde_json::from_slice::<Value>(&source.bytes) {
        if is_proof_card_deck(&value) {
            ingest_proof_cards(source, &value, builder);
            return;
        }
        if is_fingerprint_review_manifest(&value) {
            ingest_fingerprint_review_manifest(source, &value, builder);
            return;
        }
        if is_replay_capsule(&value) {
            ingest_replay_capsule(source, &value, builder);
            return;
        }
        if is_transcript_report(&value) {
            ingest_transcript_report(source, &value, builder);
            return;
        }
        if is_evidence_log_manifest(&value) {
            ingest_evidence_log_manifest(source, &value, builder);
            return;
        }
        if is_evidence_rail_receipt(&value) {
            ingest_evidence_rail_receipt(source, &value, builder);
            return;
        }
        if is_single_finding_record(&value) || looks_like_finding_record(&value) {
            ingest_finding_records(source, &[value], builder);
            return;
        }
        if is_provenance_record(&value) {
            ingest_provenance_records(source, &[value], source_index, builder);
            return;
        }
        builder.issue(source, UNSUPPORTED_FAMILY_CODE, "JSON artifact has no supported Octet evidence family");
        return;
    }

    if text.contains(COMMAND_PREFIX) && text.contains(EXPECTED_OUTPUT_PREFIX) {
        ingest_transcript_markdown(source, &text, builder);
    } else if looks_like_json(&text) {
        builder.issue(source, MALFORMED_JSON_CODE, "artifact is not parseable JSON or JSONL");
    } else {
        builder.issue(source, UNSUPPORTED_FAMILY_CODE, "artifact has no supported Octet evidence family");
    }
}

fn parse_jsonl_objects(source: &NormalizedSource, builder: &mut IndexBuilder) -> Option<Vec<Value>> {
    let text = String::from_utf8_lossy(&source.bytes);
    let lines = text.lines().filter(|line| !line.trim().is_empty()).collect::<Vec<_>>();
    if lines.is_empty() || lines.len() == usize::from(looks_like_json(&text)) {
        return None;
    }

    let mut objects = Vec::new();
    for line in lines {
        match serde_json::from_str::<Value>(line) {
            Ok(Value::Object(_)) => objects.push(serde_json::from_str::<Value>(line).unwrap_or(Value::Null)),
            Ok(_) => {
                builder.issue(source, MALFORMED_JSON_CODE, "JSONL line is not an object");
                return Some(Vec::new());
            }
            Err(_) => return None,
        }
    }
    Some(objects)
}

fn ingest_provenance_records(
    source: &NormalizedSource,
    objects: &[Value],
    source_index: &SourceIdentityIndex,
    builder: &mut IndexBuilder,
) {
    for object in objects {
        let mut required = RequiredFields::new(source, builder, "provenance");
        let schema_version = required.string(object, "schema_version");
        let source_id = required.string(object, "source_id");
        let checker_id = required.string(object, "checker_id");
        let evidence_tier = required.string(object, "evidence_tier");
        let artifact_locator = required.string(object, "artifact_locator");
        let blake3 = required.string(object, "blake3");
        let Some(stable_id) = source_id.clone().or_else(|| artifact_locator.clone()) else {
            continue;
        };
        if schema_version.as_deref() != Some(PROVENANCE_SCHEMA_VERSION) {
            builder.issue(source, "provenance.schema_version", "unsupported or missing provenance schema_version");
        }
        if let (Some(locator), Some(record_hash)) = (&artifact_locator, &blake3) {
            let normalized_locator = normalize_locator(locator);
            if let Some(expected_source) = source_index.get(&normalized_locator) {
                if expected_source.blake3 != *record_hash {
                    builder.issue(
                        source,
                        "provenance.blake3_mismatch",
                        "provenance record blake3 does not match indexed artifact bytes",
                    );
                }
            } else {
                builder.caveat(
                    source,
                    "provenance.referenced_artifact_missing",
                    "referenced artifact bytes were not supplied to this index build",
                );
            }
        }

        let record = EvidenceIndexRecordBuilder::new("octet-provenance", &stable_id, source)
            .evidence_tier(evidence_tier.as_deref().unwrap_or(MANUAL_REVIEW_TIER))
            .source_id(source_id)
            .source_id(checker_id)
            .artifact_hash(blake3)
            .provenance_source_id(object_string(object, "source_id"))
            .parent_links(object_strings(object, "parent_artifacts"))
            .summary(format!("provenance for {}", artifact_locator.as_deref().unwrap_or("unknown artifact")))
            .build();
        builder.record(record);
    }
}

fn ingest_finding_records(source: &NormalizedSource, objects: &[Value], builder: &mut IndexBuilder) {
    for object in objects {
        let mut required = RequiredFields::new(source, builder, "finding");
        let stable_id = required.string(object, "stable_id");
        let lint = required.string(object, "lint");
        let Some(stable_id) = stable_id else {
            continue;
        };
        let summary = format!(
            "finding {} at {}",
            lint.as_deref().unwrap_or("unknown-lint"),
            object_string(object, "file").unwrap_or_else(|| "unknown source".to_owned())
        );
        let record = EvidenceIndexRecordBuilder::new("octet-finding", &stable_id, source)
            .evidence_tier(object_string(object, "evidence_tier").as_deref().unwrap_or(STATIC_LINT_TIER))
            .source_id(object_string(object, "file"))
            .lint_id(lint)
            .fingerprint(object_string(object, "semantic_id"))
            .fingerprint(object_string(object, "normalized_snippet_hash"))
            .fingerprint(object_string(object, "config_hash"))
            .requirement_ids(strings_with_requirement_ids(object))
            .summary(summary)
            .build();
        builder.record(record);
    }
}

fn ingest_proof_cards(source: &NormalizedSource, value: &Value, builder: &mut IndexBuilder) {
    let Some(cards) = value.get("cards").and_then(Value::as_array) else {
        builder.issue(source, "proof_cards.cards", "proof card deck missing cards array");
        return;
    };
    if value.get("schema_version").and_then(Value::as_str) != Some(PROOF_CARDS_SCHEMA_VERSION) {
        builder.issue(source, "proof_cards.schema_version", "unsupported proof card deck schema_version");
    }
    for card in cards {
        let mut required = RequiredFields::new(source, builder, "proof_card");
        let proof_id = required.string(card, "proof_or_property_id");
        let policy_id = required.string(card, "policy_id");
        let evidence_tier = required.string(card, "evidence_tier");
        let Some(proof_id) = proof_id else {
            continue;
        };
        let artifact_hashes = card
            .get("artifact_blake3_identities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|identity| object_string(identity, "blake3"))
            .collect::<Vec<_>>();
        let parent_links = object_string(card, "receipt_path")
            .into_iter()
            .chain(object_string(card, "provenance_artifact"))
            .chain(
                card.get("artifact_blake3_identities")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|identity| object_string(identity, "artifact")),
            )
            .collect::<Vec<_>>();
        let record = EvidenceIndexRecordBuilder::new("octet-proof-card", &proof_id, source)
            .evidence_tier(evidence_tier.as_deref().unwrap_or(MANUAL_REVIEW_TIER))
            .source_id(object_string(card, "checked_rust_source_scope"))
            .requirement_id(object_string(card, "source_requirement_or_policy_id"))
            .policy_id(policy_id)
            .artifact_hashes(artifact_hashes)
            .replay_command(object_string(card, "artifact_verifier_command"))
            .parent_links(parent_links)
            .caveats(object_strings(card, "non_claims"))
            .summary("proof card linkage".to_owned())
            .build();
        builder.record(record);
    }
}

fn ingest_fingerprint_review_manifest(source: &NormalizedSource, value: &Value, builder: &mut IndexBuilder) {
    let Some(records) = value.get("records").and_then(Value::as_array) else {
        builder.issue(source, "fingerprint.records", "fingerprint review manifest missing records array");
        return;
    };
    for item in records {
        let mut required = RequiredFields::new(source, builder, "fingerprint");
        let stable_id = required.string(item, "stable_id");
        let Some(stable_id) = stable_id else {
            continue;
        };
        let mut caveats = Vec::new();
        if item.get("requires_review").and_then(Value::as_bool).unwrap_or(false) {
            caveats.push("requires_review".to_owned());
        }
        if item.get("macro_warning").and_then(Value::as_bool).unwrap_or(false) {
            caveats.push("macro_warning".to_owned());
        }
        let source_label = match (object_string(item, "path"), object_string(item, "name")) {
            (Some(path), Some(name)) => Some(format!("{path}::{name}")),
            (Some(path), None) => Some(path),
            (None, Some(name)) => Some(name),
            (None, None) => None,
        };
        let record = EvidenceIndexRecordBuilder::new("octet-fingerprint-review", &stable_id, source)
            .evidence_tier(MANUAL_REVIEW_TIER)
            .source_id(source_label)
            .fingerprint(object_string(item, "computed_hash"))
            .fingerprint(object_string(item, "normalized_body_hash"))
            .fingerprint(object_string(item, "semantic_id"))
            .fingerprint(object_string(item, "dependency_set_hash"))
            .requirement_ids(strings_with_requirement_ids(item))
            .caveats(caveats)
            .summary("function fingerprint review record".to_owned())
            .build();
        builder.record(record);
    }
}

fn ingest_replay_capsule(source: &NormalizedSource, value: &Value, builder: &mut IndexBuilder) {
    let mut required = RequiredFields::new(source, builder, "replay_capsule");
    let capsule_hash = required.string(value, "capsule_hash");
    let Some(capsule_hash) = capsule_hash else {
        return;
    };
    let manifest = value.get("manifest").unwrap_or(&Value::Null);
    let replay_command =
        manifest.get("command_argv").and_then(Value::as_array).map(|items| command_from_json_array(items));
    let artifact_hashes = hash_values_from(manifest.get("input_hashes"))
        .into_iter()
        .chain(hash_values_from(manifest.get("output_hashes")))
        .collect::<Vec<_>>();
    let record = EvidenceIndexRecordBuilder::new("octet-replay-capsule", &capsule_hash, source)
        .evidence_tier(TRANSCRIPT_ASSERTION_TIER)
        .source_id(object_string(manifest, "run_kind"))
        .fingerprint(object_string(manifest, "config_hash"))
        .artifact_hashes(artifact_hashes)
        .replay_command(replay_command)
        .caveat(Some("replay identity only; not distributed execution correctness".to_owned()))
        .summary("replay capsule".to_owned())
        .build();
    builder.record(record);
}

fn ingest_transcript_report(source: &NormalizedSource, value: &Value, builder: &mut IndexBuilder) {
    let mut required = RequiredFields::new(source, builder, "transcript");
    let transcript_hash = required.string(value, "transcript_hash");
    let Some(transcript_hash) = transcript_hash else {
        return;
    };
    let record = EvidenceIndexRecordBuilder::new("octet-transcript-report", &transcript_hash, source)
        .evidence_tier(TRANSCRIPT_ASSERTION_TIER)
        .source_id(object_string(value, "path"))
        .provenance_source_id(value.get("provenance").and_then(|item| object_string(item, "source_id")))
        .parent_link(value.get("provenance").and_then(|item| object_string(item, "artifact_locator")))
        .artifact_hash(value.get("provenance").and_then(|item| object_string(item, "blake3")))
        .summary("transcript assertion report".to_owned())
        .build();
    builder.record(record);
}

fn ingest_transcript_markdown(source: &NormalizedSource, text: &str, builder: &mut IndexBuilder) {
    let commands = text
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix(COMMAND_PREFIX).map(str::to_owned))
        .collect::<Vec<_>>();
    let stable_id = prefixed_hash(text.as_bytes());
    let record = EvidenceIndexRecordBuilder::new("octet-transcript-markdown", &stable_id, source)
        .evidence_tier(TRANSCRIPT_ASSERTION_TIER)
        .source_id(Some(source.locator.clone()))
        .replay_commands(commands)
        .summary("markdown transcript assertions".to_owned())
        .build();
    builder.record(record);
}

fn ingest_evidence_log_manifest(source: &NormalizedSource, value: &Value, builder: &mut IndexBuilder) {
    let Some(logs) = value.get("logs").and_then(Value::as_array) else {
        builder.issue(source, "evidence_log.logs", "evidence log manifest missing logs array");
        return;
    };
    for log in logs {
        let mut required = RequiredFields::new(source, builder, "evidence_log");
        let locator = required.string(log, "artifact_locator");
        let blake3 = required.string(log, "blake3");
        let Some(locator) = locator else {
            continue;
        };
        let record = EvidenceIndexRecordBuilder::new("octet-evidence-log", &locator, source)
            .evidence_tier(TRANSCRIPT_ASSERTION_TIER)
            .source_id(object_string(log, "capture_source"))
            .source_id(object_string(log, "checked_scope"))
            .artifact_hash(blake3)
            .replay_command(object_string(log, "replay_command"))
            .summary("bounded evidence log".to_owned())
            .build();
        builder.record(record);
    }
}

fn ingest_evidence_rail_receipt(source: &NormalizedSource, value: &Value, builder: &mut IndexBuilder) {
    let mut required = RequiredFields::new(source, builder, "evidence_rail");
    let rail_id = required.string(value, "rail_id");
    let checker_id = required.string(value, "checker_id");
    let Some(rail_id) = rail_id else {
        return;
    };
    let artifact_hashes = [
        object_string(value, "bounded_log_identity"),
        object_string(value, "imported_input_identity"),
        object_string(value, "baseline_identity"),
        object_string(value, "lockfile_identity"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let record = EvidenceIndexRecordBuilder::new("octet-evidence-rail-receipt", &rail_id, source)
        .evidence_tier(POLICY_CHECK_TIER)
        .source_id(checker_id)
        .policy_id(Some(rail_id.clone()))
        .artifact_hashes(artifact_hashes)
        .replay_command(value.get("command_argv").and_then(Value::as_array).map(|items| command_from_json_array(items)))
        .parent_links(object_strings(value, "parent_artifacts"))
        .caveats(object_strings(value, "non_claims"))
        .summary("external evidence rail receipt".to_owned())
        .build();
    builder.record(record);

    for finding in value.get("findings").and_then(Value::as_array).into_iter().flatten() {
        if let Some(family) = object_string(finding, "family") {
            let finding_id = format!("{rail_id}#{family}");
            let finding_record = EvidenceIndexRecordBuilder::new("octet-evidence-rail-finding", &finding_id, source)
                .evidence_tier(POLICY_CHECK_TIER)
                .policy_id(Some(rail_id.clone()))
                .lint_id(Some(family.clone()))
                .source_id(object_string(finding, "file"))
                .parent_link(Some(rail_id.clone()))
                .summary(format!("evidence rail finding {family}"))
                .build();
            builder.record(finding_record);
        }
    }
}

fn is_provenance_record(value: &Value) -> bool {
    value.get("schema_version").and_then(Value::as_str) == Some(PROVENANCE_SCHEMA_VERSION)
        || (value.get("source_id").is_some()
            && value.get("checker_id").is_some()
            && value.get("artifact_locator").is_some()
            && value.get("blake3").is_some())
}

fn is_finding_record(value: &Value) -> bool {
    value.get("stable_id").is_some() && value.get("lint").is_some() && value.get("evidence_tier").is_some()
}

fn is_single_finding_record(value: &Value) -> bool {
    value.get("stable_id").is_some() && value.get("lint").is_some()
}

fn looks_like_finding_record(value: &Value) -> bool {
    value.get("lint").is_some() && value.get("evidence_tier").is_some()
}

fn is_proof_card_deck(value: &Value) -> bool {
    value.get("schema_version").and_then(Value::as_str) == Some(PROOF_CARDS_SCHEMA_VERSION)
        || value.get("cards").is_some()
}

fn is_fingerprint_review_manifest(value: &Value) -> bool {
    value.get("manifest_hash").is_some()
        && value.get("records").is_some()
        && value.get("tool_name").and_then(Value::as_str) == Some("cargo-octet")
}

fn is_replay_capsule(value: &Value) -> bool {
    value.get("capsule_kind").and_then(Value::as_str) == Some(REPLAY_CAPSULE_KIND)
        || value.get("capsule_hash").is_some() && value.get("manifest").is_some()
}

fn is_transcript_report(value: &Value) -> bool {
    value.get("transcript_hash").is_some() && value.get("command_blocks").is_some()
}

fn is_evidence_log_manifest(value: &Value) -> bool {
    value.get("schema_version").and_then(Value::as_str) == Some(EVIDENCE_LOG_MANIFEST_SCHEMA_VERSION)
        || value.get("logs").is_some()
}

fn is_evidence_rail_receipt(value: &Value) -> bool {
    value.get("schema_version").and_then(Value::as_str) == Some(EVIDENCE_RAIL_RECEIPT_SCHEMA_VERSION)
        || value.get("rail_id").is_some() && value.get("bounded_log_identity").is_some()
}
