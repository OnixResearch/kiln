//! Pure decisions for the Lattice implementation CLI transport: which single
//! credential variable may reach the Lattice process, whether the request
//! binds the reserved plan, and what a Lattice exit proves about execution.
//!
//! The refusal vocabulary follows Lattice's coordinator transport contract
//! (`docs/candidate-worker.md`, "Coordinator transport", Lattice `db64cd65`).

use kiln_core::fair_admission::SettlementProvenance;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;

pub const REFUSAL_SCHEMA: &str = "lattice.implementation-attempt-refusal.v1";
pub const OBSERVATION_SCHEMA: &str = "lattice.implementation-attempt-observation.v1";
/// Recovery found no intent and exclusively closed the attempt.
pub const CLOSED_STATUS: &str = "not_started_closed_by_recovery";

/// Refusal reasons Lattice reports with `dispatched:false` when no worker
/// effect of the refused attempt can have started, and whose check Kiln
/// accepts as a no-start proof for its only `run` launch.
///
/// `stale_fence` is deliberately absent: Lattice `db64cd65` records a fence
/// before it writes the execution intent and does not recheck it, so a
/// concurrent lower-fence invocation can still start after another one was
/// refused as stale. It stays for reconciliation until Lattice closes that race.
const NOT_DISPATCHED_REASONS: [&str; 4] = [
    "crossed_attempt",
    "profile_revision_superseded",
    "revision_not_admitted",
    "revision_blocked",
];
/// Reasons that carry the agent-revision lifecycle's own refusal.
const REVISION_REASONS: [&str; 3] = [
    "revision_not_admitted",
    "revision_blocked",
    "export_revoked",
];

/// Credential variable names are short upper-case identifiers.
const MAX_CREDENTIAL_KEY_BYTES: usize = 128;
/// Names that change how the Lattice process itself runs, never a credential.
const RESERVED_NAMES: [&str; 14] = [
    "PATH", "HOME", "USER", "LOGNAME", "SHELL", "PWD", "OLDPWD", "TMPDIR", "IFS", "ENV",
    "BASH_ENV", "TZ", "LANG", "TERM",
];
const RESERVED_PREFIXES: [&str; 8] = [
    "LD_", "DYLD_", "LATTICE_", "LC_", "RUST_", "NIX_", "SSL_", "GIT_",
];

/// Why the request's credential reference cannot reach the Lattice process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialRefusal {
    /// The broker or credential is not the admitted reference shape.
    Malformed,
    /// Only the host `env` provider can be resolved by the standalone CLI.
    UnsupportedProvider,
    /// The key is not a bounded upper-case variable name.
    MalformedKey,
    /// The key names process-control state rather than a credential.
    ReservedName,
    /// The named variable is absent or empty in the dispatcher environment.
    Missing,
}

impl CredentialRefusal {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Malformed => "worker_credential_malformed",
            Self::UnsupportedProvider => "worker_credential_unsupported_provider",
            Self::MalformedKey => "worker_credential_malformed_key",
            Self::ReservedName => "worker_credential_reserved_name",
            Self::Missing => "worker_credential_missing",
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::Malformed => "the model broker credential is not an admitted reference",
            Self::UnsupportedProvider => "the model broker credential provider is not env",
            Self::MalformedKey => "the credential variable name is malformed",
            Self::ReservedName => "the credential variable name is reserved",
            Self::Missing => "the credential variable is absent or empty",
        }
    }
}

impl std::fmt::Display for CredentialRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: {}; nothing was dispatched",
            self.code(),
            self.message()
        )
    }
}

