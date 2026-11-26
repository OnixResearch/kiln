//! Pure operator control transitions, query validators, pane sanitization,
//! machine census projection, and bounded persistence for fleet controls.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kiln_adapter_herdr::client::MAX_READ_LINES;
use kiln_adapter_herdr::{HerdrMachine, MachineLoad, Occupancy};
use kiln_core::fleet::{ClaimState, FOREIGN_CLAIM_STALE_MS, FleetClaim, ForeignClaim};
use kiln_core::types::ContentId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::read_bounded;

pub const CONTROLS_SCHEMA: &str = "kiln.fleet-controls.v1";
pub const MAX_CONTROLS_BYTES: u64 = 256 * 1_024;
pub const MAX_REASON_BYTES: usize = 400;
pub const MAX_REQUEST_ID_BYTES: usize = 128;
pub const MAX_RECORDED_REQUEST_IDS: usize = 256;
pub const DEFAULT_PANE_LINES: u32 = 200;
pub const MAX_PANE_LINES: u32 = MAX_READ_LINES;
pub const MAX_PANE_TEXT_BYTES: usize = 128 * 1_024;
pub const MAX_EXPIRED_CLAIMS: usize = 100;
pub const MAX_DEFERRED_ITEMS: usize = 32;

/// Recorded drain state for a single fleet machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrainRecord {
    pub draining: bool,
    pub reason: Option<String>,
    pub updated_at_ms: u64,
    pub updated_by: String,
}

/// Persisted global dispatch pause state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchPauseState {
    pub paused: bool,
    pub reason: Option<String>,
    pub paused_at_ms: Option<u64>,
    pub paused_by: Option<String>,
}

/// Idempotency record for an operator mutation with a `request_id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedRequest {
    pub key: String,
    pub payload_hash: String,
    pub response: Value,
}

/// Pure operator-control state persisted beside the fleet store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetControls {
    pub schema: String,
    pub dispatch_pause: DispatchPauseState,
    pub drained_machines: BTreeMap<String, DrainRecord>,
    #[serde(default)]
    pub recent_requests: Vec<RecordedRequest>,
}

impl Default for FleetControls {
    fn default() -> Self {
        Self {
            schema: CONTROLS_SCHEMA.to_owned(),
            dispatch_pause: DispatchPauseState::default(),
            drained_machines: BTreeMap::new(),
            recent_requests: Vec::new(),
        }
    }
}

/// One deferred work candidate observed during a dispatch tick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeferredItem {
    pub work: Option<String>,
    pub reason: String,
}

/// Bounded summary of the most recent dispatch tick.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickSummary {
    pub ticked_at_ms: u64,
    pub free_slots: u32,
    pub planned_count: usize,
    pub performed_count: usize,
    pub deferred_count: usize,
    pub deferred: Vec<DeferredItem>,
    pub deferral_counts: BTreeMap<String, usize>,
}

/// Parsed `?include=` selector for `GET /v1/claims`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClaimsInclude {
    pub foreign: bool,
    pub expired: bool,
}

/// Bounded, ANSI-stripped plain-text pane transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedPane {
    pub lines: Vec<String>,
    pub text: String,
    pub line_count: usize,
    pub truncated: bool,
}

/// Conflict returned when a `request_id` is reused with a different payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestIdConflict;

impl FleetControls {
    /// Whether `machine` is currently marked draining.
    #[must_use]
    pub fn is_draining(&self, machine: &str) -> bool {
        self.drained_machines
            .get(machine)
            .is_some_and(|record| record.draining)
    }

    /// Drain record for `machine`, if currently draining.
    #[must_use]
    pub fn drain_record(&self, machine: &str) -> Option<&DrainRecord> {
        self.drained_machines
            .get(machine)
            .filter(|record| record.draining)
    }

    /// Sorted labels of currently draining machines.
    #[must_use]
    pub fn draining_machines(&self) -> Vec<String> {
        self.drained_machines
            .iter()
            .filter_map(|(machine, record)| record.draining.then(|| machine.clone()))
            .collect()
    }

