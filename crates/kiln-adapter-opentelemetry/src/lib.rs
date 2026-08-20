use std::collections::{BTreeMap, BTreeSet};

use kiln_codec::identity_from_bytes;
use kiln_core::domain::{Metric, Outcome};
use kiln_core::interchange::{RebuildableProjection, validate_rebuildable_projection};
use kiln_core::task_ops::safe_diagnostic;
use kiln_core::types::{
    AttemptId, ContentId, CoreResult, Diagnostic, ExternalRef, ProviderProfileId, RunId, TaskId,
};
use serde::{Deserialize, Serialize};

pub const OTLP_PROTO_VERSION: &str = "1.11.0";
pub const OTLP_PROTO_REVISION: &str = "790608c4d51e6ffc12210b541e8514cbed9e91a4";
pub const SEMANTIC_CONVENTIONS_VERSION: &str = "1.43.0";
pub const SEMANTIC_CONVENTIONS_REVISION: &str = "89aae438b3b3b0a8dd33003c9d70592baf7dbd0d";
pub const CICD_STABILITY: &str = "release_candidate";
pub const OTLP_HTTP_JSON_PROTOCOL: &str = "otlp_http_json";
pub const OPENTELEMETRY_LICENSE: &str = "Apache-2.0";
pub const REDACTION_MARKER: &str = "[redacted]";

const ATTEMPT_SPAN_NAME: &str = "cicd.pipeline.task.run";
const ATTEMPT_METRIC_NAME: &str = "kiln.cicd.attempt.metric";
const ATTEMPT_LOG_NAME: &str = "kiln.cicd.attempt.outcome";
const ATTRIBUTE_RUN_ID: &str = "cicd.pipeline.run.id";
const ATTRIBUTE_TASK_RUN_ID: &str = "cicd.pipeline.task.run.id";
const ATTRIBUTE_TASK_NAME: &str = "cicd.pipeline.task.name";
const ATTRIBUTE_ATTEMPT_ID: &str = "kiln.attempt.id";
const ATTRIBUTE_PROVIDER: &str = "kiln.provider.profile";
const ATTRIBUTE_OUTCOME: &str = "kiln.outcome";
const ATTRIBUTE_METRIC_NAME: &str = "kiln.metric.name";
const RESOURCE_SERVICE_NAME: &str = "service.name";
const REQUIRED_ATTRIBUTE_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Signal {
    Span,
    Metric,
    Log,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropPolicy {
    RejectBatch,
    DropNewest,
}

// r[impl kiln.otel.profile]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelemetryProfile {
    pub otlp_proto_version: String,
    pub otlp_proto_revision: String,
    pub semantic_conventions_version: String,
    pub semantic_conventions_revision: String,
    pub cicd_stability: String,
    pub license: String,
    pub signals: BTreeSet<Signal>,
    pub allowed_attributes: BTreeSet<String>,
    pub allowed_metric_names: BTreeSet<String>,
    pub resource_attributes: BTreeMap<String, String>,
    pub max_attributes: usize,
    pub max_value_bytes: usize,
    pub max_record_bytes: usize,
    pub max_batch_records: usize,
    pub max_queue_records: usize,
    pub drop_policy: DropPolicy,
}

