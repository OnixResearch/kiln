//! Pure decisions for the Lattice implementation CLI transport: which single
//! credential variable may reach the Lattice process, whether the request
//! binds the reserved plan, and what a Lattice exit proves about execution.
//!
//! The refusal vocabulary follows Lattice's coordinator transport contract
//! (`docs/candidate-worker.md`, "Coordinator transport", Lattice `db64cd65`).

use serde_json::Value;
use std::ffi::OsString;

pub const REFUSAL_SCHEMA: &str = "lattice.implementation-attempt-refusal.v1";
pub const OBSERVATION_SCHEMA: &str = "lattice.implementation-attempt-observation.v1";
/// Recovery found no intent and exclusively closed the attempt.
pub const CLOSED_STATUS: &str = "not_started_closed_by_recovery";

/// Refusal reasons Lattice reports with `dispatched:false` when no worker
/// effect of the refused attempt can have started.
const NOT_DISPATCHED_REASONS: [&str; 5] = [
    "stale_fence",
    "crossed_attempt",
    "profile_revision_superseded",
    "revision_not_admitted",
    "revision_blocked",
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

/// Classify the complete stdout of a non-zero Lattice exit against the
/// request envelope.
#[must_use]
pub fn classify_refusal(envelope: &Value, stdout: &[u8]) -> RefusalOutcome {
    let Ok(record) = serde_json::from_slice::<Value>(stdout) else {
        return RefusalOutcome::Retain;
    };
    let same_text = |field: &str| {
        record.get(field).and_then(Value::as_str).is_some()
            && record.get(field) == envelope.get(field)
    };
    let bound = record.get("schema").and_then(Value::as_str) == Some(REFUSAL_SCHEMA)
        && same_text("work_id")
        && same_text("attempt_id")
        && record.get("fence").and_then(Value::as_u64).is_some()
        && record.get("fence") == envelope.get("fence");
    match (
        bound,
        record.get("dispatched").and_then(Value::as_bool),
        record.get("reason").and_then(Value::as_str),
    ) {
        (true, Some(false), Some(reason)) if NOT_DISPATCHED_REASONS.contains(&reason) => {
            RefusalOutcome::BeforeEffect {
                reason: reason.to_owned(),
            }
        }
        _ => RefusalOutcome::Retain,
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

/// Classify one exit-zero observation of the exact request and plan.
///
/// # Errors
/// Refuses another schema, envelope or echoed plan, and a closure that carries
/// a result.
pub fn classify_observation(
    request: &Value,
    observed: &Value,
    plan: &str,
) -> Result<Observed, &'static str> {
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
    if observed
        .get("result")
        .is_none_or(|result| !result.is_null())
    {
        return Err("closed-before-execution observation carries a result");
    }
    Ok(Observed::ClosedBeforeExecution)
}

#[cfg(test)]
#[path = "worker_settlement_tests.rs"]
mod tests;