    /// Set or clear drain state on `machine`.
    pub fn set_machine_drain(
        &mut self,
        machine: &str,
        draining: bool,
        reason: Option<String>,
        actor: &str,
        now_ms: u64,
    ) {
        if draining {
            self.drained_machines.insert(
                machine.to_owned(),
                DrainRecord {
                    draining: true,
                    reason,
                    updated_at_ms: now_ms,
                    updated_by: actor.to_owned(),
                },
            );
        } else {
            self.drained_machines.remove(machine);
        }
    }

    /// Set or clear global dispatch pause state.
    pub fn set_dispatch_pause(
        &mut self,
        paused: bool,
        reason: Option<String>,
        actor: &str,
        now_ms: u64,
    ) {
        if paused {
            self.dispatch_pause = DispatchPauseState {
                paused: true,
                reason,
                paused_at_ms: Some(now_ms),
                paused_by: Some(actor.to_owned()),
            };
        } else {
            self.dispatch_pause = DispatchPauseState::default();
        }
    }

    /// Look up a prior idempotent request by `(key, payload_hash)`.
    ///
    /// # Errors
    /// Returns [`RequestIdConflict`] when `key` was already recorded with a
    /// different payload hash.
    pub fn lookup_request(
        &self,
        key: &str,
        payload_hash: &str,
    ) -> Result<Option<&Value>, RequestIdConflict> {
        let Some(existing) = self.recent_requests.iter().find(|entry| entry.key == key) else {
            return Ok(None);
        };
        if existing.payload_hash == payload_hash {
            Ok(Some(&existing.response))
        } else {
            Err(RequestIdConflict)
        }
    }

    /// Record a completed idempotent request, evicting the oldest entries past
    /// [`MAX_RECORDED_REQUEST_IDS`].
    pub fn record_request(&mut self, key: String, payload_hash: String, response: Value) {
        if self.recent_requests.iter().any(|entry| entry.key == key) {
            return;
        }
        self.recent_requests.push(RecordedRequest {
            key,
            payload_hash,
            response,
        });
        if self.recent_requests.len() > MAX_RECORDED_REQUEST_IDS {
            let excess = self
                .recent_requests
                .len()
                .saturating_sub(MAX_RECORDED_REQUEST_IDS);
            self.recent_requests.drain(..excess);
        }
    }
}

/// Validate an optional operator reason: non-empty after trim, bounded, no
/// control characters.
///
/// # Errors
/// Returns a bounded diagnostic if the reason is empty or malformed.
pub fn validate_reason(raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("reason must not be empty".to_owned());
    }
    if trimmed.len() > MAX_REASON_BYTES {
        return Err(format!("reason exceeds {MAX_REASON_BYTES} bytes"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("reason must not contain control characters".to_owned());
    }
    Ok(Some(trimmed.to_owned()))
}

/// Validate an optional idempotency `request_id`: bounded ASCII graphic token.
///
/// # Errors
/// Returns a bounded diagnostic if the token is empty, oversized or contains
/// whitespace/control characters.
pub fn validate_request_id(raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.is_empty() || raw.len() > MAX_REQUEST_ID_BYTES {
        return Err(format!(
            "request_id must be 1..={MAX_REQUEST_ID_BYTES} bytes"
        ));
    }
    if !raw.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err("request_id must contain only ASCII graphic characters".to_owned());
    }
    Ok(Some(raw.to_owned()))
}

/// Deterministic BLAKE3 digest of a JSON value for request idempotency checks.
#[must_use]
pub fn payload_hash(value: &Value) -> String {
    blake3::hash(value.to_string().as_bytes())
        .to_hex()
        .to_string()
}

/// Parse `?include=foreign,expired` on `GET /v1/claims`.
///
/// # Errors
/// Rejects empty queries, unknown parameters, empty/duplicate tokens, and
/// unknown include values.
pub fn parse_claims_query(query: Option<&str>) -> Result<ClaimsInclude, &'static str> {
    let Some(raw) = query else {
        return Ok(ClaimsInclude::default());
    };
    if raw.is_empty() {
        return Err("query must not be empty");
    }
    let Some((key, value)) = raw.split_once('=') else {
        return Err("malformed query parameter");
    };
    if key != "include" || value.contains('&') {
        return Err("only optional include=foreign,expired is accepted on claims route");
    }
    if value.is_empty() {
        return Err("include must name foreign and/or expired");
    }
    let mut parsed = ClaimsInclude::default();
    for token in value.split(',') {
        match token {
            "foreign" => {
                if parsed.foreign {
                    return Err("duplicate include token: foreign");
                }
                parsed.foreign = true;
            }
            "expired" => {
                if parsed.expired {
                    return Err("duplicate include token: expired");
                }
                parsed.expired = true;
            }
            _ => return Err("include accepts only foreign and expired"),
        }
    }
    Ok(parsed)
}

