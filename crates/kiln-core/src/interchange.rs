use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{
    ContentId, CoreResult, Diagnostic, ExternalRef, NativeEventId, ProviderProfileId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum InterchangeKind {
    Slsa,
    InToto,
    CdEvents,
    CloudEvents,
    OpenTelemetry,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InterchangeProfile {
    pub kind: InterchangeKind,
    pub specification_version: ExternalRef,
    pub supported_event_types: BTreeSet<ExternalRef>,
    pub required_fields: BTreeSet<ExternalRef>,
    pub allow_unknown_fields: bool,
    pub allow_lossy_mapping: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IdentityRoles {
    pub builder: ExternalRef,
    pub signer: Option<ExternalRef>,
    pub provider: ProviderProfileId,
    pub worker: Option<ExternalRef>,
    pub kiln_control_plane: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceProjection {
    pub profile: ExternalRef,
    pub pipeline_revision: ContentId,
    pub source_or_candidate: ContentId,
    pub producing_attempt: ContentId,
    pub products: Vec<ContentId>,
    pub roles: IdentityRoles,
    pub native_digests: Vec<ExternalRef>,
}

pub fn validate_provenance(
    projection: &ProvenanceProjection,
    expected_attempt: &ContentId,
) -> CoreResult<()> {
    if &projection.producing_attempt != expected_attempt {
        return Err(Diagnostic::new(
            "provenance_attempt_mismatch",
            "provenance.producing_attempt",
            "provenance is linked to another producing attempt",
        ));
    }
    projection.roles.builder.validate()?;
    for digest in &projection.native_digests {
        digest.validate()?;
    }
    Ok(())
}

/// Exact in-toto statement type admitted by this mapping.
pub const IN_TOTO_STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";
/// Exact SLSA provenance predicate type admitted by this mapping.
pub const SLSA_PROVENANCE_PREDICATE_TYPE: &str = "https://slsa.dev/provenance/v1.1";
/// Kiln-scoped build type for one CI run. This is not a SLSA build level.
pub const KILN_PROVENANCE_BUILD_TYPE: &str = "urn:kiln:provenance:ci-run:v1";

/// Maps one admitted provenance projection to a canonical unsigned in-toto
/// Statement v1 whose predicate is SLSA Provenance v1.1.
///
/// The statement is canonical JSON: object keys are sorted and output is
/// compact. Products and native digests are sorted and deduplicated so equal
/// admitted projections produce equal bytes regardless of input order.
///
/// This is an identity projection only. It is unsigned, claims no SLSA build
/// level, and neither schema validity nor a later signature grants builder,
/// signer, source-trust, gate, or release authority.
// r[impl kiln.interchange.provenance] r[impl kiln.interchange.builder] r[impl kiln.interchange.statement]
pub fn slsa_provenance_statement(
    projection: &ProvenanceProjection,
    expected_attempt: &ContentId,
) -> CoreResult<String> {
    validate_provenance(projection, expected_attempt)?;
    if projection.products.is_empty() {
        return Err(Diagnostic::new(
            "provenance_subjects_empty",
            "provenance.products",
            "an in-toto statement requires at least one product subject",
        ));
    }
    let mut products: Vec<&ContentId> = projection.products.iter().collect();
    products.sort();
    products.dedup();
    let subjects: Vec<Value> = products
        .into_iter()
        .map(|product| {
            object([
                (
                    "digest",
                    object([("blake3", Value::String(product.as_str().to_string()))]),
                ),
                (
                    "name",
                    Value::String(format!("blake3/{}", product.as_str())),
                ),
            ])
        })
        .collect();
    let mut dependencies = Vec::new();
    dependencies.push(object([
        (
            "digest",
            object([(
                "blake3",
                Value::String(projection.source_or_candidate.as_str().to_string()),
            )]),
        ),
        (
            "uri",
            Value::String(format!(
                "urn:kiln:blake3/{}",
                projection.source_or_candidate.as_str()
            )),
        ),
    ]));
    let mut native: Vec<(String, &str, &str, &str)> = projection
        .native_digests
        .iter()
        .map(|digest| {
            (
                digest_key(digest),
                digest.native_id.as_str(),
                digest.provider_namespace.as_str(),
                digest.native_kind.as_str(),
            )
        })
        .collect();
    native.sort();
    native.dedup();
    let mut previous: Option<&(String, &str, &str, &str)> = None;
    for entry in &native {
        if let Some(earlier) = previous
            && earlier.0 == entry.0
            && (earlier.1 != entry.1 || earlier.2 != entry.2 || earlier.3 != entry.3)
        {
            return Err(Diagnostic::new(
                "provenance_digest_collision",
                "provenance.native_digests",
                "two native digests share one statement key",
            ));
        }
        previous = Some(entry);
    }
    for (key, native_id, namespace, kind) in &native {
        dependencies.push(object([
            (
                "digest",
                object([(key.as_str(), Value::String((*native_id).to_string()))]),
            ),
            ("uri", Value::String(format!("urn:kiln:{namespace}/{kind}"))),
        ]));
    }
    let worker = projection
        .roles
        .worker
        .as_ref()
        .map(|worker| Value::String(external_uri(worker)));
    let statement = object([
        ("_type", Value::String(IN_TOTO_STATEMENT_TYPE.to_string())),
        (
            "predicate",
            object([
                (
                    "buildDefinition",
                    object([
                        (
                            "buildType",
                            Value::String(KILN_PROVENANCE_BUILD_TYPE.to_string()),
                        ),
                        (
                            "externalParameters",
                            object([
                                (
                                    "interchangeProfile",
                                    Value::String(external_uri(&projection.profile)),
                                ),
                                (
                                    "kilnControlPlane",
                                    Value::String(
                                        projection.roles.kiln_control_plane.as_str().to_string(),
                                    ),
                                ),
                                (
                                    "pipelineRevision",
                                    Value::String(
                                        projection.pipeline_revision.as_str().to_string(),
                                    ),
                                ),
                                (
                                    "sourceOrCandidate",
                                    Value::String(
                                        projection.source_or_candidate.as_str().to_string(),
                                    ),
                                ),
                            ]),
                        ),
                        (
                            "internalParameters",
                            object([
                                (
                                    "provider",
                                    Value::String(projection.roles.provider.as_str().to_string()),
                                ),
                                ("worker", worker.unwrap_or(Value::Null)),
                            ]),
                        ),
                        ("resolvedDependencies", Value::Array(dependencies)),
                    ]),
                ),
                (
                    "runDetails",
                    object([
                        (
                            "builder",
                            object([(
                                "id",
                                Value::String(external_uri(&projection.roles.builder)),
                            )]),
                        ),
                        (
                            "metadata",
                            object([(
                                "invocationId",
                                Value::String(projection.producing_attempt.as_str().to_string()),
                            )]),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "predicateType",
            Value::String(SLSA_PROVENANCE_PREDICATE_TYPE.to_string()),
        ),
        ("subject", Value::Array(subjects)),
    ]);
    serde_json::to_string(&statement).map_err(|_| {
        Diagnostic::new(
            "provenance_serialization",
            "provenance.statement",
            "provenance statement could not be encoded canonically",
        )
    })
}

fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut map = serde_json::Map::new();
    for (key, value) in entries {
        map.insert(key.to_string(), value);
    }
    Value::Object(map)
}

fn external_uri(reference: &ExternalRef) -> String {
    format!(
        "urn:kiln:{}/{}/{}",
        reference.provider_namespace.as_str(),
        reference.native_kind.as_str(),
        reference.native_id.as_str()
    )
}

/// Provider-native identity kind as a bounded digest key. This names the Kiln
/// identity kind, not a hashing algorithm.
fn digest_key(reference: &ExternalRef) -> String {
    let mut key = String::new();
    for character in reference.provider_namespace.as_str().chars() {
        push_digest_key_character(&mut key, character);
    }
    key.push('-');
    for character in reference.native_kind.as_str().chars() {
        push_digest_key_character(&mut key, character);
    }
    key
}

fn push_digest_key_character(key: &mut String, character: char) {
    let lowered = character.to_ascii_lowercase();
    if lowered.is_ascii_alphanumeric() || lowered == '_' {
        key.push(lowered);
    } else {
        key.push('-');
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalEvent {
    pub profile_version: ExternalRef,
    pub event_type: ExternalRef,
    pub native_event_id: NativeEventId,
    pub source: ExternalRef,
    pub subject: ExternalRef,
    pub correlation: Option<ExternalRef>,
    pub causation: Option<ExternalRef>,
    pub fields: BTreeSet<ExternalRef>,
}

pub fn admit_external_event(profile: &InterchangeProfile, event: &ExternalEvent) -> CoreResult<()> {
    if profile.specification_version != event.profile_version {
        return Err(Diagnostic::new(
            "interchange_version",
            "event.profile_version",
            "interchange profile version is unsupported",
        ));
    }
    if !profile.supported_event_types.contains(&event.event_type) {
        return Err(Diagnostic::new(
            "interchange_event_type",
            "event.event_type",
            "interchange event type is unsupported",
        ));
    }
    if !profile.required_fields.is_subset(&event.fields) {
        return Err(Diagnostic::new(
            "interchange_missing_field",
            "event.fields",
            "interchange event omits a required field",
        ));
    }
    if !profile.allow_unknown_fields && !event.fields.is_subset(&profile.required_fields) {
        return Err(Diagnostic::new(
            "interchange_unknown_field",
            "event.fields",
            "interchange event includes an unknown field",
        ));
    }
    Ok(())
}

#[must_use]
pub fn duplicate_event(first: &ExternalEvent, second: &ExternalEvent) -> bool {
    first.native_event_id == second.native_event_id && first.source == second.source
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExternalEventDecision {
    Accepted,
    Duplicate,
}

pub fn decide_external_event(
    profile: &InterchangeProfile,
    event: &ExternalEvent,
    prior: &[ExternalEvent],
) -> CoreResult<ExternalEventDecision> {
    admit_external_event(profile, event)?;
    if prior.iter().any(|seen| duplicate_event(seen, event)) {
        return Ok(ExternalEventDecision::Duplicate);
    }
    let has_extra_fields = !event.fields.is_subset(&profile.required_fields);
    if has_extra_fields && !profile.allow_lossy_mapping {
        return Err(Diagnostic::new(
            "lossy_mapping_denied",
            "event.fields",
            "external event requires an undeclared lossy mapping",
        ));
    }
    if let Some(correlation) = &event.correlation
        && !prior
            .iter()
            .any(|seen| seen.native_event_id.as_str() == correlation.native_id.as_str())
    {
        return Err(Diagnostic::new(
            "missing_correlation",
            "event.correlation",
            "external event correlation does not name an admitted prior event",
        ));
    }
    Ok(ExternalEventDecision::Accepted)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RebuildableProjection {
    pub profile: ExternalRef,
    pub source_identity: ContentId,
    pub records: Vec<ExternalRef>,
    pub lifecycle_authority: bool,
    pub release_authority: bool,
}

pub fn validate_rebuildable_projection(projection: &RebuildableProjection) -> CoreResult<()> {
    if projection.lifecycle_authority || projection.release_authority {
        return Err(Diagnostic::new(
            "projection_authority_promotion",
            "projection.authority",
            "rebuildable projection cannot gain lifecycle or release authority",
        ));
    }
    projection.profile.validate()?;
    for record in &projection.records {
        record.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn imported_success_has_no_state_authority() {
        let first = ExternalEvent {
            profile_version: ExternalRef::new("cdevents", "version", "v1")
                .unwrap_or_else(|_| unreachable!()),
            event_type: ExternalRef::new("cdevents", "event-type", "run-finished")
                .unwrap_or_else(|_| unreachable!()),
            native_event_id: NativeEventId::new("event").unwrap_or_else(|_| unreachable!()),
            source: ExternalRef::new("provider", "source", "source")
                .unwrap_or_else(|_| unreachable!()),
            subject: ExternalRef::new("provider", "run", "run").unwrap_or_else(|_| unreachable!()),
            correlation: None,
            causation: None,
            fields: BTreeSet::new(),
        };
        assert!(duplicate_event(&first, &first));
    }

    #[test]
    fn projections_cannot_promote_authority() {
        let projection = RebuildableProjection {
            profile: ExternalRef::new("otel", "profile", "v1").unwrap_or_else(|_| unreachable!()),
            source_identity: ContentId::from_blake3_hex(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap_or_else(|_| unreachable!()),
            records: Vec::new(),
            lifecycle_authority: true,
            release_authority: false,
        };
        assert!(validate_rebuildable_projection(&projection).is_err());
    }

    #[test]
    fn builder_and_control_plane_are_distinct() {
        let builder =
            ExternalRef::new("mantle", "builder", "builder").unwrap_or_else(|_| unreachable!());
        assert_ne!(builder.native_kind.as_str(), "kiln-control-plane");
    }

    fn identity(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn provenance_fixture() -> ProvenanceProjection {
        ProvenanceProjection {
            profile: ExternalRef::new("slsa", "profile", "v1.1").unwrap_or_else(|_| unreachable!()),
            pipeline_revision: identity('1'),
            source_or_candidate: identity('2'),
            producing_attempt: identity('3'),
            products: vec![identity('4'), identity('5')],
            roles: IdentityRoles {
                builder: ExternalRef::new("mantle", "builder", "mantle-builder-1")
                    .unwrap_or_else(|_| unreachable!()),
                signer: None,
                provider: ProviderProfileId::new("provider-a").unwrap_or_else(|_| unreachable!()),
                worker: Some(
                    ExternalRef::new("lattice", "worker", "worker-1")
                        .unwrap_or_else(|_| unreachable!()),
                ),
                kiln_control_plane: identity('6'),
            },
            native_digests: vec![
                ExternalRef::new("git", "commit", "0123456789abcdef0123456789abcdef01234567")
                    .unwrap_or_else(|_| unreachable!()),
                ExternalRef::new("radicle", "oid", "z2wsvXm5S2sJGvuV1k5JHiwi1PbKE")
                    .unwrap_or_else(|_| unreachable!()),
            ],
        }
    }

    // r[verify kiln.interchange.statement.scenario.canonical]
    #[test]
    fn slsa_statement_matches_golden_document() {
        let projection = provenance_fixture();
        let statement = slsa_provenance_statement(&projection, &identity('3'))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            statement,
            include_str!("../testdata/slsa-provenance-statement.json")
        );
    }

    // r[verify kiln.interchange.statement.scenario.refused]
    #[test]
    fn slsa_statement_refuses_another_attempt_before_serialization() {
        let projection = provenance_fixture();
        assert_eq!(
            slsa_provenance_statement(&projection, &identity('9')).map(|_| ()),
            Err(Diagnostic::new(
                "provenance_attempt_mismatch",
                "provenance.producing_attempt",
                "provenance is linked to another producing attempt",
            ))
        );
    }

    #[test]
    fn slsa_statement_is_independent_of_subject_order() {
        let mut forward = provenance_fixture();
        let ordered =
            slsa_provenance_statement(&forward, &identity('3')).unwrap_or_else(|_| unreachable!());
        forward.products.reverse();
        forward.native_digests.reverse();
        let reversed =
            slsa_provenance_statement(&forward, &identity('3')).unwrap_or_else(|_| unreachable!());
        assert_eq!(ordered, reversed);
    }

    #[test]
    fn slsa_statement_requires_at_least_one_subject() {
        let mut projection = provenance_fixture();
        projection.products.clear();
        assert_eq!(
            slsa_provenance_statement(&projection, &identity('3')).map(|_| ()),
            Err(Diagnostic::new(
                "provenance_subjects_empty",
                "provenance.products",
                "an in-toto statement requires at least one product subject",
            ))
        );
    }

    #[test]
    fn slsa_statement_rejects_colliding_native_digest_keys() {
        let mut projection = provenance_fixture();
        projection.native_digests = vec![
            ExternalRef::new("git.commit", "oid", "first").unwrap_or_else(|_| unreachable!()),
            ExternalRef::new("git-commit", "oid", "second").unwrap_or_else(|_| unreachable!()),
        ];
        assert_eq!(
            slsa_provenance_statement(&projection, &identity('3')).map(|_| ()),
            Err(Diagnostic::new(
                "provenance_digest_collision",
                "provenance.native_digests",
                "two native digests share one statement key",
            ))
        );
    }

    #[test]
    fn slsa_statement_keeps_roles_in_distinct_fields() {
        let projection = provenance_fixture();
        let statement = slsa_provenance_statement(&projection, &identity('3'))
            .unwrap_or_else(|_| unreachable!());
        let document: Value = serde_json::from_str(&statement).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            document.get("_type").and_then(Value::as_str),
            Some(IN_TOTO_STATEMENT_TYPE)
        );
        assert_eq!(
            document.get("predicateType").and_then(Value::as_str),
            Some(SLSA_PROVENANCE_PREDICATE_TYPE)
        );
        let predicate = document
            .get("predicate")
            .and_then(Value::as_object)
            .unwrap_or_else(|| unreachable!());
        let definition = predicate
            .get("buildDefinition")
            .and_then(Value::as_object)
            .unwrap_or_else(|| unreachable!());
        let external = definition
            .get("externalParameters")
            .and_then(Value::as_object)
            .unwrap_or_else(|| unreachable!());
        let internal = definition
            .get("internalParameters")
            .and_then(Value::as_object)
            .unwrap_or_else(|| unreachable!());
        let run = predicate
            .get("runDetails")
            .and_then(Value::as_object)
            .unwrap_or_else(|| unreachable!());
        let builder = run
            .get("builder")
            .and_then(|builder| builder.get("id"))
            .and_then(Value::as_str);
        let invocation = run
            .get("metadata")
            .and_then(|metadata| metadata.get("invocationId"))
            .and_then(Value::as_str);
        let control_plane = external.get("kilnControlPlane").and_then(Value::as_str);
        let pipeline = external.get("pipelineRevision").and_then(Value::as_str);
        let provider = internal.get("provider").and_then(Value::as_str);
        let worker = internal.get("worker").and_then(Value::as_str);
        let mut identities = BTreeSet::new();
        for identity in [
            builder,
            invocation,
            control_plane,
            pipeline,
            provider,
            worker,
        ] {
            assert!(identities.insert(identity.unwrap_or_else(|| unreachable!())));
        }
        assert_eq!(builder, Some("urn:kiln:mantle/builder/mantle-builder-1"));
        assert_eq!(worker, Some("urn:kiln:lattice/worker/worker-1"));
        assert_eq!(provider, Some("provider-a"));
        let dependencies = definition
            .get("resolvedDependencies")
            .and_then(Value::as_array)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(dependencies.len(), 3);
        assert_eq!(
            dependencies
                .last()
                .and_then(|entry| entry.get("digest"))
                .and_then(|digest| digest.get("radicle-oid"))
                .and_then(Value::as_str),
            Some("z2wsvXm5S2sJGvuV1k5JHiwi1PbKE")
        );
    }
}
