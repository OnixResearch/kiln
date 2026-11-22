//! Cairn-owned bounded JSON translation of Valence's published sidecar contract.
//! Reviewed source: contracts/stack-provenance/contracts.ncl and
//! validate_stack_provenance_sidecar at VALENCE_REVISION. No upstream code is
//! copied. Admission is the intersection of the Nickel and Rust requirements;
//! it is not built-binary thread verification or executed-test coverage.
use super::{ProducerIssue, MAX_CHECKS, MAX_TEXT_BYTES};
use crate::canonical_json_hash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const VALENCE_REVISION: &str = "e40c76b4d2070a29636e00c85c0dff93f03dba2f";
const SCHEMA: &str = "valence.stack-provenance-sidecar.v1";
const SCOPE: &str = "identity-linkage-sidecar";
const CHAIN_SCHEMA: &str = "valence.provenance-chain.v1";
const STACK_BOUNDARY: &str = "stack provenance proves identity and linkage only; it does not prove behavioral correctness, semantic equivalence, whole-program safety, or requirement satisfaction";
const CHAIN_BOUNDARY: &str = "provenance chain proves identity and linkage only; it does not prove behavioral correctness, semantic equivalence, or that the binary satisfies the requirement";
const DIGEST_DIGITS: usize = 64;
const OVERCLAIMS: [&str; 8] = [
    "proves behavioral correctness",
    "proves semantic equivalence",
    "proves requirement satisfaction",
    "proves release eligibility",
    "proves whole-program safety",
    "certifies release eligibility",
    "guarantees behavioral correctness",
    "guarantees whole-program safety",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sidecar {
    schema: String,
    claim_scope: String,
    binary_hash: String,
    octet_export_artifact_dir: String,
    chain: Chain,
    parent_artifacts: Vec<Artifact>,
    #[serde(default)]
    diagnostics: Vec<String>,
    non_claims: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Chain {
    schema: String,
    links: Vec<Link>,
    boundary: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    source_id: String,
    function_object_id: String,
    requirement_ids: Vec<String>,
    artifact_hash: Option<String>,
    binary_hash: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    artifact: String,
    blake3: String,
}

#[derive(Debug, Serialize)]
pub struct ValenceAdmission {
    pub producer_revision: &'static str,
    pub native_schema: String,
    pub native_claim_scope: String,
    pub native_receipt_hash: String,
    pub binary_identity: String,
    pub source_identity_hashes: Vec<String>,
    pub parent_artifact_identities: Vec<String>,
    pub native_non_claims: Vec<String>,
    pub completion_eligible: bool,
}

// r[impl cairn.check_evidence_producer.adapters]
pub fn admit_valence_json(value: &Value) -> Result<ValenceAdmission, ProducerIssue> {
    let sidecar: Sidecar =
        serde_json::from_value(value.clone()).map_err(|_| ProducerIssue::Observation)?;
    if sidecar.schema != SCHEMA
        || sidecar.claim_scope != SCOPE
        || sidecar.chain.schema != CHAIN_SCHEMA
        || sidecar.chain.boundary != CHAIN_BOUNDARY
        || !digest(&sidecar.binary_hash)
        || !text(&sidecar.octet_export_artifact_dir)
        || !rows(&sidecar.chain.links)
        || !rows(&sidecar.parent_artifacts)
        || !rows(&sidecar.non_claims)
        || !sidecar.diagnostics.is_empty()
        || !sidecar
            .non_claims
            .iter()
            .any(|claim| claim == STACK_BOUNDARY)
        || sidecar.non_claims.iter().any(|claim| {
            !text(claim)
                || ![STACK_BOUNDARY, CHAIN_BOUNDARY].contains(&claim.as_str())
                || OVERCLAIMS.iter().any(|pattern| claim.contains(pattern))
        })
        || sidecar
            .parent_artifacts
            .iter()
            .any(|artifact| !text(&artifact.artifact) || !digest(&artifact.blake3))
    {
        return Err(ProducerIssue::Observation);
    }
    let mut sources = BTreeSet::new();
    for link in &sidecar.chain.links {
        if !text(&link.source_id) || !text(&link.function_object_id)
            || !sources.insert(&link.source_id) || !rows(&link.requirement_ids)
            || link.requirement_ids.iter().any(|id| !text(id))
            || link.artifact_hash.as_deref().is_some_and(|hash| !digest(hash))
            // Nickel requires exact text equality, stronger than Rust's digest
            // comparison. Preserve that published intersection, not a new alias.
            || link.binary_hash.as_deref() != Some(sidecar.binary_hash.as_str())
        {
            return Err(ProducerIssue::Observation);
        }
    }
    Ok(ValenceAdmission {
        producer_revision: VALENCE_REVISION,
        native_schema: sidecar.schema,
        native_claim_scope: sidecar.claim_scope,
        native_receipt_hash: canonical_json_hash(value),
        binary_identity: sidecar.binary_hash,
        source_identity_hashes: sidecar
            .chain
            .links
            .iter()
            .map(|link| canonical_json_hash(&Value::String(link.source_id.clone())))
            .collect(),
        parent_artifact_identities: sidecar
            .parent_artifacts
            .into_iter()
            .map(|artifact| artifact.blake3)
            .collect(),
        native_non_claims: sidecar.non_claims,
        completion_eligible: false,
    })
}
fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT_BYTES
}
fn rows<T>(values: &[T]) -> bool {
    !values.is_empty() && values.len() <= MAX_CHECKS
}
fn digest(value: &str) -> bool {
    let hex = value.strip_prefix("b3:").unwrap_or(value);
    hex.len() == DIGEST_DIGITS && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../fixtures/check-evidence/valence-sidecar.json"
        ))
        .expect("upstream exported fixture")
    }
    // r[verify cairn.check_evidence_producer.adapters]
    // r[verify cairn.check_evidence_producer.conformance]
    #[test]
    fn exported_native_sidecar_preserves_linkage_without_test_claims() {
        let value = fixture();
        let admitted = admit_valence_json(&value).expect("published sidecar");
        assert!(!admitted.completion_eligible);
        assert_eq!(admitted.native_claim_scope, SCOPE);
        assert_eq!(
            admitted.native_non_claims,
            vec![STACK_BOUNDARY, CHAIN_BOUNDARY]
        );
        assert_eq!(admitted.parent_artifact_identities.len(), 2);
        assert_eq!(admitted.native_receipt_hash, canonical_json_hash(&value));
    }
    #[test]
    fn substitutions_and_nonclaims_fail_closed() {
        for (pointer, replacement) in [
            ("/schema", json!("substituted")),
            ("/claim_scope", json!("executed-test")),
            ("/binary_hash", json!("2".repeat(DIGEST_DIGITS))),
            ("/chain/schema", json!("substituted")),
            ("/chain/boundary", json!("proves correctness")),
            ("/chain/links/0/binary_hash", Value::Null),
            ("/chain/links/0/artifact_hash", json!("not-a-digest")),
            ("/chain/links/0/source_id", json!(" ")),
            ("/chain/links/0/requirement_ids", json!([])),
            ("/parent_artifacts", json!([])),
            ("/parent_artifacts/0/blake3", json!("bad")),
            ("/non_claims", json!([])),
            (
                "/non_claims",
                json!([STACK_BOUNDARY, "proves release eligibility"]),
            ),
            ("/diagnostics", json!(["credential-secret"])),
            (
                "/octet_export_artifact_dir",
                json!("x".repeat(MAX_TEXT_BYTES + 1)),
            ),
        ] {
            let mut value = fixture();
            *value.pointer_mut(pointer).expect("fixture pointer") = replacement;
            assert!(admit_valence_json(&value).is_err(), "admitted {pointer}");
        }
        let mut duplicate = fixture();
        let link = duplicate["chain"]["links"][0].clone();
        duplicate["chain"]["links"]
            .as_array_mut()
            .expect("links")
            .push(link);
        assert!(admit_valence_json(&duplicate).is_err());
        let mut unknown = fixture();
        unknown["status"] = json!("passed");
        assert!(admit_valence_json(&unknown).is_err());
    }
}