/// The one variable the admitted request's `omp.model_broker.credential`
/// names, or `None` when the broker declares no credential.
///
/// # Errors
/// Refuses a malformed reference, a provider other than `env`, a `field`, a
/// malformed key and a reserved process-control name.
pub fn credential_variable(request: &Value) -> Result<Option<&str>, CredentialRefusal> {
    let broker = request
        .pointer("/omp/model_broker")
        .and_then(Value::as_object)
        .ok_or(CredentialRefusal::Malformed)?;
    let credential = match broker.get("credential") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Object(credential)) => credential,
        Some(_) => return Err(CredentialRefusal::Malformed),
    };
    if credential
        .keys()
        .any(|name| !matches!(name.as_str(), "provider" | "key" | "field"))
        || credential
            .get("field")
            .is_some_and(|field| !field.is_null())
    {
        return Err(CredentialRefusal::Malformed);
    }
    match credential.get("provider").map(Value::as_str) {
        Some(Some("env")) => {}
        Some(Some(_)) => return Err(CredentialRefusal::UnsupportedProvider),
        _ => return Err(CredentialRefusal::Malformed),
    }
    let key = credential
        .get("key")
        .and_then(Value::as_str)
        .ok_or(CredentialRefusal::Malformed)?;
    let mut characters = key.chars();
    if key.len() > MAX_CREDENTIAL_KEY_BYTES
        || !characters
            .next()
            .is_some_and(|first| first.is_ascii_uppercase())
        || !characters.all(|next| next.is_ascii_uppercase() || next.is_ascii_digit() || next == '_')
    {
        return Err(CredentialRefusal::MalformedKey);
    }
    if RESERVED_NAMES.contains(&key)
        || RESERVED_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
    {
        return Err(CredentialRefusal::ReservedName);
    }
    Ok(Some(key))
}

/// The exact environment of a fresh `run`: the named variable with its
/// dispatcher value, or nothing when no credential is declared.
///
/// # Errors
/// Refuses an absent or empty value for a declared variable.
pub fn credential_environment(
    variable: Option<&str>,
    value: Option<OsString>,
) -> Result<Vec<(OsString, OsString)>, CredentialRefusal> {
    let Some(variable) = variable else {
        return Ok(Vec::new());
    };
    match value {
        Some(value) if !value.is_empty() => Ok(vec![(OsString::from(variable), value)]),
        _ => Err(CredentialRefusal::Missing),
    }
}

/// Whether the exact request bytes carry the reserved dispatch-plan identity.
#[must_use]
pub fn request_binds_plan(request: &Value, plan: &str) -> bool {
    request.get("dispatch_plan").and_then(Value::as_str) == Some(plan)
}

/// What a non-zero Lattice exit proves about the attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefusalOutcome {
    /// A typed refusal of this attempt in which Lattice states no worker
    /// effect of the attempt can have started.
    BeforeEffect { reason: String },
    /// Anything else: a refusal that may follow an effect, a contradictory or
    /// crossed record, or no record at all. The attempt stays for
    /// reconciliation.
    Retain,
}

/// Which Lattice command produced an outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Launch {
    /// The attempt's first and only `run`, launched with no prior transport intent.
    Run,
    /// `recover` with the state root and program bound at the first launch.
    Recover,
}

/// The exact refusal record; every other field is refused.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RefusalRecord {
    schema: String,
    reason: String,
    work_id: String,
    attempt_id: String,
    fence: u64,
    dispatched: bool,
    #[serde(default)]
    revision_refusal: Option<serde_json::Map<String, Value>>,
}

/// Classify the complete stdout of a non-zero Lattice exit against the
/// request envelope.
///
/// A refusal returned by `recover` describes that invocation, not whether the
/// original `run` started, so it is always retained. Only one strict record
/// is admitted: duplicate or unknown fields, such as a result, artifacts or
/// acceptance, are retained, and `revision_refusal` is admitted only for the
/// reasons that carry it.
#[must_use]
pub fn classify_refusal(envelope: &Value, stdout: &[u8], launch: Launch) -> RefusalOutcome {
    if launch != Launch::Run {
        return RefusalOutcome::Retain;
    }
    let Ok(record) = serde_json::from_slice::<RefusalRecord>(stdout) else {
        return RefusalOutcome::Retain;
    };
    let same_text =
        |field: &str, value: &str| envelope.get(field).and_then(Value::as_str) == Some(value);
    let bound = record.schema == REFUSAL_SCHEMA
        && same_text("work_id", &record.work_id)
        && same_text("attempt_id", &record.attempt_id)
        && envelope.get("fence").and_then(Value::as_u64) == Some(record.fence);
    let reason = record.reason.as_str();
    let consistent = record.revision_refusal.is_some() == REVISION_REASONS.contains(&reason);
    if bound && consistent && !record.dispatched && NOT_DISPATCHED_REASONS.contains(&reason) {
        RefusalOutcome::BeforeEffect {
            reason: record.reason,
        }
    } else {
        RefusalOutcome::Retain
    }
}

