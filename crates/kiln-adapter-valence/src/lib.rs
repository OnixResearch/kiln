use std::collections::{BTreeMap, BTreeSet};

use kiln_core::types::{ContentId, CoreResult, Diagnostic, ExternalRef};
use serde::{Deserialize, Serialize};
use serde_json::json;
use valence_core::evidence_ir::{
    EVIDENCE_IR_SCHEMA, EvidenceArtifact, EvidenceSpan, REQUIRED_NON_CLAIM, VerificationRole,
    build_evidence_artifact, check_evidence_artifact,
};
use valence_core::evidence_model::RECEIPT_TRANSCRIPT_SCHEMA;
use valence_core::stack_provenance_adapter::STACK_PROVENANCE_SIDECAR_SCHEMA;

pub const VALENCE_SOURCE_REVISION: &str = "220307a371e7bc4e3309447b09447d0fe6c4db5f";
pub const ARTIFACT_AUTH_SOURCE_REVISION: &str = "e41340bec587b6d049b5cc518ec7db925dde84be";
pub const VALENCE_CORE_LICENSE: &str = "MPL-2.0";
pub const KILN_ARTIFACT_KIND: &str = "kiln-ci-observation-v1";
pub const ROLE_REGISTRY_PIN: &str =
    "valence-core::stack_role_registry@220307a371e7bc4e3309447b09447d0fe6c4db5f";
pub const SPAN_REGISTRY_PIN: &str =
    "valence-core::evidence_ir::EvidenceSpan@220307a371e7bc4e3309447b09447d0fe6c4db5f";
pub const KILN_NON_CLAIM: &str = "Kiln evidence records identity and linkage only; it does not establish CI correctness or release eligibility";
pub const PROVIDER_NON_CLAIM: &str =
    "provider observations remain bounded external facts and do not prove provider correctness";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Candidate,
    Pipeline,
    Run,
    Attempt,
    Product,
    Event,
    Receipt,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRole {
    Producer,
    Checker,
    Builder,
    Signer,
    Provider,
    Worker,
    Linker,
    Consumer,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    Source,
    Receipt,
    Event,
}

// r[impl kiln.valence.profile]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValenceProfile {
    pub source_revision: String,
    pub artifact_auth_revision: String,
    pub evidence_ir_schema: String,
    pub sidecar_schema: String,
    pub receipt_schema: String,
    pub role_registry: String,
    pub span_registry: String,
    pub license: String,
    pub subject_kinds: BTreeSet<SubjectKind>,
    pub identity_roles: BTreeSet<IdentityRole>,
    pub span_kinds: BTreeSet<SpanKind>,
    pub required_non_claims: BTreeSet<String>,
    pub max_artifact_bytes: usize,
    pub max_facts: usize,
    pub max_roles: usize,
    pub max_text_bytes: usize,
}

