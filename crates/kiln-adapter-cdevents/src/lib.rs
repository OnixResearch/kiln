use std::collections::BTreeSet;

use kiln_core::domain::Outcome;
use kiln_core::interchange::{
    ExternalEvent, ExternalEventDecision, InterchangeKind, InterchangeProfile,
    decide_external_event,
};
use kiln_core::types::{CoreResult, DEFAULT_MAX_ID_BYTES, Diagnostic, ExternalRef, NativeEventId};
use serde::{Deserialize, Serialize};

pub const CDEVENTS_SPEC_VERSION: &str = "0.5.1";
pub const CDEVENTS_SOURCE_REVISION: &str = "df02ce0b6563a13bf35cc109993b03b18da8a14a";
pub const CLOUDEVENTS_SPEC_VERSION: &str = "1.0.2";
pub const CLOUDEVENTS_CONTEXT_VERSION: &str = "1.0";
pub const CDEVENTS_LICENSE: &str = "Apache-2.0";
pub const TASK_RUN_STARTED_TYPE: &str = "dev.cdevents.taskrun.started.0.3.0";
pub const TASK_RUN_FINISHED_TYPE: &str = "dev.cdevents.taskrun.finished.0.3.0";
pub const JSON_CONTENT_TYPE: &str = "application/json";
pub const CAUSATION_LINK_KIND: &str = "CAUSED_BY";

const REQUIRED_FIELD_COUNT: usize = 4;
const FIELD_CONTEXT_ID: &str = "context.id";
const FIELD_CONTEXT_SOURCE: &str = "context.source";
const FIELD_SUBJECT_ID: &str = "subject.id";
const FIELD_SUBJECT_CONTENT: &str = "subject.content";

// r[impl kiln.cdevents.nonclaims]
// This crate maps bounded values. It does not execute transport or grant CI authority.

// r[impl kiln.cdevents.profile]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CdEventsProfile {
    pub cdevents_spec_version: String,
    pub cdevents_source_revision: String,
    pub cloudevents_spec_version: String,
    pub cloudevents_context_version: String,
    pub license: String,
    pub supported_event_types: BTreeSet<String>,
    pub max_envelope_bytes: usize,
    pub max_text_bytes: usize,
    pub max_fields: usize,
    pub allow_lossy_mapping: bool,
}