/// What an exit-zero Lattice observation records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Observed {
    /// The attempt's worker result, to reconcile as its single execution.
    Execution,
    /// Recovery closed the attempt before any execution.
    ClosedBeforeExecution,
}

/// The exact closure record `recover` prints; every other field is refused.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ClosureRecord {
    schema: String,
    envelope: Value,
    request_identity: String,
    status: String,
    result: Value,
    accepted: bool,
    integrated: bool,
    dispatch_plan: String,
}

/// Classify the complete stdout of one exit-zero observation of the exact
/// request and plan.
///
/// # Errors
/// Refuses malformed output, another schema, envelope or echoed plan, and a
/// closure that is not exactly Lattice's closure record: one with duplicate
/// or unknown fields, a result, acceptance, integration, or another request
/// identity.
pub fn classify_observation(
    request: &Value,
    stdout: &[u8],
    plan: &str,
) -> Result<Observed, &'static str> {
    let observed: Value = serde_json::from_slice(stdout)
        .map_err(|_| "worker response malformed after possible dispatch")?;
    if observed.get("schema").and_then(Value::as_str) != Some(OBSERVATION_SCHEMA)
        || observed.get("envelope").is_none()
        || observed.get("envelope") != request.get("envelope")
    {
        return Err("worker output substituted the admitted envelope");
    }
    if observed.get("dispatch_plan").and_then(Value::as_str) != Some(plan) {
        return Err("worker output does not echo the reserved dispatch plan");
    }
    if observed.get("status").and_then(Value::as_str) != Some(CLOSED_STATUS) {
        return Ok(Observed::Execution);
    }
    let closure: ClosureRecord = serde_json::from_slice(stdout)
        .map_err(|_| "closed-before-execution observation is not the exact closure record")?;
    if closure.schema != OBSERVATION_SCHEMA
        || Some(&closure.envelope) != request.get("envelope")
        || request.get("admitted_envelope").and_then(Value::as_str)
            != Some(closure.request_identity.as_str())
        || closure.status != CLOSED_STATUS
        || !closure.result.is_null()
        || closure.accepted
        || closure.integrated
        || closure.dispatch_plan != plan
    {
        return Err(
            "closed-before-execution observation contradicts the request or carries evidence",
        );
    }
    Ok(Observed::ClosedBeforeExecution)
}

/// Observed per-dimension draw extracted from a Lattice attempt or repair
/// observation.
///
/// Missing, malformed, or duplicated draw records always fail closed to
/// [`ObservedDraw::Unknown`] so the full reservation remains charged. When
/// Lattice reports explicit `charged` numbers (including under `unknown`
/// provenance), [`ObservedDraw::reported`] carries them so any draw exceeding
/// the reservation is refused as a protocol violation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservedDraw {
    HostMeasured {
        charged: BTreeMap<String, u64>,
    },
    Unknown {
        reported: Option<BTreeMap<String, u64>>,
    },
}

impl ObservedDraw {
    #[must_use]
    pub const fn provenance(&self) -> SettlementProvenance {
        match self {
            Self::HostMeasured { .. } => SettlementProvenance::HostMeasured,
            Self::Unknown { .. } => SettlementProvenance::Unknown,
        }
    }

