use kiln_codec::{encode_and_identify, identity_from_bytes};
use kiln_core::domain::{Event, State};
use kiln_core::operations::{JournalRecord, replay};
use kiln_core::types::{CONTRACT_VERSION_V1, CoreResult, Diagnostic, ExternalRef};
use serde::{Deserialize, Serialize};

/// Named bound on one journal document read.
pub const MAX_JOURNAL_BYTES: usize = 16 * 1024 * 1024;

/// Named bound on the record count of one journal document.
pub const MAX_JOURNAL_RECORDS: usize = 65_536;

/// Named bound on the canonical encoding of one journal event.
pub const MAX_EVENT_IDENTITY_BYTES: usize = 4 * 1024 * 1024;

/// Exact field set of one canonical journal record.
const RECORD_FIELDS: [&str; 5] = [
    "event",
    "event_identity",
    "expected_revision",
    "schema_version",
    "stream",
];

/// Canonical encoding convention behind a journal event identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventIdentityConvention {
    /// BLAKE3 over the packed Preserves encoding of the event.
    PackedPreserves,
    /// BLAKE3 over the canonical JSON encoding of the event.
    CanonicalJson,
}

impl EventIdentityConvention {
    /// Map an admitted writer identity kind onto its encoding convention.
    #[must_use]
    pub fn for_kind(kind: &str) -> Option<Self> {
        match kind {
            "pilot-event-blake3" | "ci-event-blake3" => Some(Self::PackedPreserves),
            "event-blake3" => Some(Self::CanonicalJson),
            _ => None,
        }
    }
}

/// A journal document admitted into canonical Kiln state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedJournal {
    pub state: State,
    pub records: Vec<JournalRecord>,
    pub schema: Option<String>,
    pub declared_identity: Option<String>,
}

/// The canonical record stream of one Kiln journal document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalDocument {
    pub schema: Option<String>,
    pub declared_identity: Option<String>,
    pub records: Vec<JournalRecord>,
}

#[derive(Deserialize)]
struct RawJournalDocument {
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    identity: Option<String>,
    records: Vec<serde_json::Value>,
}

impl JournalDocument {
    /// Parse a journal document from its canonical JSON bytes.
    ///
    /// The document envelope is shell owned, so unknown envelope fields are
    /// ignored. Every journal record is parsed with an exact field set.
    pub fn parse(bytes: &[u8]) -> CoreResult<Self> {
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err(Diagnostic::new(
                "journal_byte_bound",
                "journal.document",
                "journal document exceeds its named byte bound",
            ));
        }
        let raw: RawJournalDocument = serde_json::from_slice(bytes).map_err(|_| {
            Diagnostic::new(
                "malformed_journal_document",
                "journal.document",
                "journal document is not a canonical Kiln journal envelope",
            )
        })?;
        if raw.records.len() > MAX_JOURNAL_RECORDS {
            return Err(Diagnostic::new(
                "journal_record_bound",
                "journal.document.records",
                "journal document exceeds its named record bound",
            ));
        }
        let mut records = Vec::with_capacity(raw.records.len());
        for value in &raw.records {
            records.push(parse_record(value)?);
        }
        Ok(Self {
            schema: raw.schema,
            declared_identity: raw.identity,
            records,
        })
    }
}

fn parse_record(value: &serde_json::Value) -> CoreResult<JournalRecord> {
    let field = "journal.document.records";
    let object = value.as_object().ok_or_else(|| {
        Diagnostic::new(
            "malformed_journal_record",
            field,
            "journal record is not a JSON object",
        )
    })?;
    for name in RECORD_FIELDS {
        if !object.contains_key(name) {
            return Err(Diagnostic::new(
                "malformed_journal_record",
                field,
                "journal record omits a canonical field",
            ));
        }
    }
    if object.len() != RECORD_FIELDS.len() {
        return Err(Diagnostic::new(
            "malformed_journal_record",
            field,
            "journal record carries an unexpected field",
        ));
    }
    serde_json::from_value(value.clone()).map_err(|_| {
        Diagnostic::new(
            "malformed_journal_record",
            field,
            "journal record is not a canonical Kiln journal record",
        )
    })
}