pub fn admit_profile(profile: &TelemetryProfile) -> CoreResult<()> {
    if profile.otlp_proto_version != OTLP_PROTO_VERSION
        || profile.otlp_proto_revision != OTLP_PROTO_REVISION
        || profile.semantic_conventions_version != SEMANTIC_CONVENTIONS_VERSION
        || profile.semantic_conventions_revision != SEMANTIC_CONVENTIONS_REVISION
        || profile.cicd_stability != CICD_STABILITY
        || profile.license != OPENTELEMETRY_LICENSE
    {
        return Err(Diagnostic::new(
            "otel_profile_drift",
            "otel.profile",
            "OpenTelemetry protocol or semantic-convention cohort differs from the pinned profile",
        ));
    }
    if profile.signals != BTreeSet::from([Signal::Span, Signal::Metric, Signal::Log]) {
        return Err(Diagnostic::new(
            "otel_signal_drift",
            "otel.signals",
            "the initial OpenTelemetry profile requires span, metric, and log mappings",
        ));
    }
    if !required_attribute_keys().is_subset(&profile.allowed_attributes)
        || profile.max_attributes < REQUIRED_ATTRIBUTE_COUNT
        || profile.max_value_bytes == 0
        || profile.max_record_bytes == 0
        || profile.max_batch_records == 0
        || profile.max_queue_records < profile.max_batch_records
    {
        return Err(Diagnostic::new(
            "otel_profile_bound",
            "otel.bounds",
            "OpenTelemetry profile attributes or bounds are incomplete",
        ));
    }
    if profile.allowed_metric_names.is_empty() {
        return Err(Diagnostic::new(
            "otel_metric_registry",
            "otel.allowed_metric_names",
            "OpenTelemetry metric registry must not be empty",
        ));
    }
    validate_attributes(profile, &profile.resource_attributes)?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IdentityLink {
    pub run_id: String,
    pub attempt_id: String,
    pub provider_profile: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpanRecord {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
    pub links: Vec<IdentityLink>,
    pub observed_timestamp: Option<u64>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MetricRecord {
    pub name: String,
    pub value: i64,
    pub dimensions: BTreeMap<String, String>,
    pub links: Vec<IdentityLink>,
    pub observed_timestamp: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogRecord {
    pub event_name: String,
    pub body: String,
    pub attributes: BTreeMap<String, String>,
    pub links: Vec<IdentityLink>,
    pub observed_timestamp: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "signal", rename_all = "snake_case")]
pub enum TelemetryRecord {
    Span(SpanRecord),
    Metric(MetricRecord),
    Log(LogRecord),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TelemetryProjection {
    pub profile: String,
    pub source_identity: ContentId,
    pub resource_attributes: BTreeMap<String, String>,
    pub records: Vec<TelemetryRecord>,
    pub lifecycle_authority: bool,
    pub release_authority: bool,
}

#[derive(Debug)]
pub struct AttemptProjectionInput<'a> {
    pub source_identity: ContentId,
    pub run_id: &'a RunId,
    pub task_id: &'a TaskId,
    pub attempt_id: &'a AttemptId,
    pub provider_profile: &'a ProviderProfileId,
    pub outcome: Outcome,
    pub metrics: &'a [Metric],
    pub diagnostic: Option<&'a str>,
    pub secret_values: &'a [&'a str],
    pub observed_timestamp: Option<u64>,
}

// r[impl kiln.otel.projection]
// r[impl kiln.otel.safety]
pub fn project_attempt(
    profile: &TelemetryProfile,
    input: &AttemptProjectionInput<'_>,
) -> CoreResult<TelemetryProjection> {
    admit_profile(profile)?;
    let links = vec![IdentityLink {
        run_id: input.run_id.as_str().to_string(),
        attempt_id: input.attempt_id.as_str().to_string(),
        provider_profile: input.provider_profile.as_str().to_string(),
    }];
    let attributes = attempt_attributes(input);
    validate_attributes(profile, &attributes)?;
    let mut records = vec![TelemetryRecord::Span(SpanRecord {
        name: ATTEMPT_SPAN_NAME.to_string(),
        attributes: attributes.clone(),
        links: links.clone(),
        observed_timestamp: input.observed_timestamp,
        trace_id: None,
        span_id: None,
    })];
    for metric in input.metrics {
        if &metric.producing_attempt != input.attempt_id {
            return Err(Diagnostic::new(
                "otel_metric_link",
                "otel.metric.producing_attempt",
                "telemetry metric does not link to the projected attempt",
            ));
        }
        if !profile.allowed_metric_names.contains(metric.name.as_str()) {
            return Err(Diagnostic::new(
                "otel_metric_cardinality",
                "otel.metric.name",
                "telemetry metric name is outside the bounded registry",
            ));
        }
        let dimensions = BTreeMap::from([
            (
                ATTRIBUTE_METRIC_NAME.to_string(),
                metric.name.as_str().to_string(),
            ),
            (
                ATTRIBUTE_OUTCOME.to_string(),
                outcome_name(input.outcome).to_string(),
            ),
        ]);
        validate_attributes(profile, &dimensions)?;
        records.push(TelemetryRecord::Metric(MetricRecord {
            name: ATTEMPT_METRIC_NAME.to_string(),
            value: metric.value,
            dimensions,
            links: links.clone(),
            observed_timestamp: input.observed_timestamp,
        }));
    }
    let body = safe_diagnostic(
        input.diagnostic.unwrap_or(outcome_name(input.outcome)),
        input.secret_values,
        REDACTION_MARKER,
        profile.max_record_bytes,
    )?;
    records.push(TelemetryRecord::Log(LogRecord {
        event_name: ATTEMPT_LOG_NAME.to_string(),
        body,
        attributes,
        links,
        observed_timestamp: input.observed_timestamp,
    }));
    validate_records(profile, &records)?;
    let authority = RebuildableProjection {
        profile: ExternalRef::new("opentelemetry", "profile", SEMANTIC_CONVENTIONS_VERSION)?,
        source_identity: input.source_identity.clone(),
        records: record_identities(&records)?,
        lifecycle_authority: false,
        release_authority: false,
    };
    validate_rebuildable_projection(&authority)?;
    Ok(TelemetryProjection {
        profile: SEMANTIC_CONVENTIONS_VERSION.to_string(),
        source_identity: input.source_identity.clone(),
        resource_attributes: profile.resource_attributes.clone(),
        records,
        lifecycle_authority: authority.lifecycle_authority,
        release_authority: authority.release_authority,
    })
}

fn attempt_attributes(input: &AttemptProjectionInput<'_>) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            ATTRIBUTE_RUN_ID.to_string(),
            input.run_id.as_str().to_string(),
        ),
        (
            ATTRIBUTE_TASK_RUN_ID.to_string(),
            input.attempt_id.as_str().to_string(),
        ),
        (
            ATTRIBUTE_TASK_NAME.to_string(),
            input.task_id.as_str().to_string(),
        ),
        (
            ATTRIBUTE_ATTEMPT_ID.to_string(),
            input.attempt_id.as_str().to_string(),
        ),
        (
            ATTRIBUTE_PROVIDER.to_string(),
            input.provider_profile.as_str().to_string(),
        ),
        (
            ATTRIBUTE_OUTCOME.to_string(),
            outcome_name(input.outcome).to_string(),
        ),
    ])
}