    #[must_use]
    pub const fn reported(&self) -> Option<&BTreeMap<String, u64>> {
        match self {
            Self::HostMeasured { charged } => Some(charged),
            Self::Unknown { reported } => reported.as_ref(),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ObservationDrawEnvelope {
    #[serde(default)]
    budget_lineage: Option<BudgetLineageDrawField>,
    #[serde(default)]
    usage: Option<Vec<UsageDrawRecord>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum BudgetLineageDrawField {
    /// Single-shot `lattice.implementation-attempt-observation.v1`:
    /// `budget_lineage: { binding, usage: [...], ... }`.
    Object(BudgetLineageDrawObject),
    /// Multi-step repair `lattice.implementation-attempt-repair-observation.v1`:
    /// `budget_lineage: "<64-hex>"` paired with top-level `usage: [...]`.
    Binding(String),
}

#[derive(Debug, serde::Deserialize)]
struct BudgetLineageDrawObject {
    #[serde(default)]
    binding: Option<String>,
    usage: Vec<UsageDrawRecord>,
}

#[derive(Debug, serde::Deserialize)]
struct UsageDrawRecord {
    phase: String,
    iteration: u64,
    #[serde(default)]
    model_requests: Option<u64>,
    #[serde(default)]
    tool_invocations: Option<u64>,
    provenance: String,
    charged: ChargedDrawRecord,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargedDrawRecord {
    model_requests: u64,
    #[serde(default)]
    tool_invocations: Option<u64>,
}

fn is_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Extract and validate Lattice's per-attempt draw from raw observation stdout.
///
/// Deserializing directly from raw bytes ensures duplicate JSON keys inside
/// `budget_lineage`, `usage`, or `charged` fail closed to `Unknown` rather
/// than silently taking the last value.
#[must_use]
pub fn extract_observed_draw(stdout: &[u8]) -> ObservedDraw {
    let fail_closed = || ObservedDraw::Unknown { reported: None };
    let Ok(envelope) = serde_json::from_slice::<ObservationDrawEnvelope>(stdout) else {
        return fail_closed();
    };
    let records = match (envelope.budget_lineage, envelope.usage) {
        (Some(BudgetLineageDrawField::Object(object)), None) => {
            if object
                .binding
                .as_deref()
                .is_some_and(|binding| !is_hex_digest(binding))
            {
                return fail_closed();
            }
            object.usage
        }
        (Some(BudgetLineageDrawField::Binding(binding)), Some(usage)) => {
            if !is_hex_digest(&binding) {
                return fail_closed();
            }
            usage
        }
        _ => return fail_closed(),
    };
    if records.is_empty() {
        return fail_closed();
    }
    let mut seen_steps = BTreeSet::new();
    let mut all_host_measured = true;
    let mut all_tools_measured = true;
    let mut model_requests_total: u64 = 0;
    let mut tool_invocations_total: u64 = 0;
    let mut any_tool_invocations = false;
    for record in records {
        if record.phase.trim().is_empty()
            || !seen_steps.insert((record.phase.clone(), record.iteration))
        {
            return fail_closed();
        }
        let Some(next_model) = model_requests_total.checked_add(record.charged.model_requests)
        else {
            return fail_closed();
        };
        model_requests_total = next_model;
        if let Some(tools) = record.charged.tool_invocations {
            any_tool_invocations = true;
            let Some(next_tools) = tool_invocations_total.checked_add(tools) else {
                return fail_closed();
            };
            tool_invocations_total = next_tools;
        }
        match record.provenance.as_str() {
            "host_measured" => {
                if record.model_requests != Some(record.charged.model_requests) {
                    return fail_closed();
                }
                match record.tool_invocations {
                    Some(tools) => {
                        if record.charged.tool_invocations != Some(tools) {
                            return fail_closed();
                        }
                    }
                    None => {
                        all_tools_measured = false;
                    }
                }
            }
            "unknown" => {
                all_host_measured = false;
                all_tools_measured = false;
            }
            _ => return fail_closed(),
        }
    }
    let mut charged = BTreeMap::new();
    charged.insert("model_requests".to_owned(), model_requests_total);
    if all_host_measured {
        if any_tool_invocations && all_tools_measured {
            charged.insert("tool_invocations".to_owned(), tool_invocations_total);
        }
        ObservedDraw::HostMeasured { charged }
    } else {
        if any_tool_invocations {
            charged.insert("tool_invocations".to_owned(), tool_invocations_total);
        }
        ObservedDraw::Unknown {
            reported: Some(charged),
        }
    }
}

#[cfg(test)]
#[path = "worker_settlement_tests.rs"]
mod tests;