pub fn admit_profile(profile: &ValenceProfile) -> CoreResult<()> {
    if profile.source_revision != VALENCE_SOURCE_REVISION
        || profile.artifact_auth_revision != ARTIFACT_AUTH_SOURCE_REVISION
        || profile.evidence_ir_schema != EVIDENCE_IR_SCHEMA
        || profile.sidecar_schema != STACK_PROVENANCE_SIDECAR_SCHEMA
        || profile.receipt_schema != RECEIPT_TRANSCRIPT_SCHEMA
        || profile.role_registry != ROLE_REGISTRY_PIN
        || profile.span_registry != SPAN_REGISTRY_PIN
        || profile.license != VALENCE_CORE_LICENSE
    {
        return Err(Diagnostic::new(
            "valence_profile_drift",
            "valence.profile",
            "Valence revision, schema, registry, receipt, or license differs from the pinned cohort",
        ));
    }
    if profile.subject_kinds.is_empty()
        || profile.identity_roles.is_empty()
        || profile.span_kinds.is_empty()
        || profile.max_artifact_bytes == 0
        || profile.max_facts == 0
        || profile.max_roles == 0
        || profile.max_text_bytes == 0
    {
        return Err(Diagnostic::new(
            "valence_profile_bound",
            "valence.profile",
            "Valence profile registry or bounds are empty",
        ));
    }
    for required in [REQUIRED_NON_CLAIM, KILN_NON_CLAIM, PROVIDER_NON_CLAIM] {
        if !profile.required_non_claims.contains(required) {
            return Err(Diagnostic::new(
                "valence_nonclaim_profile",
                "valence.required_non_claims",
                "Valence profile omits a required non-claim",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubjectIdentity {
    pub kind: SubjectKind,
    pub kiln_id: String,
    pub content_identity: Option<ContentId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RoleBinding {
    pub role: IdentityRole,
    pub identity: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpanFact {
    pub kind: SpanKind,
    pub source_id: String,
    pub source_path: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub label: Option<String>,
}

#[derive(Debug)]
pub struct EvidenceProjectionInput<'a> {
    pub subject: &'a SubjectIdentity,
    pub facts: &'a BTreeMap<String, String>,
    pub roles: &'a [RoleBinding],
    pub span: Option<&'a SpanFact>,
    pub non_claims: &'a BTreeSet<String>,
}

// r[impl kiln.valence.projection]
pub fn project_evidence(
    profile: &ValenceProfile,
    input: &EvidenceProjectionInput<'_>,
) -> CoreResult<EvidenceArtifact> {
    admit_profile(profile)?;
    admit_subject(profile, input.subject)?;
    admit_facts(profile, input.facts)?;
    admit_roles(profile, input.roles)?;
    admit_non_claims(profile, input.non_claims)?;
    let span = input
        .span
        .map(|value| convert_span(profile, value))
        .transpose()?;
    let payload = json!({
        "subject": input.subject,
        "facts": input.facts,
        "roles": input.roles,
        "role_registry": profile.role_registry,
        "span_registry": profile.span_registry,
        "sidecar_schema": profile.sidecar_schema,
        "receipt_schema": profile.receipt_schema,
        "algorithms": ["BLAKE3"],
    });
    let stable_id = subject_stable_id(input.subject);
    let artifact = build_evidence_artifact(
        KILN_ARTIFACT_KIND,
        &stable_id,
        span,
        VerificationRole::RecordedOnly,
        payload,
        input.non_claims.iter().cloned().collect(),
    )
    .map_err(|_| {
        Diagnostic::new(
            "valence_projection",
            "valence.artifact",
            "Valence Evidence IR construction rejected the projected values",
        )
    })?;
    let encoded = serde_json::to_string(&artifact).map_err(|_| {
        Diagnostic::new(
            "valence_encoding",
            "valence.artifact",
            "Valence Evidence IR encoding failed",
        )
    })?;
    if encoded.len() > profile.max_artifact_bytes {
        return Err(Diagnostic::new(
            "valence_artifact_bound",
            "valence.artifact",
            "Valence Evidence IR exceeds its named byte bound",
        ));
    }
    let checked = check_evidence_artifact(&encoded);
    if !checked.valid {
        return Err(Diagnostic::new(
            "valence_projection_invalid",
            "valence.artifact",
            "Valence rejected the projected Evidence IR",
        ));
    }
    Ok(artifact)
}

fn admit_subject(profile: &ValenceProfile, subject: &SubjectIdentity) -> CoreResult<()> {
    if !profile.subject_kinds.contains(&subject.kind) {
        return Err(Diagnostic::new(
            "valence_subject_kind",
            "valence.subject.kind",
            "Valence subject kind is outside the admitted registry",
        ));
    }
    admit_text(
        &subject.kiln_id,
        profile.max_text_bytes,
        "valence.subject.kiln_id",
    )
}

fn admit_facts(profile: &ValenceProfile, facts: &BTreeMap<String, String>) -> CoreResult<()> {
    if facts.len() > profile.max_facts {
        return Err(Diagnostic::new(
            "valence_fact_bound",
            "valence.facts",
            "Valence fact count exceeds its named bound",
        ));
    }
    for (key, value) in facts {
        admit_text(key, profile.max_text_bytes, "valence.facts.key")?;
        admit_text(value, profile.max_text_bytes, "valence.facts.value")?;
    }
    Ok(())
}

fn admit_roles(profile: &ValenceProfile, roles: &[RoleBinding]) -> CoreResult<()> {
    if roles.is_empty() || roles.len() > profile.max_roles {
        return Err(Diagnostic::new(
            "valence_role_bound",
            "valence.roles",
            "Valence roles are empty or exceed their named bound",
        ));
    }
    let mut observed = BTreeSet::new();
    for binding in roles {
        if !profile.identity_roles.contains(&binding.role) || !observed.insert(binding.role) {
            return Err(Diagnostic::new(
                "valence_role_registry",
                "valence.roles",
                "Valence role is unsupported or duplicated",
            ));
        }
        binding.identity.validate()?;
        if matches!(
            binding.role,
            IdentityRole::Builder | IdentityRole::Checker | IdentityRole::Signer
        ) && binding.identity.provider_namespace.as_str() == "kiln"
        {
            return Err(Diagnostic::new(
                "valence_role_promotion",
                "valence.roles",
                "Kiln cannot promote itself into a builder, checker, or signer role",
            ));
        }
    }
    Ok(())
}

fn admit_non_claims(profile: &ValenceProfile, non_claims: &BTreeSet<String>) -> CoreResult<()> {
    if !profile.required_non_claims.is_subset(non_claims) {
        return Err(Diagnostic::new(
            "valence_nonclaim_missing",
            "valence.non_claims",
            "Valence projection omits a required non-claim",
        ));
    }
    Ok(())
}

fn convert_span(profile: &ValenceProfile, span: &SpanFact) -> CoreResult<EvidenceSpan> {
    if !profile.span_kinds.contains(&span.kind) {
        return Err(Diagnostic::new(
            "valence_span_kind",
            "valence.span.kind",
            "Valence span kind is outside the admitted registry",
        ));
    }
    admit_text(
        &span.source_id,
        profile.max_text_bytes,
        "valence.span.source_id",
    )?;
    admit_text(
        &span.source_path,
        profile.max_text_bytes,
        "valence.span.source_path",
    )?;
    Ok(EvidenceSpan {
        source_id: span.source_id.clone(),
        source_path: span.source_path.clone(),
        start_byte: span.start_byte,
        end_byte: span.end_byte,
        start_line: span.start_line,
        start_column: span.start_column,
        end_line: span.end_line,
        end_column: span.end_column,
        label: span.label.clone(),
    })
}

fn admit_text(value: &str, max_bytes: usize, field: &'static str) -> CoreResult<()> {
    if value.is_empty() || value.len() > max_bytes || value.trim() != value {
        return Err(Diagnostic::new(
            "valence_text_bound",
            field,
            "Valence text is empty, padded, or exceeds its named byte bound",
        ));
    }
    Ok(())
}

fn subject_stable_id(subject: &SubjectIdentity) -> String {
    let kind = match subject.kind {
        SubjectKind::Candidate => "candidate",
        SubjectKind::Pipeline => "pipeline",
        SubjectKind::Run => "run",
        SubjectKind::Attempt => "attempt",
        SubjectKind::Product => "product",
        SubjectKind::Event => "event",
        SubjectKind::Receipt => "receipt",
    };
    format!("kiln:{kind}:{}", subject.kiln_id)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportObservation {
    pub subject: String,
    pub canonical_hash: String,
    pub verification_role: String,
    pub applicable: bool,
    pub state_authority: bool,
}

// r[impl kiln.valence.import]
pub fn import_evidence(
    profile: &ValenceProfile,
    bytes: &[u8],
    expected_subject: &SubjectIdentity,
) -> CoreResult<ImportObservation> {
    admit_profile(profile)?;
    if bytes.len() > profile.max_artifact_bytes {
        return Err(Diagnostic::new(
            "valence_artifact_bound",
            "valence.artifact",
            "Valence Evidence IR exceeds its named byte bound",
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        Diagnostic::new(
            "valence_encoding",
            "valence.artifact",
            "Valence Evidence IR must use UTF-8 JSON",
        )
    })?;
    let checked = check_evidence_artifact(text);
    if !checked.valid {
        return Err(Diagnostic::new(
            "valence_import_invalid",
            "valence.artifact",
            "Valence Evidence IR validation failed",
        ));
    }
    let artifact: EvidenceArtifact = serde_json::from_str(text).map_err(|_| {
        Diagnostic::new(
            "valence_import_decode",
            "valence.artifact",
            "Valence Evidence IR decoding failed",
        )
    })?;
    let expected_stable_id = subject_stable_id(expected_subject);
    if artifact.stable_id != expected_stable_id {
        return Err(Diagnostic::new(
            "valence_subject_stale",
            "valence.stable_id",
            "Valence Evidence IR names a different or stale Kiln subject",
        ));
    }
    if artifact.verification_role != VerificationRole::RecordedOnly {
        return Err(Diagnostic::new(
            "valence_import_role",
            "valence.verification_role",
            "Kiln imports Valence linkage only as recorded-only evidence",
        ));
    }
    Ok(ImportObservation {
        subject: artifact.stable_id,
        canonical_hash: artifact.canonical_hash,
        verification_role: artifact.verification_role.as_str().to_string(),
        applicable: true,
        state_authority: false,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValenceTransportProfile {
    pub endpoint_profile: ExternalRef,
    pub max_response_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptObservation {
    pub receipt_schema: String,
    pub artifact_hash: String,
    pub accepted: bool,
    pub external_reference: Option<ExternalRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValenceTransportError {
    pub message: String,
}

pub trait ValenceTransport {
    fn publish(
        &mut self,
        endpoint_profile: &ExternalRef,
        bytes: &[u8],
        max_response_bytes: usize,
    ) -> Result<ReceiptObservation, ValenceTransportError>;
}

pub fn publish_evidence<T: ValenceTransport>(
    profile: &ValenceProfile,
    transport_profile: &ValenceTransportProfile,
    artifact: &EvidenceArtifact,
    transport: &mut T,
) -> CoreResult<ReceiptObservation> {
    admit_profile(profile)?;
    transport_profile.endpoint_profile.validate()?;
    if transport_profile.max_response_bytes == 0 {
        return Err(Diagnostic::new(
            "valence_transport_bound",
            "valence.transport.max_response_bytes",
            "Valence transport response bound must be positive",
        ));
    }
    let bytes = serde_json::to_vec(artifact).map_err(|_| {
        Diagnostic::new(
            "valence_encoding",
            "valence.artifact",
            "Valence Evidence IR encoding failed",
        )
    })?;
    if bytes.len() > profile.max_artifact_bytes {
        return Err(Diagnostic::new(
            "valence_artifact_bound",
            "valence.artifact",
            "Valence Evidence IR exceeds its named byte bound",
        ));
    }
    let receipt = transport
        .publish(
            &transport_profile.endpoint_profile,
            &bytes,
            transport_profile.max_response_bytes,
        )
        .map_err(|_| {
            Diagnostic::new(
                "valence_transport_unavailable",
                "valence.transport",
                "Valence transport is unavailable",
            )
        })?;
    if receipt.receipt_schema != profile.receipt_schema
        || receipt.artifact_hash != artifact.canonical_hash
    {
        return Err(Diagnostic::new(
            "valence_receipt_link",
            "valence.receipt",
            "Valence receipt does not match the pinned schema or artifact",
        ));
    }
    Ok(receipt)
}

// r[impl kiln.valence.conformance]
// r[impl kiln.valence.nonclaims]
#[cfg(test)]
mod tests {
    use kiln_core::domain::State;

    use super::*;

    const MAX_ARTIFACT_BYTES: usize = 32_768;
    const MAX_FACTS: usize = 16;
    const MAX_ROLES: usize = 8;
    const MAX_TEXT_BYTES: usize = 256;
    const MAX_RESPONSE_BYTES: usize = 4096;
    const END_BYTE: u64 = 8;
    const END_COLUMN: u32 = 8;

    fn content(value: char) -> ContentId {
        ContentId::from_blake3_hex(value.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }

    fn profile() -> ValenceProfile {
        ValenceProfile {
            source_revision: VALENCE_SOURCE_REVISION.to_string(),
            artifact_auth_revision: ARTIFACT_AUTH_SOURCE_REVISION.to_string(),
            evidence_ir_schema: EVIDENCE_IR_SCHEMA.to_string(),
            sidecar_schema: STACK_PROVENANCE_SIDECAR_SCHEMA.to_string(),
            receipt_schema: RECEIPT_TRANSCRIPT_SCHEMA.to_string(),
            role_registry: ROLE_REGISTRY_PIN.to_string(),
            span_registry: SPAN_REGISTRY_PIN.to_string(),
            license: VALENCE_CORE_LICENSE.to_string(),
            subject_kinds: BTreeSet::from([
                SubjectKind::Candidate,
                SubjectKind::Pipeline,
                SubjectKind::Run,
                SubjectKind::Attempt,
                SubjectKind::Product,
                SubjectKind::Event,
                SubjectKind::Receipt,
            ]),
            identity_roles: BTreeSet::from([
                IdentityRole::Producer,
                IdentityRole::Checker,
                IdentityRole::Builder,
                IdentityRole::Signer,
                IdentityRole::Provider,
                IdentityRole::Worker,
                IdentityRole::Linker,
                IdentityRole::Consumer,
            ]),
            span_kinds: BTreeSet::from([SpanKind::Source, SpanKind::Receipt, SpanKind::Event]),
            required_non_claims: required_non_claims(),
            max_artifact_bytes: MAX_ARTIFACT_BYTES,
            max_facts: MAX_FACTS,
            max_roles: MAX_ROLES,
            max_text_bytes: MAX_TEXT_BYTES,
        }
    }

    fn required_non_claims() -> BTreeSet<String> {
        [REQUIRED_NON_CLAIM, KILN_NON_CLAIM, PROVIDER_NON_CLAIM]
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    fn subject(id: &str) -> SubjectIdentity {
        SubjectIdentity {
            kind: SubjectKind::Attempt,
            kiln_id: id.to_string(),
            content_identity: Some(content('a')),
        }
    }

    fn span() -> SpanFact {
        SpanFact {
            kind: SpanKind::Event,
            source_id: "kiln:event:attempt-a".to_string(),
            source_path: "journal/events".to_string(),
            start_byte: 0,
            end_byte: END_BYTE,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: END_COLUMN,
            label: Some("attempt-completed".to_string()),
        }
    }

    fn roles() -> Vec<RoleBinding> {
        vec![
            RoleBinding {
                role: IdentityRole::Producer,
                identity: ExternalRef::new("kiln", "control-plane", "kiln")
                    .unwrap_or_else(|_| unreachable!()),
            },
            RoleBinding {
                role: IdentityRole::Builder,
                identity: ExternalRef::new("mantle", "builder", "builder-a")
                    .unwrap_or_else(|_| unreachable!()),
            },
            RoleBinding {
                role: IdentityRole::Provider,
                identity: ExternalRef::new("provider", "build", "provider-a")
                    .unwrap_or_else(|_| unreachable!()),
            },
        ]
    }

    fn artifact(subject: &SubjectIdentity) -> CoreResult<EvidenceArtifact> {
        let facts = BTreeMap::from([
            ("outcome".to_string(), "success".to_string()),
            ("provider_observation".to_string(), "accepted".to_string()),
        ]);
        let roles = roles();
        let span = span();
        let non_claims = required_non_claims();
        project_evidence(
            &profile(),
            &EvidenceProjectionInput {
                subject,
                facts: &facts,
                roles: &roles,
                span: Some(&span),
                non_claims: &non_claims,
            },
        )
    }

    // r[verify kiln.valence.profile.scenario.match]
    // r[verify kiln.valence.projection.scenario.attempt]
    #[test]
    fn attempt_projection_is_valid_recorded_only_evidence() {
        let artifact = artifact(&subject("attempt-a")).unwrap_or_else(|_| unreachable!());
        let json = serde_json::to_string(&artifact).unwrap_or_else(|_| unreachable!());
        assert!(check_evidence_artifact(&json).valid);
        assert_eq!(artifact.verification_role, VerificationRole::RecordedOnly);
    }

    // r[verify kiln.valence.profile.scenario.role]
    // r[verify kiln.valence.projection.scenario.builder]
    #[test]
    fn profile_drift_and_kiln_builder_promotion_fail_closed() {
        let mut drift = profile();
        drift.source_revision = "unsupported".to_string();
        assert!(admit_profile(&drift).is_err());

        let subject = subject("attempt-a");
        let facts = BTreeMap::new();
        let roles = [RoleBinding {
            role: IdentityRole::Builder,
            identity: ExternalRef::new("kiln", "builder", "kiln")
                .unwrap_or_else(|_| unreachable!()),
        }];
        let non_claims = required_non_claims();
        assert!(
            project_evidence(
                &profile(),
                &EvidenceProjectionInput {
                    subject: &subject,
                    facts: &facts,
                    roles: &roles,
                    span: None,
                    non_claims: &non_claims,
                },
            )
            .is_err()
        );
    }

    // r[verify kiln.valence.import.scenario.success]
    #[test]
    fn imported_success_does_not_change_canonical_state() {
        let before = State::default();
        let subject = subject("attempt-a");
        let artifact = artifact(&subject).unwrap_or_else(|_| unreachable!());
        let bytes = serde_json::to_vec(&artifact).unwrap_or_else(|_| unreachable!());
        let observation = import_evidence(&profile(), &bytes, &subject);
        assert_eq!(observation.map(|value| value.state_authority), Ok(false));
        assert_eq!(before, State::default());
    }

    // r[verify kiln.valence.import.scenario.stale]
    #[test]
    fn stale_subject_is_rejected() {
        let artifact = artifact(&subject("attempt-a")).unwrap_or_else(|_| unreachable!());
        let bytes = serde_json::to_vec(&artifact).unwrap_or_else(|_| unreachable!());
        assert!(import_evidence(&profile(), &bytes, &subject("attempt-b")).is_err());
    }

    // r[verify kiln.valence.conformance.scenario.nonclaim]
    #[test]
    fn missing_nonclaim_and_malformed_span_are_rejected() {
        let subject = subject("attempt-a");
        let facts = BTreeMap::new();
        let roles = roles();
        let mut non_claims = required_non_claims();
        non_claims.remove(KILN_NON_CLAIM);
        assert!(
            project_evidence(
                &profile(),
                &EvidenceProjectionInput {
                    subject: &subject,
                    facts: &facts,
                    roles: &roles,
                    span: None,
                    non_claims: &non_claims,
                },
            )
            .is_err()
        );
        let mut malformed_span = span();
        malformed_span.start_byte = END_BYTE;
        malformed_span.end_byte = 0;
        assert!(
            project_evidence(
                &profile(),
                &EvidenceProjectionInput {
                    subject: &subject,
                    facts: &facts,
                    roles: &roles,
                    span: Some(&malformed_span),
                    non_claims: &required_non_claims(),
                },
            )
            .is_err()
        );
    }

    // r[verify kiln.valence.conformance]
    #[test]
    fn pinned_producer_fixture_passes_independent_import() {
        let bytes = include_bytes!("../fixtures/kiln-attempt-evidence-v1.json");
        let observation = import_evidence(&profile(), bytes, &subject("fixture-attempt"));
        assert!(observation.is_ok());
        assert_eq!(
            observation.map(|value| value.verification_role),
            Ok("recorded_only".to_string())
        );
    }

    struct UnavailableTransport;

    impl ValenceTransport for UnavailableTransport {
        fn publish(
            &mut self,
            _endpoint_profile: &ExternalRef,
            _bytes: &[u8],
            _max_response_bytes: usize,
        ) -> Result<ReceiptObservation, ValenceTransportError> {
            Err(ValenceTransportError {
                message: "provider unavailable".to_string(),
            })
        }
    }

    // r[verify kiln.valence.nonclaims.scenario.scope]
    #[test]
    fn unavailable_provider_does_not_promote_or_mutate_state() {
        let state = State::default();
        let artifact = artifact(&subject("attempt-a")).unwrap_or_else(|_| unreachable!());
        let result = publish_evidence(
            &profile(),
            &ValenceTransportProfile {
                endpoint_profile: ExternalRef::new("valence", "endpoint", "local")
                    .unwrap_or_else(|_| unreachable!()),
                max_response_bytes: MAX_RESPONSE_BYTES,
            },
            &artifact,
            &mut UnavailableTransport,
        );
        assert!(result.is_err());
        assert_eq!(state, State::default());
    }
}