/// Parse `?lines=N` on `GET /v1/attempts/{id}/pane`.
///
/// # Errors
/// Rejects malformed queries, unknown parameters, and counts outside
/// `1..=MAX_PANE_LINES`.
pub fn parse_pane_lines_query(query: Option<&str>) -> Result<u32, &'static str> {
    let Some(raw) = query else {
        return Ok(DEFAULT_PANE_LINES);
    };
    if raw.is_empty() {
        return Err("query must not be empty");
    }
    let Some((key, value)) = raw.split_once('=') else {
        return Err("malformed query parameter");
    };
    if key != "lines" || value.contains('&') {
        return Err("only optional lines=<1..=1000> is accepted on pane route");
    }
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("lines must be a positive integer");
    }
    let lines = value
        .parse::<u32>()
        .map_err(|_| "lines must be within 1..=1000")?;
    if !(1..=MAX_PANE_LINES).contains(&lines) {
        return Err("lines must be within 1..=1000");
    }
    Ok(lines)
}

/// Strip ANSI escape sequences and control characters, keeping at most the
/// trailing `max_lines` lines and at most `max_bytes` bytes.
#[must_use]
pub fn sanitize_pane_output(raw: &str, max_lines: u32, max_bytes: usize) -> SanitizedPane {
    let cleaned = strip_ansi_and_controls(raw);
    let all_lines: Vec<String> = cleaned.lines().map(str::to_owned).collect();
    let max_lines = usize::try_from(max_lines).unwrap_or(usize::MAX).max(1);
    let mut truncated = all_lines.len() > max_lines;
    let start = all_lines.len().saturating_sub(max_lines);
    let mut lines: Vec<String> = all_lines.into_iter().skip(start).collect();

    let joined_len = |slice: &[String]| -> usize {
        if slice.is_empty() {
            0
        } else {
            slice.iter().map(String::len).sum::<usize>() + slice.len().saturating_sub(1)
        }
    };
    while joined_len(&lines) > max_bytes && lines.len() > 1 {
        lines.remove(0);
        truncated = true;
    }
    if lines.len() == 1 && lines[0].len() > max_bytes {
        let mut end = max_bytes;
        while end > 0 && !lines[0].is_char_boundary(end) {
            end -= 1;
        }
        lines[0].truncate(end);
        truncated = true;
    }
    let text = lines.join("\n");
    let line_count = lines.len();
    SanitizedPane {
        lines,
        text,
        line_count,
        truncated,
    }
}