pub fn admit_profile(profile: &CdEventsProfile) -> CoreResult<()> {
    if profile.cdevents_spec_version != CDEVENTS_SPEC_VERSION
        || profile.cdevents_source_revision != CDEVENTS_SOURCE_REVISION
        || profile.cloudevents_spec_version != CLOUDEVENTS_SPEC_VERSION
        || profile.cloudevents_context_version != CLOUDEVENTS_CONTEXT_VERSION
        || profile.license != CDEVENTS_LICENSE
    {
        return Err(Diagnostic::new(
            "cdevents_profile_drift",
            "cdevents.profile",
            "CDEvents or CloudEvents cohort differs from the pinned profile",
        ));
    }
    let required_types = supported_event_types();
    if profile.supported_event_types != required_types {
        return Err(Diagnostic::new(
            "cdevents_registry_drift",
            "cdevents.supported_event_types",
            "CDEvents event registry differs from the implemented mappings",
        ));
    }
    if profile.max_envelope_bytes == 0
        || profile.max_text_bytes == 0
        || profile.max_text_bytes > DEFAULT_MAX_ID_BYTES
        || profile.max_fields < REQUIRED_FIELD_COUNT
    {
        return Err(Diagnostic::new(
            "cdevents_bound",
            "cdevents.bounds",
            "CDEvents bounds are zero, too small, or exceed the core identity bound",
        ));
    }
    if profile.allow_lossy_mapping {
        return Err(Diagnostic::new(
            "cdevents_lossy_profile",
            "cdevents.allow_lossy_mapping",
            "the pinned CDEvents profile rejects lossy mappings",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudEventEnvelope {
    pub specversion: String,
    pub id: String,
    pub source: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub subject: String,
    pub time: String,
    pub datacontenttype: String,
    pub data: CdEvent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CdEvent {
    pub context: CdContext,
    pub subject: CdSubject,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CdContext {
    pub specversion: String,
    pub id: String,
    pub source: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub timestamp: String,
    #[serde(rename = "chainId", skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<CdLink>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CdLink {
    #[serde(rename = "linkType")]
    pub link_type: String,
    #[serde(rename = "linkKind")]
    pub link_kind: String,
    pub target: CdLinkTarget,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CdLinkTarget {
    #[serde(rename = "contextId")]
    pub context_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CdSubject {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub content: CdTaskRunContent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CdTaskRunContent {
    #[serde(rename = "taskName")]
    pub task_name: String,
    #[serde(rename = "pipelineRun")]
    pub pipeline_run: CdPipelineRun,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CdPipelineRun {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskRunKind {
    Started,
    Finished,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskRunProjection<'a> {
    pub kind: TaskRunKind,
    pub event_id: &'a str,
    pub source: &'a str,
    pub timestamp: &'a str,
    pub attempt_id: &'a str,
    pub task_name: &'a str,
    pub pipeline_run_id: &'a str,
    pub outcome: Option<Outcome>,
    pub correlation_id: Option<&'a str>,
    pub causation_id: Option<&'a str>,
}

// r[impl kiln.cdevents.mapping]
pub fn project_task_run(
    profile: &CdEventsProfile,
    input: &TaskRunProjection<'_>,
) -> CoreResult<CloudEventEnvelope> {
    admit_profile(profile)?;
    for (value, field) in [
        (input.event_id, "cdevents.id"),
        (input.source, "cdevents.source"),
        (input.timestamp, "cdevents.timestamp"),
        (input.attempt_id, "cdevents.subject.id"),
        (input.task_name, "cdevents.subject.task_name"),
        (input.pipeline_run_id, "cdevents.pipeline_run.id"),
    ] {
        admit_text(value, profile.max_text_bytes, field)?;
    }
    let (event_type, outcome) = match (input.kind, input.outcome) {
        (TaskRunKind::Started, None) => (TASK_RUN_STARTED_TYPE, None),
        (TaskRunKind::Finished, Some(outcome)) if outcome.is_terminal() => (
            TASK_RUN_FINISHED_TYPE,
            Some(outcome_name(outcome).to_string()),
        ),
        (TaskRunKind::Started, Some(_)) | (TaskRunKind::Finished, None) => {
            return Err(Diagnostic::new(
                "cdevents_outcome_link",
                "cdevents.outcome",
                "started events cannot contain outcomes and finished events require terminal outcomes",
            ));
        }
        (TaskRunKind::Finished, Some(_)) => {
            return Err(Diagnostic::new(
                "cdevents_nonterminal_finish",
                "cdevents.outcome",
                "finished CDEvents require a terminal Kiln outcome",
            ));
        }
    };
    let chain_id = optional_text(
        input.correlation_id,
        profile.max_text_bytes,
        "cdevents.chain_id",
    )?;
    let causation_id = optional_text(
        input.causation_id,
        profile.max_text_bytes,
        "cdevents.causation_id",
    )?;
    let links = causation_id
        .map(|context_id| {
            vec![CdLink {
                link_type: "RELATION".to_string(),
                link_kind: CAUSATION_LINK_KIND.to_string(),
                target: CdLinkTarget { context_id },
            }]
        })
        .unwrap_or_default();
    let data = CdEvent {
        context: CdContext {
            specversion: CDEVENTS_SPEC_VERSION.to_string(),
            id: input.event_id.to_string(),
            source: input.source.to_string(),
            event_type: event_type.to_string(),
            timestamp: input.timestamp.to_string(),
            chain_id,
            links,
        },
        subject: CdSubject {
            id: input.attempt_id.to_string(),
            source: Some(input.source.to_string()),
            content: CdTaskRunContent {
                task_name: input.task_name.to_string(),
                pipeline_run: CdPipelineRun {
                    id: input.pipeline_run_id.to_string(),
                    source: Some(input.source.to_string()),
                },
                outcome,
                errors: None,
            },
        },
    };
    Ok(CloudEventEnvelope {
        specversion: CLOUDEVENTS_CONTEXT_VERSION.to_string(),
        id: input.event_id.to_string(),
        source: input.source.to_string(),
        event_type: event_type.to_string(),
        subject: input.attempt_id.to_string(),
        time: input.timestamp.to_string(),
        datacontenttype: JSON_CONTENT_TYPE.to_string(),
        data,
    })
}

// r[impl kiln.cdevents.envelope]
pub fn encode(profile: &CdEventsProfile, envelope: &CloudEventEnvelope) -> CoreResult<Vec<u8>> {
    admit_envelope(profile, envelope)?;
    let bytes = serde_json::to_vec(envelope).map_err(|_| {
        Diagnostic::new(
            "cdevents_encode",
            "cdevents.envelope",
            "CDEvents JSON encoding failed",
        )
    })?;
    if bytes.len() > profile.max_envelope_bytes {
        return Err(Diagnostic::new(
            "cdevents_envelope_bound",
            "cdevents.envelope",
            "CDEvents envelope exceeds its named byte bound",
        ));
    }
    Ok(bytes)
}

pub fn decode(profile: &CdEventsProfile, bytes: &[u8]) -> CoreResult<CloudEventEnvelope> {
    admit_profile(profile)?;
    if bytes.len() > profile.max_envelope_bytes {
        return Err(Diagnostic::new(
            "cdevents_envelope_bound",
            "cdevents.envelope",
            "CDEvents envelope exceeds its named byte bound",
        ));
    }
    let envelope: CloudEventEnvelope = serde_json::from_slice(bytes).map_err(|_| {
        Diagnostic::new(
            "cdevents_decode",
            "cdevents.envelope",
            "CDEvents JSON envelope is malformed or has unknown fields",
        )
    })?;
    admit_envelope(profile, &envelope)?;
    Ok(envelope)
}

pub fn admit_envelope(profile: &CdEventsProfile, envelope: &CloudEventEnvelope) -> CoreResult<()> {
    admit_profile(profile)?;
    if envelope.specversion != CLOUDEVENTS_CONTEXT_VERSION
        || envelope.data.context.specversion != CDEVENTS_SPEC_VERSION
        || envelope.datacontenttype != JSON_CONTENT_TYPE
        || !profile.supported_event_types.contains(&envelope.event_type)
    {
        return Err(Diagnostic::new(
            "cdevents_envelope_profile",
            "cdevents.envelope",
            "CDEvents envelope version, type, or content type is unsupported",
        ));
    }
    if envelope.id != envelope.data.context.id
        || envelope.source != envelope.data.context.source
        || envelope.event_type != envelope.data.context.event_type
        || envelope.subject != envelope.data.subject.id
        || envelope.time != envelope.data.context.timestamp
    {
        return Err(Diagnostic::new(
            "cdevents_envelope_link",
            "cdevents.envelope",
            "CloudEvents context does not match the embedded CDEvent",
        ));
    }
    for (value, field) in [
        (envelope.id.as_str(), "cdevents.id"),
        (envelope.source.as_str(), "cdevents.source"),
        (envelope.event_type.as_str(), "cdevents.type"),
        (envelope.subject.as_str(), "cdevents.subject"),
        (envelope.time.as_str(), "cdevents.time"),
        (
            envelope.data.subject.content.task_name.as_str(),
            "cdevents.task_name",
        ),
        (
            envelope.data.subject.content.pipeline_run.id.as_str(),
            "cdevents.pipeline_run.id",
        ),
    ] {
        admit_text(value, profile.max_text_bytes, field)?;
    }
    let causation_links = envelope
        .data
        .context
        .links
        .iter()
        .filter(|link| link.link_kind == CAUSATION_LINK_KIND)
        .count();
    if causation_links > 1 {
        return Err(Diagnostic::new(
            "cdevents_causation_ambiguous",
            "cdevents.context.links",
            "CDEvents envelope has more than one causation link",
        ));
    }
    Ok(())
}

// r[impl kiln.cdevents.import]
pub fn import_external_event(
    profile: &CdEventsProfile,
    envelope: &CloudEventEnvelope,
    prior: &[ExternalEvent],
) -> CoreResult<(ExternalEvent, ExternalEventDecision)> {
    admit_envelope(profile, envelope)?;
    let correlation = envelope
        .data
        .context
        .chain_id
        .as_deref()
        .map(|value| ExternalRef::new("cdevents", "chain", value))
        .transpose()?;
    let causation = envelope
        .data
        .context
        .links
        .iter()
        .find(|link| link.link_kind == CAUSATION_LINK_KIND)
        .map(|link| ExternalRef::new("cdevents", "event", &link.target.context_id))
        .transpose()?;
    let event = ExternalEvent {
        profile_version: ExternalRef::new(
            "cdevents",
            "spec-version",
            &profile.cdevents_spec_version,
        )?,
        event_type: ExternalRef::new("cdevents", "event-type", &envelope.event_type)?,
        native_event_id: NativeEventId::new(envelope.id.clone())?,
        source: ExternalRef::new("cdevents", "source", &envelope.source)?,
        subject: ExternalRef::new("cdevents", "subject", &envelope.subject)?,
        correlation,
        causation,
        fields: required_fields()?,
    };
    let interchange_profile = InterchangeProfile {
        kind: InterchangeKind::CdEvents,
        specification_version: event.profile_version.clone(),
        supported_event_types: profile
            .supported_event_types
            .iter()
            .map(|value| ExternalRef::new("cdevents", "event-type", value))
            .collect::<CoreResult<BTreeSet<_>>>()?,
        required_fields: required_fields()?,
        allow_unknown_fields: false,
        allow_lossy_mapping: profile.allow_lossy_mapping,
    };
    let decision = decide_external_event(&interchange_profile, &event, prior)?;
    Ok((event, decision))
}

fn supported_event_types() -> BTreeSet<String> {
    BTreeSet::from([
        TASK_RUN_STARTED_TYPE.to_string(),
        TASK_RUN_FINISHED_TYPE.to_string(),
    ])
}

fn required_fields() -> CoreResult<BTreeSet<ExternalRef>> {
    [
        FIELD_CONTEXT_ID,
        FIELD_CONTEXT_SOURCE,
        FIELD_SUBJECT_ID,
        FIELD_SUBJECT_CONTENT,
    ]
    .into_iter()
    .map(|field| ExternalRef::new("cdevents", "field", field))
    .collect()
}

fn admit_text(value: &str, max_bytes: usize, field: &'static str) -> CoreResult<()> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(Diagnostic::new(
            "cdevents_text_bound",
            field,
            "CDEvents text is empty or exceeds its named byte bound",
        ));
    }
    Ok(())
}

fn optional_text(
    value: Option<&str>,
    max_bytes: usize,
    field: &'static str,
) -> CoreResult<Option<String>> {
    value
        .map(|text| {
            admit_text(text, max_bytes, field)?;
            Ok(text.to_string())
        })
        .transpose()
}

const fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pending => "pending",
        Outcome::Success => "success",
        Outcome::DomainFailure => "failure",
        Outcome::Denied => "denied",
        Outcome::Cancelled => "cancelled",
        Outcome::TimedOut => "timed_out",
        Outcome::ProviderError => "provider_error",
        Outcome::Unknown => "unknown",
    }
}

// r[impl kiln.cdevents.conformance]
#[cfg(test)]
mod tests {
    use super::*;

    const MAX_ENVELOPE_BYTES: usize = 16_384;
    const MAX_TEXT_BYTES: usize = 128;
    const MAX_FIELDS: usize = 16;
    const SOURCE: &str = "/kiln/tests";
    const TIMESTAMP: &str = "2026-08-19T00:00:00Z";

    fn profile() -> CdEventsProfile {
        CdEventsProfile {
            cdevents_spec_version: CDEVENTS_SPEC_VERSION.to_string(),
            cdevents_source_revision: CDEVENTS_SOURCE_REVISION.to_string(),
            cloudevents_spec_version: CLOUDEVENTS_SPEC_VERSION.to_string(),
            cloudevents_context_version: CLOUDEVENTS_CONTEXT_VERSION.to_string(),
            license: CDEVENTS_LICENSE.to_string(),
            supported_event_types: supported_event_types(),
            max_envelope_bytes: MAX_ENVELOPE_BYTES,
            max_text_bytes: MAX_TEXT_BYTES,
            max_fields: MAX_FIELDS,
            allow_lossy_mapping: false,
        }
    }

    fn finished<'a>(event_id: &'a str, correlation: Option<&'a str>) -> TaskRunProjection<'a> {
        TaskRunProjection {
            kind: TaskRunKind::Finished,
            event_id,
            source: SOURCE,
            timestamp: TIMESTAMP,
            attempt_id: "attempt-a",
            task_name: "build",
            pipeline_run_id: "run-a",
            outcome: Some(Outcome::Success),
            correlation_id: correlation,
            causation_id: None,
        }
    }

    // r[verify kiln.cdevents.profile.scenario.supported]
    // r[verify kiln.cdevents.mapping.scenario.roundtrip]
    // r[verify kiln.cdevents.envelope.scenario.valid]
    #[test]
    fn task_run_round_trip_preserves_declared_fields() {
        let profile = profile();
        let projected = project_task_run(&profile, &finished("event-a", None));
        let bytes = projected.and_then(|value| encode(&profile, &value));
        let decoded = bytes.and_then(|value| decode(&profile, &value));
        assert!(decoded.is_ok());
        assert_eq!(
            decoded.map(|value| value.event_type),
            Ok(TASK_RUN_FINISHED_TYPE.to_string())
        );
    }

    // r[verify kiln.cdevents.profile.scenario.unknown]
    // r[verify kiln.cdevents.envelope.scenario.missing]
    // r[verify kiln.cdevents.mapping.scenario.loss]
    #[test]
    fn unknown_event_type_and_missing_fields_fail_closed() {
        let profile = profile();
        let mut envelope = project_task_run(&profile, &finished("event-a", None))
            .unwrap_or_else(|_| unreachable!());
        envelope.event_type = "dev.cdevents.unknown.0.1.0".to_string();
        envelope.data.context.event_type = envelope.event_type.clone();
        assert!(admit_envelope(&profile, &envelope).is_err());
        assert!(decode(&profile, br#"{"specversion":"1.0"}"#).is_err());
    }

    // r[verify kiln.cdevents.import.scenario.duplicate]
    // r[verify kiln.cdevents.import.scenario.success]
    #[test]
    fn imported_success_is_observational_and_duplicates_are_explicit() {
        let profile = profile();
        let envelope = project_task_run(&profile, &finished("event-a", None))
            .unwrap_or_else(|_| unreachable!());
        let first =
            import_external_event(&profile, &envelope, &[]).unwrap_or_else(|_| unreachable!());
        assert_eq!(first.1, ExternalEventDecision::Accepted);
        let duplicate = import_external_event(&profile, &envelope, &[first.0]);
        assert_eq!(
            duplicate.map(|value| value.1),
            Ok(ExternalEventDecision::Duplicate)
        );
    }

    // r[verify kiln.cdevents.import.scenario.success]
    #[test]
    fn missing_correlation_rejects_without_canonical_action() {
        let profile = profile();
        let envelope = project_task_run(&profile, &finished("event-b", Some("event-a")))
            .unwrap_or_else(|_| unreachable!());
        assert!(import_external_event(&profile, &envelope, &[]).is_err());
    }

    // r[verify kiln.cdevents.conformance]
    // r[verify kiln.cdevents.conformance.scenario.negative]
    #[test]
    fn independent_fixture_decodes_with_the_pinned_profile() {
        let bytes = include_bytes!("../fixtures/taskrun-finished-v0.5.1.json");
        let decoded = decode(&profile(), bytes);
        assert!(decoded.is_ok());
        assert_eq!(
            decoded.map(|value| value.id),
            Ok("fixture-event".to_string())
        );
    }

    // r[verify kiln.cdevents.nonclaims.scenario.scope]
    #[test]
    fn lossy_profile_is_rejected() {
        let mut profile = profile();
        profile.allow_lossy_mapping = true;
        assert!(admit_profile(&profile).is_err());
    }
}