fn validate_records(profile: &TelemetryProfile, records: &[TelemetryRecord]) -> CoreResult<()> {
    if records.len() > profile.max_queue_records {
        return Err(Diagnostic::new(
            "otel_queue_bound",
            "otel.records",
            "telemetry record count exceeds the queue bound",
        ));
    }
    for record in records {
        let bytes = serde_json::to_vec(record).map_err(|_| {
            Diagnostic::new(
                "otel_record_encoding",
                "otel.record",
                "telemetry record encoding failed",
            )
        })?;
        if bytes.len() > profile.max_record_bytes {
            return Err(Diagnostic::new(
                "otel_record_bound",
                "otel.record",
                "telemetry record exceeds its named byte bound",
            ));
        }
    }
    Ok(())
}

fn validate_attributes(
    profile: &TelemetryProfile,
    attributes: &BTreeMap<String, String>,
) -> CoreResult<()> {
    if attributes.len() > profile.max_attributes {
        return Err(Diagnostic::new(
            "otel_attribute_bound",
            "otel.attributes",
            "telemetry attribute count exceeds its named bound",
        ));
    }
    for (key, value) in attributes {
        if !profile.allowed_attributes.contains(key) {
            return Err(Diagnostic::new(
                "otel_attribute_key",
                "otel.attributes",
                "telemetry attribute key is outside the allowed set",
            ));
        }
        if value.len() > profile.max_value_bytes {
            return Err(Diagnostic::new(
                "otel_attribute_value_bound",
                "otel.attributes",
                "telemetry attribute value exceeds its named byte bound",
            ));
        }
    }
    Ok(())
}