fn strip_ansi_and_controls(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if byte == 0x1b {
            i += 1;
            if i < bytes.len() && bytes[i] == b'[' {
                // CSI sequence: ESC [ ... final byte 0x40..=0x7E
                i += 1;
                while i < bytes.len() {
                    let b = bytes[i];
                    i += 1;
                    if (0x40..=0x7e).contains(&b) {
                        break;
                    }
                }
            } else if i < bytes.len() && bytes[i] == b']' {
                // OSC sequence: ESC ] ... BEL (0x07) or ST (ESC \)
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            } else if i < bytes.len() {
                // Single-character ESC sequence.
                i += 1;
            }
            continue;
        }
        let Some(rest) = raw.get(i..) else {
            break;
        };
        let Some(ch) = rest.chars().next() else {
            break;
        };
        i += ch.len_utf8();
        match ch {
            '\r' => {
                if bytes.get(i) != Some(&b'\n') {
                    out.push('\n');
                }
            }
            '\n' | '\t' => out.push(ch),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Project one machine's health, per-kind slot occupancy, and explicit error.
///
/// Unreachable or unconfigured machines NEVER project `0/0` idle slots:
/// `slots` is `None` (`null`) whenever health is `"unavailable"`.
#[must_use]
pub fn project_machine_slots(
    state: &str,
    inventory_machine: Option<&HerdrMachine>,
    load: Option<&MachineLoad>,
    herdr_configured: bool,
) -> (bool, Option<Value>, Option<Value>) {
    let Some(machine) = inventory_machine else {
        return (
            false,
            None,
            Some(json!({
                "class": "unavailable",
                "code": "placement_not_configured",
                "detail": "machine is not configured in herdr inventory",
            })),
        );
    };
    if !herdr_configured && load.is_none() {
        return (
            false,
            None,
            Some(json!({
                "class": "unavailable",
                "code": "herdr_unconfigured",
                "detail": "herdr inventory is not attached or dispatch is disabled",
            })),
        );
    }
    match state {
        "reporting" => match load {
            Some(MachineLoad::Available(counts)) => {
                let mut slots = serde_json::Map::new();
                for agent in &machine.agents {
                    let used = counts.get(&agent.kind).copied().unwrap_or(0);
                    let capacity = agent.slots;
                    slots.insert(
                        agent.kind.as_str().to_owned(),
                        json!({
                            "used": used,
                            "capacity": capacity,
                            "free": capacity.saturating_sub(used),
                        }),
                    );
                }
                for (kind, used) in counts {
                    if !slots.contains_key(kind.as_str()) {
                        let capacity = machine.slots(*kind);
                        slots.insert(
                            kind.as_str().to_owned(),
                            json!({
                                "used": used,
                                "capacity": capacity,
                                "free": capacity.saturating_sub(*used),
                            }),
                        );
                    }
                }
                (true, Some(Value::Object(slots)), None)
            }
            Some(MachineLoad::Unavailable(failure)) => (
                false,
                None,
                Some(json!({
                    "class": failure.class.as_str(),
                    "code": failure.code,
                    "detail": failure.detail,
                })),
            ),
            None => (
                false,
                None,
                Some(json!({
                    "class": "unavailable",
                    "code": "occupancy_not_observed",
                    "detail": "per-kind occupancy has not been observed yet",
                })),
            ),
        },
        "unreachable" => match load {
            Some(MachineLoad::Unavailable(failure)) => (
                false,
                None,
                Some(json!({
                    "class": failure.class.as_str(),
                    "code": failure.code,
                    "detail": failure.detail,
                })),
            ),
            _ => (
                false,
                None,
                Some(json!({
                    "class": "unavailable",
                    "code": "herdr_unreachable",
                    "detail": "herdr did not answer on this machine",
                })),
            ),
        },
        "stale" => (
            false,
            None,
            Some(json!({
                "class": "unavailable",
                "code": "observation_stale",
                "detail": "machine occupancy observation exceeded its stale window",
            })),
        ),
        _ => (
            false,
            None,
            Some(json!({
                "class": "unavailable",
                "code": "not_reporting",
                "detail": "machine has not reported occupancy yet",
            })),
        ),
    }
}

/// Project static inventory metadata for one configured machine.
#[must_use]
pub fn project_inventory_machine(machine: &HerdrMachine) -> Value {
    let repositories: Vec<&str> = machine.repositories.keys().map(String::as_str).collect();
    let agents: Vec<Value> = machine
        .agents
        .iter()
        .map(|entry| {
            json!({
                "kind": entry.kind.as_str(),
                "slots": entry.slots,
            })
        })
        .collect();
    json!({
        "herdr_machine": machine.herdr_machine,
        "ssh_target": machine.ssh_target,
        "worktree_root": machine.worktree_root,
        "repositories": repositories,
        "agents": agents,
    })
}

/// Return an occupancy snapshot containing only non-draining machines.
#[must_use]
pub fn filter_draining_occupancy(occupancy: &Occupancy, controls: &FleetControls) -> Occupancy {
    Occupancy {
        machines: occupancy
            .machines
            .iter()
            .filter(|(label, _)| !controls.is_draining(label))
            .map(|(label, load)| (label.clone(), load.clone()))
            .collect(),
    }
}

/// Select recently expired (or overdue-open) claims, newest-expired first,
/// bounded by `limit`.
#[must_use]
pub fn select_expired_claims(
    claims: &BTreeMap<ContentId, FleetClaim>,
    now_ms: u64,
    limit: usize,
) -> (Vec<FleetClaim>, bool) {
    let mut expired: Vec<(u64, &FleetClaim)> = claims
        .values()
        .filter_map(|claim| match claim.state {
            ClaimState::Expired { at_ms } => Some((at_ms, claim)),
            ClaimState::Open if claim.expires_ms <= now_ms => Some((claim.expires_ms, claim)),
            _ => None,
        })
        .collect();
    expired.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.identity.as_str().cmp(right.1.identity.as_str()))
    });
    let truncated = expired.len() > limit;
    let rows = expired
        .into_iter()
        .take(limit)
        .map(|(_, claim)| claim.clone())
        .collect();
    (rows, truncated)
}