/// Admit a parsed journal document into canonical Kiln state.
///
/// Admission verifies every event identity against the canonical encoding of
/// its own event and replays the whole record stream canonically.
// r[impl kiln.observe.dashboard.journal]
// r[impl kiln.observe.dashboard.journal.scenario.match]
pub fn admit_document(document: &JournalDocument) -> CoreResult<AdmittedJournal> {
    for record in &document.records {
        verify_record(record)?;
    }
    let state = replay(&State::default(), &document.records).map_err(|error| {
        // r[impl kiln.observe.dashboard.journal.scenario.replay_failure]
        Diagnostic::new(
            "journal_replay_rejected",
            "journal.document.records",
            &format!(
                "journal record stream does not replay canonically: {}",
                error.message
            ),
        )
    })?;
    Ok(AdmittedJournal {
        state,
        records: document.records.clone(),
        schema: document.schema.clone(),
        declared_identity: document.declared_identity.clone(),
    })
}

fn verify_record(record: &JournalRecord) -> CoreResult<()> {
    let field = "journal.document.records";
    if record.schema_version != CONTRACT_VERSION_V1 {
        return Err(Diagnostic::new(
            "unsupported_history_version",
            field,
            "journal record version has no admitted deterministic replay path",
        ));
    }
    record.stream.validate()?;
    record.event_identity.validate()?;
    let kind = record.event_identity.native_kind.as_str();
    if record.event_identity.provider_namespace.as_str() != "kiln" {
        return Err(Diagnostic::new(
            "unsupported_event_identity",
            field,
            "journal event identity is not owned by the Kiln provider namespace",
        ));
    }
    let Some(convention) = EventIdentityConvention::for_kind(kind) else {
        // r[impl kiln.observe.dashboard.journal.scenario.unsupported_identity_kind]
        return Err(Diagnostic::new(
            "unsupported_event_identity",
            field,
            "journal event identity kind has no admitted encoding convention",
        ));
    };
    let expected = recompute_identity(&record.event, convention)?;
    if expected != record.event_identity.native_id.as_str() {
        // r[impl kiln.observe.dashboard.journal.scenario.substituted_event]
        return Err(Diagnostic::new(
            "event_identity_mismatch",
            field,
            "journal event bytes do not match their recorded BLAKE3 identity",
        ));
    }
    Ok(())
}

fn recompute_identity(event: &Event, convention: EventIdentityConvention) -> CoreResult<String> {
    match convention {
        EventIdentityConvention::PackedPreserves => {
            let (_, identity) =
                encode_and_identify(event, MAX_EVENT_IDENTITY_BYTES).map_err(|_| {
                    Diagnostic::new(
                        "event_identity_encoding",
                        "journal.document.records.event",
                        "canonical packed event encoding failed",
                    )
                })?;
            Ok(identity.as_str().to_string())
        }
        EventIdentityConvention::CanonicalJson => {
            let bytes = serde_json::to_vec(event).map_err(|_| {
                Diagnostic::new(
                    "event_identity_encoding",
                    "journal.document.records.event",
                    "canonical JSON event encoding failed",
                )
            })?;
            let identity = identity_from_bytes(&bytes).map_err(|_| {
                Diagnostic::new(
                    "event_identity_encoding",
                    "journal.document.records.event",
                    "canonical event identity construction failed",
                )
            })?;
            Ok(identity.as_str().to_string())
        }
    }
}

/// Describe one external reference for operator display.
#[must_use]
pub fn reference_text(reference: &ExternalRef) -> String {
    format!(
        "{}:{}:{}",
        reference.provider_namespace.as_str(),
        reference.native_kind.as_str(),
        reference.native_id.as_str()
    )
}

/// The admitted journal facts reported to the operator.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalFacts {
    pub schema: Option<String>,
    pub declared_identity: Option<String>,
    pub record_count: u64,
    pub state_revision: u64,
}