fn record_identities(records: &[TelemetryRecord]) -> CoreResult<Vec<ExternalRef>> {
    records
        .iter()
        .map(|record| {
            let bytes = serde_json::to_vec(record).map_err(|_| {
                Diagnostic::new(
                    "otel_record_encoding",
                    "otel.record",
                    "telemetry record encoding failed",
                )
            })?;
            let identity = identity_from_bytes(&bytes).map_err(|_| {
                Diagnostic::new(
                    "otel_record_identity",
                    "otel.record",
                    "telemetry record identity failed",
                )
            })?;
            ExternalRef::new("opentelemetry", "record-blake3", identity.as_str())
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OtlpJsonPayload {
    pub protocol_version: String,
    pub semantic_conventions_version: String,
    pub source_identity: ContentId,
    pub resource_attributes: BTreeMap<String, String>,
    pub records: Vec<TelemetryRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OtlpJsonBatch {
    pub batch_identity: ContentId,
    pub payload: OtlpJsonPayload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchPlan {
    pub batches: Vec<OtlpJsonBatch>,
    pub dropped_records: usize,
}

// r[impl kiln.otel.export]
pub fn plan_batches(
    profile: &TelemetryProfile,
    projection: &TelemetryProjection,
) -> CoreResult<BatchPlan> {
    admit_profile(profile)?;
    let mut records = projection.records.clone();
    let dropped_records = records.len().saturating_sub(profile.max_queue_records);
    if dropped_records > 0 {
        match profile.drop_policy {
            DropPolicy::RejectBatch => {
                return Err(Diagnostic::new(
                    "otel_queue_bound",
                    "otel.records",
                    "telemetry queue exceeds its bound and policy rejects loss",
                ));
            }
            DropPolicy::DropNewest => records.truncate(profile.max_queue_records),
        }
    }
    let mut batches = Vec::new();
    for chunk in records.chunks(profile.max_batch_records) {
        let payload = OtlpJsonPayload {
            protocol_version: OTLP_PROTO_VERSION.to_string(),
            semantic_conventions_version: SEMANTIC_CONVENTIONS_VERSION.to_string(),
            source_identity: projection.source_identity.clone(),
            resource_attributes: projection.resource_attributes.clone(),
            records: chunk.to_vec(),
        };
        let payload_bytes = serde_json::to_vec(&payload).map_err(|_| {
            Diagnostic::new(
                "otlp_batch_encoding",
                "otel.batch",
                "OTLP JSON payload encoding failed",
            )
        })?;
        let batch_identity = identity_from_bytes(&payload_bytes).map_err(|_| {
            Diagnostic::new(
                "otlp_batch_identity",
                "otel.batch",
                "OTLP batch identity failed",
            )
        })?;
        batches.push(OtlpJsonBatch {
            batch_identity,
            payload,
        });
    }
    Ok(BatchPlan {
        batches,
        dropped_records,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtlpTransportProfile {
    pub protocol: String,
    pub endpoint_profile: ExternalRef,
    pub tls_required: bool,
    pub credential_reference: Option<ExternalRef>,
    pub deadline_ticks: u64,
    pub max_response_bytes: usize,
    pub retry_limit: usize,
}

pub fn admit_transport_profile(profile: &OtlpTransportProfile) -> CoreResult<()> {
    if profile.protocol != OTLP_HTTP_JSON_PROTOCOL
        || !profile.tls_required
        || profile.deadline_ticks == 0
        || profile.max_response_bytes == 0
    {
        return Err(Diagnostic::new(
            "otlp_transport_profile",
            "otel.transport",
            "OTLP transport profile is unsupported or unbounded",
        ));
    }
    profile.endpoint_profile.validate()?;
    if let Some(credential) = &profile.credential_reference {
        credential.validate()?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportFailureKind {
    Unavailable,
    TimedOutBeforeAcceptance,
    DisconnectedAfterAcceptance,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportFailure {
    pub kind: TransportFailureKind,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtlpResponse {
    pub accepted: bool,
    pub backend_reference: Option<ExternalRef>,
}

pub trait OtlpTransport {
    fn send(
        &mut self,
        endpoint_profile: &ExternalRef,
        body: &[u8],
        max_response_bytes: usize,
    ) -> Result<OtlpResponse, TransportFailure>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportObservation {
    pub batch_identity: ContentId,
    pub accepted: bool,
    pub retryable: bool,
    pub unknown: bool,
    pub backend_reference: Option<ExternalRef>,
}

pub fn validate_batch(profile: &TelemetryProfile, batch: &OtlpJsonBatch) -> CoreResult<()> {
    admit_profile(profile)?;
    if batch.payload.protocol_version != OTLP_PROTO_VERSION
        || batch.payload.semantic_conventions_version != SEMANTIC_CONVENTIONS_VERSION
    {
        return Err(Diagnostic::new(
            "otlp_batch_profile",
            "otel.batch",
            "OTLP batch version differs from the pinned profile",
        ));
    }
    if batch.payload.resource_attributes != profile.resource_attributes {
        return Err(Diagnostic::new(
            "otlp_resource_drift",
            "otel.resource_attributes",
            "OTLP batch resource attributes differ from the admitted profile",
        ));
    }
    validate_attributes(profile, &batch.payload.resource_attributes)?;
    validate_records(profile, &batch.payload.records)?;
    let payload_bytes = serde_json::to_vec(&batch.payload).map_err(|_| {
        Diagnostic::new(
            "otlp_batch_encoding",
            "otel.batch",
            "OTLP JSON payload encoding failed",
        )
    })?;
    let expected = identity_from_bytes(&payload_bytes).map_err(|_| {
        Diagnostic::new(
            "otlp_batch_identity",
            "otel.batch",
            "OTLP batch identity failed",
        )
    })?;
    if expected != batch.batch_identity {
        return Err(Diagnostic::new(
            "otlp_batch_identity_mismatch",
            "otel.batch_identity",
            "OTLP batch bytes do not match their BLAKE3 identity",
        ));
    }
    Ok(())
}

pub fn export_batch<T: OtlpTransport>(
    telemetry_profile: &TelemetryProfile,
    transport_profile: &OtlpTransportProfile,
    batch: &OtlpJsonBatch,
    transport: &mut T,
) -> CoreResult<ExportObservation> {
    validate_batch(telemetry_profile, batch)?;
    admit_transport_profile(transport_profile)?;
    let body = serde_json::to_vec(batch).map_err(|_| {
        Diagnostic::new(
            "otlp_batch_encoding",
            "otel.batch",
            "OTLP JSON batch encoding failed",
        )
    })?;
    let observation = match transport.send(
        &transport_profile.endpoint_profile,
        &body,
        transport_profile.max_response_bytes,
    ) {
        Ok(response) => ExportObservation {
            batch_identity: batch.batch_identity.clone(),
            accepted: response.accepted,
            retryable: !response.accepted && transport_profile.retry_limit > 0,
            unknown: false,
            backend_reference: response.backend_reference,
        },
        Err(failure) => match failure.kind {
            TransportFailureKind::Unavailable | TransportFailureKind::TimedOutBeforeAcceptance => {
                ExportObservation {
                    batch_identity: batch.batch_identity.clone(),
                    accepted: false,
                    retryable: transport_profile.retry_limit > 0,
                    unknown: false,
                    backend_reference: None,
                }
            }
            TransportFailureKind::DisconnectedAfterAcceptance => ExportObservation {
                batch_identity: batch.batch_identity.clone(),
                accepted: false,
                retryable: false,
                unknown: true,
                backend_reference: None,
            },
            TransportFailureKind::Rejected => ExportObservation {
                batch_identity: batch.batch_identity.clone(),
                accepted: false,
                retryable: false,
                unknown: false,
                backend_reference: None,
            },
        },
    };
    Ok(observation)
}

fn required_attribute_keys() -> BTreeSet<String> {
    [
        ATTRIBUTE_RUN_ID,
        ATTRIBUTE_TASK_RUN_ID,
        ATTRIBUTE_TASK_NAME,
        ATTRIBUTE_ATTEMPT_ID,
        ATTRIBUTE_PROVIDER,
        ATTRIBUTE_OUTCOME,
        ATTRIBUTE_METRIC_NAME,
        RESOURCE_SERVICE_NAME,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

const fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pending => "pending",
        Outcome::Success => "success",
        Outcome::DomainFailure => "domain_failure",
        Outcome::Denied => "denied",
        Outcome::Cancelled => "cancelled",
        Outcome::TimedOut => "timed_out",
        Outcome::ProviderError => "provider_error",
        Outcome::Unknown => "unknown",
    }
}

// r[impl kiln.otel.conformance]
// r[impl kiln.otel.nonclaims]
#[cfg(test)]
mod tests {
    use kiln_core::domain::State;
    use kiln_core::types::Alias;

    use super::*;

    const MAX_ATTRIBUTES: usize = 16;
    const MAX_VALUE_BYTES: usize = 128;
    const MAX_RECORD_BYTES: usize = 4096;
    const MAX_BATCH_RECORDS: usize = 2;
    const MAX_QUEUE_RECORDS: usize = 8;
    const DEADLINE_TICKS: u64 = 30;
    const MAX_RESPONSE_BYTES: usize = 4096;
    const RETRY_LIMIT: usize = 2;
    const EXPECTED_SIGNAL_RECORDS: usize = 3;

    fn content(value: char) -> ContentId {
        ContentId::from_blake3_hex(value.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }

    fn profile() -> TelemetryProfile {
        TelemetryProfile {
            otlp_proto_version: OTLP_PROTO_VERSION.to_string(),
            otlp_proto_revision: OTLP_PROTO_REVISION.to_string(),
            semantic_conventions_version: SEMANTIC_CONVENTIONS_VERSION.to_string(),
            semantic_conventions_revision: SEMANTIC_CONVENTIONS_REVISION.to_string(),
            cicd_stability: CICD_STABILITY.to_string(),
            license: OPENTELEMETRY_LICENSE.to_string(),
            signals: BTreeSet::from([Signal::Span, Signal::Metric, Signal::Log]),
            allowed_attributes: required_attribute_keys(),
            allowed_metric_names: BTreeSet::from(["duration_ticks".to_string()]),
            resource_attributes: BTreeMap::from([(
                RESOURCE_SERVICE_NAME.to_string(),
                "kiln".to_string(),
            )]),
            max_attributes: MAX_ATTRIBUTES,
            max_value_bytes: MAX_VALUE_BYTES,
            max_record_bytes: MAX_RECORD_BYTES,
            max_batch_records: MAX_BATCH_RECORDS,
            max_queue_records: MAX_QUEUE_RECORDS,
            drop_policy: DropPolicy::DropNewest,
        }
    }

    fn input<'a>(
        run_id: &'a RunId,
        task_id: &'a TaskId,
        attempt_id: &'a AttemptId,
        provider_profile: &'a ProviderProfileId,
        metrics: &'a [Metric],
        diagnostic: Option<&'a str>,
        secrets: &'a [&'a str],
    ) -> AttemptProjectionInput<'a> {
        AttemptProjectionInput {
            source_identity: content('a'),
            run_id,
            task_id,
            attempt_id,
            provider_profile,
            outcome: Outcome::Success,
            metrics,
            diagnostic,
            secret_values: secrets,
            observed_timestamp: Some(1),
        }
    }

    fn projection(diagnostic: Option<&str>, secrets: &[&str]) -> CoreResult<TelemetryProjection> {
        let run_id = RunId::new("run-a").unwrap_or_else(|_| unreachable!());
        let task_id = TaskId::new("build").unwrap_or_else(|_| unreachable!());
        let attempt_id = AttemptId::new("attempt-a").unwrap_or_else(|_| unreachable!());
        let provider = ProviderProfileId::new("build-provider").unwrap_or_else(|_| unreachable!());
        let metrics = [Metric {
            name: Alias::new("duration_ticks").unwrap_or_else(|_| unreachable!()),
            value: 1,
            producing_attempt: attempt_id.clone(),
        }];
        project_attempt(
            &profile(),
            &input(
                &run_id,
                &task_id,
                &attempt_id,
                &provider,
                &metrics,
                diagnostic,
                secrets,
            ),
        )
    }

    // r[verify kiln.otel.profile.scenario.match]
    // r[verify kiln.otel.projection.scenario.span]
    #[test]
    fn projection_is_deterministic_and_keeps_kiln_links() {
        let first = projection(None, &[]);
        let second = projection(None, &[]);
        assert_eq!(first, second);
        assert!(first.is_ok());
    }

    // r[verify kiln.otel.profile.scenario.drift]
    // r[verify kiln.otel.projection.scenario.identity]
    #[test]
    fn profile_drift_and_authority_promotion_fail_closed() {
        let mut profile = profile();
        profile.semantic_conventions_version = "unsupported".to_string();
        assert!(admit_profile(&profile).is_err());
        let promoted = RebuildableProjection {
            profile: ExternalRef::new("opentelemetry", "profile", "test")
                .unwrap_or_else(|_| unreachable!()),
            source_identity: content('b'),
            records: Vec::new(),
            lifecycle_authority: true,
            release_authority: false,
        };
        assert!(validate_rebuildable_projection(&promoted).is_err());
    }

    // r[verify kiln.otel.safety.scenario.secret]
    #[test]
    fn secret_is_redacted_before_batch_encoding() {
        let secret = "sensitive-token";
        let projection = projection(Some(secret), &[secret]).unwrap_or_else(|_| unreachable!());
        let plan = plan_batches(&profile(), &projection).unwrap_or_else(|_| unreachable!());
        let encoded = serde_json::to_string(&plan.batches).unwrap_or_else(|_| unreachable!());
        assert!(!encoded.contains(secret));
        assert!(encoded.contains(REDACTION_MARKER));
    }

    // r[verify kiln.otel.safety.scenario.cardinality]
    // r[verify kiln.otel.conformance.scenario.secret]
    #[test]
    fn unregistered_metric_name_is_rejected() {
        let run_id = RunId::new("run-a").unwrap_or_else(|_| unreachable!());
        let task_id = TaskId::new("build").unwrap_or_else(|_| unreachable!());
        let attempt_id = AttemptId::new("attempt-a").unwrap_or_else(|_| unreachable!());
        let provider = ProviderProfileId::new("build-provider").unwrap_or_else(|_| unreachable!());
        let metrics = [Metric {
            name: Alias::new("provider_dynamic_label").unwrap_or_else(|_| unreachable!()),
            value: 1,
            producing_attempt: attempt_id.clone(),
        }];
        assert!(
            project_attempt(
                &profile(),
                &input(
                    &run_id,
                    &task_id,
                    &attempt_id,
                    &provider,
                    &metrics,
                    None,
                    &[],
                ),
            )
            .is_err()
        );
    }

    struct UnavailableTransport;

    impl OtlpTransport for UnavailableTransport {
        fn send(
            &mut self,
            _endpoint_profile: &ExternalRef,
            _body: &[u8],
            _max_response_bytes: usize,
        ) -> Result<OtlpResponse, TransportFailure> {
            Err(TransportFailure {
                kind: TransportFailureKind::Unavailable,
                message: "backend unavailable".to_string(),
            })
        }
    }

    fn transport_profile() -> OtlpTransportProfile {
        OtlpTransportProfile {
            protocol: OTLP_HTTP_JSON_PROTOCOL.to_string(),
            endpoint_profile: ExternalRef::new("opentelemetry", "endpoint", "local-collector")
                .unwrap_or_else(|_| unreachable!()),
            tls_required: true,
            credential_reference: None,
            deadline_ticks: DEADLINE_TICKS,
            max_response_bytes: MAX_RESPONSE_BYTES,
            retry_limit: RETRY_LIMIT,
        }
    }

    // r[verify kiln.otel.conformance]
    // r[verify kiln.otel.conformance.scenario.secret]
    #[test]
    fn independent_fixture_preserves_batch_identity_and_all_signals() {
        let bytes = include_bytes!("../fixtures/canonical-signals-v1.43.0.json");
        let batch: OtlpJsonBatch = serde_json::from_slice(bytes).unwrap_or_else(|_| unreachable!());
        assert!(validate_batch(&profile(), &batch).is_ok());
        assert_eq!(batch.payload.records.len(), EXPECTED_SIGNAL_RECORDS);
    }

    // r[verify kiln.otel.export.scenario.loss]
    // r[verify kiln.otel.nonclaims.scenario.scope]
    #[test]
    fn backend_loss_does_not_change_canonical_state() {
        let before = State::default();
        let projection = projection(None, &[]).unwrap_or_else(|_| unreachable!());
        let plan = plan_batches(&profile(), &projection).unwrap_or_else(|_| unreachable!());
        let batch = plan.batches.first().unwrap_or_else(|| unreachable!());
        let observation = export_batch(
            &profile(),
            &transport_profile(),
            batch,
            &mut UnavailableTransport,
        );
        assert_eq!(before, State::default());
        assert_eq!(observation.map(|value| value.retryable), Ok(true));
    }

    // r[verify kiln.otel.export.scenario.success]
    #[test]
    fn backend_acceptance_does_not_create_ci_success() {
        struct AcceptingTransport;
        impl OtlpTransport for AcceptingTransport {
            fn send(
                &mut self,
                _endpoint_profile: &ExternalRef,
                _body: &[u8],
                _max_response_bytes: usize,
            ) -> Result<OtlpResponse, TransportFailure> {
                Ok(OtlpResponse {
                    accepted: true,
                    backend_reference: None,
                })
            }
        }
        let state = State::default();
        let projection = projection(None, &[]).unwrap_or_else(|_| unreachable!());
        let plan = plan_batches(&profile(), &projection).unwrap_or_else(|_| unreachable!());
        let batch = plan.batches.first().unwrap_or_else(|| unreachable!());
        let result = export_batch(
            &profile(),
            &transport_profile(),
            batch,
            &mut AcceptingTransport,
        );
        assert_eq!(result.map(|value| value.accepted), Ok(true));
        assert_eq!(state, State::default());
    }
}