/// Project one [`ForeignClaim`] for `/v1/claims?include=foreign`.
#[must_use]
pub fn project_foreign_claim(claim: &ForeignClaim, now_ms: u64) -> Value {
    json!({
        "scope": claim.scope,
        "scope_text": claim.scope.text(),
        "message_id": claim.message_id,
        "author": claim.author,
        "holder": claim.author,
        "date_unix_ms": claim.date_unix_ms,
        "age_ms": now_ms.saturating_sub(claim.date_unix_ms),
        "stale_after_ms": FOREIGN_CLAIM_STALE_MS,
        "mirror_status": "foreign",
    })
}

/// Summarize one dispatch tick with bounded deferral rows and reason counts.
#[must_use]
pub fn summarize_tick(
    ticked_at_ms: u64,
    free_slots: u32,
    planned_count: usize,
    performed_count: usize,
    deferred_items: Vec<DeferredItem>,
) -> TickSummary {
    let mut deferral_counts: BTreeMap<String, usize> = BTreeMap::new();
    for item in &deferred_items {
        let count = deferral_counts.entry(item.reason.clone()).or_insert(0);
        *count = count.saturating_add(1);
    }
    let deferred_count = deferred_items.len();
    let deferred = deferred_items.into_iter().take(MAX_DEFERRED_ITEMS).collect();
    TickSummary {
        ticked_at_ms,
        free_slots,
        planned_count,
        performed_count,
        deferred_count,
        deferred,
        deferral_counts,
    }
}

/// Thin imperative persistence shell for [`FleetControls`].
#[derive(Debug)]
pub struct ControlStore {
    path: PathBuf,
    state: Mutex<FleetControls>,
}

impl ControlStore {
    /// Load persisted controls from `<database>.controls.json` when present.
    ///
    /// # Errors
    /// Returns a diagnostic if an existing controls file is unreadable or
    /// malformed.
    pub fn load(database: &Path) -> Result<Self, String> {
        let path = PathBuf::from(format!("{}.controls.json", database.display()));
        let controls = if path.exists() {
            let bytes = read_bounded(&path, MAX_CONTROLS_BYTES)?;
            let parsed: FleetControls = serde_json::from_slice(&bytes)
                .map_err(|error| format!("fleet controls file is malformed: {error}"))?;
            if parsed.schema != CONTROLS_SCHEMA {
                return Err(format!("fleet controls schema must be {CONTROLS_SCHEMA}"));
            }
            parsed
        } else {
            FleetControls::default()
        };
        Ok(Self {
            path,
            state: Mutex::new(controls),
        })
    }

