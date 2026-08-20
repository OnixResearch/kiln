use std::collections::{BTreeMap, BTreeSet};

use kiln_adapter_valence::{
    ARTIFACT_AUTH_SOURCE_REVISION, EvidenceProjectionInput, IdentityRole, KILN_NON_CLAIM,
    PROVIDER_NON_CLAIM, ROLE_REGISTRY_PIN, RoleBinding, SPAN_REGISTRY_PIN, SpanFact, SpanKind,
    SubjectIdentity, SubjectKind, VALENCE_CORE_LICENSE, VALENCE_SOURCE_REVISION, ValenceProfile,
    project_evidence,
};
use kiln_core::types::{ContentId, ExternalRef};
use valence_core::evidence_ir::{EVIDENCE_IR_SCHEMA, REQUIRED_NON_CLAIM};
use valence_core::evidence_model::RECEIPT_TRANSCRIPT_SCHEMA;
use valence_core::stack_provenance_adapter::STACK_PROVENANCE_SIDECAR_SCHEMA;

const MAX_ARTIFACT_BYTES: usize = 32_768;
const MAX_FACTS: usize = 16;
const MAX_ROLES: usize = 8;
const MAX_TEXT_BYTES: usize = 256;
const END_BYTE: u64 = 8;
const END_COLUMN: u32 = 8;

fn main() -> Result<(), String> {
    let profile = ValenceProfile {
        source_revision: VALENCE_SOURCE_REVISION.to_string(),
        artifact_auth_revision: ARTIFACT_AUTH_SOURCE_REVISION.to_string(),
        evidence_ir_schema: EVIDENCE_IR_SCHEMA.to_string(),
        sidecar_schema: STACK_PROVENANCE_SIDECAR_SCHEMA.to_string(),
        receipt_schema: RECEIPT_TRANSCRIPT_SCHEMA.to_string(),
        role_registry: ROLE_REGISTRY_PIN.to_string(),
        span_registry: SPAN_REGISTRY_PIN.to_string(),
        license: VALENCE_CORE_LICENSE.to_string(),
        subject_kinds: BTreeSet::from([SubjectKind::Attempt]),
        identity_roles: BTreeSet::from([
            IdentityRole::Producer,
            IdentityRole::Builder,
            IdentityRole::Provider,
        ]),
        span_kinds: BTreeSet::from([SpanKind::Event]),
        required_non_claims: non_claims(),
        max_artifact_bytes: MAX_ARTIFACT_BYTES,
        max_facts: MAX_FACTS,
        max_roles: MAX_ROLES,
        max_text_bytes: MAX_TEXT_BYTES,
    };
    let subject = SubjectIdentity {
        kind: SubjectKind::Attempt,
        kiln_id: "fixture-attempt".to_string(),
        content_identity: Some(
            ContentId::from_blake3_hex("a".repeat(kiln_core::types::BLAKE3_HEX_BYTES))
                .map_err(|error| error.message)?,
        ),
    };
    let roles = vec![
        RoleBinding {
            role: IdentityRole::Producer,
            identity: ExternalRef::new("kiln", "control-plane", "kiln")
                .map_err(|error| error.message)?,
        },
        RoleBinding {
            role: IdentityRole::Builder,
            identity: ExternalRef::new("mantle", "builder", "fixture-builder")
                .map_err(|error| error.message)?,
        },
        RoleBinding {
            role: IdentityRole::Provider,
            identity: ExternalRef::new("provider", "build", "fixture-provider")
                .map_err(|error| error.message)?,
        },
    ];
    let facts = BTreeMap::from([("outcome".to_string(), "success".to_string())]);
    let span = SpanFact {
        kind: SpanKind::Event,
        source_id: "kiln:event:fixture-attempt".to_string(),
        source_path: "journal/events".to_string(),
        start_byte: 0,
        end_byte: END_BYTE,
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: END_COLUMN,
        label: Some("attempt-completed".to_string()),
    };
    let required = non_claims();
    let artifact = project_evidence(
        &profile,
        &EvidenceProjectionInput {
            subject: &subject,
            facts: &facts,
            roles: &roles,
            span: Some(&span),
            non_claims: &required,
        },
    )
    .map_err(|error| error.message)?;
    let output = serde_json::to_string_pretty(&artifact).map_err(|error| error.to_string())?;
    println!("{output}");
    Ok(())
}

fn non_claims() -> BTreeSet<String> {
    [REQUIRED_NON_CLAIM, KILN_NON_CLAIM, PROVIDER_NON_CLAIM]
        .into_iter()
        .map(str::to_string)
        .collect()
}