    /// Return a cloned snapshot of current operator controls.
    #[must_use]
    pub fn snapshot(&self) -> FleetControls {
        self.state
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Apply `mutate` to a staged copy of [`FleetControls`], atomically persist
    /// it to disk, and commit in memory only after the disk write succeeds.
    ///
    /// # Errors
    /// Returns a diagnostic if the mutex is poisoned, serialization/write
    /// fails, or `mutate` returns an error.
    pub fn mutate<F, R>(&self, mutate: F) -> Result<R, String>
    where
        F: FnOnce(&mut FleetControls) -> Result<R, String>,
    {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| "fleet controls state lock is poisoned".to_owned())?;
        let mut next = guard.clone();
        let result = mutate(&mut next)?;
        let encoded = serde_json::to_vec_pretty(&next)
            .map_err(|error| format!("cannot encode fleet controls: {error}"))?;
        let tmp = PathBuf::from(format!("{}.tmp.{}", self.path.display(), std::process::id()));
        fs::write(&tmp, &encoded)
            .map_err(|error| format!("cannot write fleet controls: {error}"))?;
        fs::rename(&tmp, &self.path)
            .map_err(|error| format!("cannot persist fleet controls: {error}"))?;
        *guard = next;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_adapter_herdr::{AgentKind, AgentSlots, FailureClass, HerdrFailure, KindCounts};
    use kiln_core::fleet::{
        FleetAgent, FleetScope, FleetTarget, MirrorStatus, ScopePath,
    };

    #[test]
    fn claims_query_accepts_only_foreign_and_expired() {
        assert_eq!(parse_claims_query(None), Ok(ClaimsInclude::default()));
        assert_eq!(
            parse_claims_query(Some("include=foreign")),
            Ok(ClaimsInclude {
                foreign: true,
                expired: false
            })
        );
        assert_eq!(
            parse_claims_query(Some("include=expired")),
            Ok(ClaimsInclude {
                foreign: false,
                expired: true
            })
        );
        assert_eq!(
            parse_claims_query(Some("include=foreign,expired")),
            Ok(ClaimsInclude {
                foreign: true,
                expired: true
            })
        );
        assert_eq!(
            parse_claims_query(Some("include=expired,foreign")),
            Ok(ClaimsInclude {
                foreign: true,
                expired: true
            })
        );
        for bad in [
            "",
            "include",
            "include=",
            "include=all",
            "include=foreign,bogus",
            "include=foreign,foreign",
            "include=foreign,",
            "other=foreign",
            "include=foreign&other=1",
        ] {
            assert!(parse_claims_query(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn pane_lines_query_bounds_are_enforced() {
        assert_eq!(parse_pane_lines_query(None), Ok(DEFAULT_PANE_LINES));
        assert_eq!(parse_pane_lines_query(Some("lines=1")), Ok(1));
        assert_eq!(parse_pane_lines_query(Some("lines=200")), Ok(200));
        assert_eq!(parse_pane_lines_query(Some("lines=1000")), Ok(1000));
        for bad in [
            "",
            "lines",
            "lines=",
            "lines=0",
            "lines=1001",
            "lines=-5",
            "lines=abc",
            "lines=10&extra=1",
            "foo=200",
        ] {
            assert!(parse_pane_lines_query(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn pane_sanitization_strips_ansi_and_bounds_lines_and_bytes() {
        let raw = "\x1b[31merror:\x1b[0m line 1\r\n\x1b]0;title\x07line 2\x00bell\nline 3\nline 4\n";
        let pane = sanitize_pane_output(raw, 2, MAX_PANE_TEXT_BYTES);
        assert_eq!(pane.lines, vec!["line 3", "line 4"]);
        assert_eq!(pane.text, "line 3\nline 4");
        assert_eq!(pane.line_count, 2);
        assert!(pane.truncated);

        let all = sanitize_pane_output(raw, 10, MAX_PANE_TEXT_BYTES);
        assert_eq!(
            all.lines,
            vec!["error: line 1", "line 2 bell", "line 3", "line 4"]
        );
        assert!(!all.truncated);

        let tight = sanitize_pane_output("alpha\nbeta\ngamma\n", 10, 10);
        assert!(tight.truncated);
        assert!(tight.text.len() <= 10);
        assert_eq!(tight.lines, vec!["beta", "gamma"]);
    }

    #[test]
    fn offline_machines_never_report_zero_zero_idle_slots() {
        let machine = HerdrMachine {
            label: "aspen3".to_owned(),
            herdr_machine: Some("aspen3".to_owned()),
            ssh_target: Some("kiln@aspen3".to_owned()),
            worktree_root: "/srv/worktrees".to_owned(),
            repositories: BTreeMap::new(),
            agents: vec![AgentSlots {
                kind: AgentKind::Omp,
                slots: 2,
                args: Vec::new(),
            }],
        };

        let failure = HerdrFailure {
            class: FailureClass::Unavailable,
            code: Some("server_not_running".to_owned()),
            detail: "herdr server not running".to_owned(),
        };
        let (ok, slots, err) = project_machine_slots(
            "unreachable",
            Some(&machine),
            Some(&MachineLoad::Unavailable(failure)),
            true,
        );
        assert!(!ok);
        assert!(slots.is_none());
        assert_eq!(
            err.as_ref().and_then(|v| v.get("code")).and_then(Value::as_str),
            Some("server_not_running")
        );

        let counts = KindCounts::from([(AgentKind::Omp, 1)]);
        let (ok, slots, err) = project_machine_slots(
            "reporting",
            Some(&machine),
            Some(&MachineLoad::Available(counts)),
            true,
        );
        assert!(ok);
        assert!(err.is_none());
        assert_eq!(
            slots.unwrap().pointer("/omp"),
            Some(&json!({ "used": 1, "capacity": 2, "free": 1 }))
        );
    }

    #[test]
    fn drain_and_pause_transitions_and_request_id_dedup_work() {
        let mut controls = FleetControls::default();
        assert!(!controls.is_draining("aspen3"));
        controls.set_machine_drain("aspen3", true, Some("maintenance".into()), "britton-desktop", 10);
        assert!(controls.is_draining("aspen3"));
        assert_eq!(controls.draining_machines(), vec!["aspen3"]);

        let occ = Occupancy {
            machines: BTreeMap::from([
                ("aspen3".into(), MachineLoad::Available(KindCounts::new())),
                ("aspen1".into(), MachineLoad::Available(KindCounts::new())),
            ]),
        };
        let filtered = filter_draining_occupancy(&occ, &controls);
        assert!(!filtered.machines.contains_key("aspen3"));
        assert!(filtered.machines.contains_key("aspen1"));

        controls.set_machine_drain("aspen3", false, None, "britton-desktop", 20);
        assert!(!controls.is_draining("aspen3"));

        controls.set_dispatch_pause(true, Some("incident".into()), "britton-desktop", 30);
        assert!(controls.dispatch_pause.paused);
        assert_eq!(controls.dispatch_pause.reason.as_deref(), Some("incident"));
        controls.set_dispatch_pause(false, None, "britton-desktop", 40);
        assert!(!controls.dispatch_pause.paused);

        let hash1 = payload_hash(&json!({ "text": "hello" }));
        let hash2 = payload_hash(&json!({ "text": "other" }));
        controls.record_request("prompt:a:req1".into(), hash1.clone(), json!({ "ok": true }));
        assert_eq!(
            controls.lookup_request("prompt:a:req1", &hash1),
            Ok(Some(&json!({ "ok": true })))
        );
        assert_eq!(
            controls.lookup_request("prompt:a:req1", &hash2),
            Err(RequestIdConflict)
        );
    }

    #[test]
    fn select_expired_claims_orders_newest_and_bounds() {
        let make = |ch: char, state: ClaimState, expires_ms: u64| FleetClaim {
            identity: ContentId::from_blake3_hex(ch.to_string().repeat(64)).unwrap(),
            scope: FleetScope {
                target: FleetTarget::Repository("onixresearch/kiln".into()),
                path: ScopePath::Whole,
            },
            holder: FleetAgent {
                tool: "omp".into(),
                host: "aspen3".into(),
                session: "s1".into(),
            },
            purpose: "test".into(),
            fence: 1,
            granted_ms: 100,
            expires_ms,
            state,
            attempt: None,
            claim_mirror: MirrorStatus::Pending,
            release_mirror: MirrorStatus::Pending,
        };
        let mut map = BTreeMap::new();
        let c_open = make('a', ClaimState::Open, 5_000);
        let c_overdue = make('b', ClaimState::Open, 900);
        let c_exp1 = make('c', ClaimState::Expired { at_ms: 800 }, 800);
        let c_exp2 = make('d', ClaimState::Expired { at_ms: 950 }, 950);
        map.insert(c_open.identity.clone(), c_open);
        map.insert(c_overdue.identity.clone(), c_overdue.clone());
        map.insert(c_exp1.identity.clone(), c_exp1.clone());
        map.insert(c_exp2.identity.clone(), c_exp2.clone());

        let (rows, truncated) = select_expired_claims(&map, 1_000, 2);
        assert!(truncated);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].identity, c_exp2.identity);
        assert_eq!(rows[1].identity, c_overdue.identity);
    }
}
