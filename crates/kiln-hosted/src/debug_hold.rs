//! Shell for the opt-in microVM debug hold.
//!
//! The job thread that owns a held VM's guest session is the relay: it
//! reserves the hold row, polls for release, capacity pressure and queued
//! debug sessions, relays each command over the held vsock session and writes
//! the bounded report back. Customer actions issue single-use capabilities to
//! project principals with write access, redeem one for exactly one command,
//! release holds and list holds and sessions. Decisions come from the pure
//! `kiln_microvm::debug_hold` core; only digests of capabilities are stored.
use std::io::Read;
use std::time::Duration;

use kiln_microvm::debug_hold::{
    self, CapabilityRecord, DEBUG_COMMAND_TIMEOUT_MS, HoldEnd, HoldFacts, MAX_CAPABILITIES_PER_HOLD,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Deserialize;
use serde_json::{Value, json};

/// Poll interval of the relay and of a waiting `debug_exec` caller.
pub(crate) const HOLD_POLL: Duration = Duration::from_millis(50);
/// Unit runtime added beyond the grace so a held VM outlives its hold.
pub(crate) const HOLD_UNIT_MARGIN_MS: u64 = 2_000;
/// Time the relay waits for the guest's report beyond the command deadline
/// (the guest's drain grace plus framing).
pub(crate) const GUEST_REPLY_MARGIN_MS: u64 = 2_000;
/// Time a `debug_exec` caller waits beyond the command deadline.
pub(crate) const DEBUG_RELAY_MARGIN_MS: u64 = 5_000;
const _: () = assert!(GUEST_REPLY_MARGIN_MS < DEBUG_RELAY_MARGIN_MS);
const _: () =
    assert!(DEBUG_COMMAND_TIMEOUT_MS + DEBUG_RELAY_MARGIN_MS < crate::EXCHANGE_TIMEOUT_MS);
/// Holds and sessions returned by one listing.
const MAX_LISTED: i64 = 128;

const WRITE_ROLES: &[&str] = &["owner", "admin", "member"];
const READ_ROLES: &[&str] = &["owner", "admin", "member", "viewer", "billing"];

pub fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS hosted_microvm_hold(org TEXT NOT NULL,project TEXT NOT NULL,attempt TEXT NOT NULL,job TEXT NOT NULL,runner TEXT NOT NULL,state TEXT NOT NULL,started_ms INTEGER NOT NULL,expires_ms INTEGER NOT NULL,connect TEXT NOT NULL,issued INTEGER NOT NULL,end_reason TEXT,ended_ms INTEGER,PRIMARY KEY(org,attempt,job));
         CREATE TABLE IF NOT EXISTS hosted_microvm_debug_capability(digest TEXT PRIMARY KEY,org TEXT NOT NULL,attempt TEXT NOT NULL,job TEXT NOT NULL,peer TEXT NOT NULL,issued_ms INTEGER NOT NULL,expires_ms INTEGER NOT NULL,used_ms INTEGER);
         CREATE TABLE IF NOT EXISTS hosted_microvm_debug_session(ordinal INTEGER PRIMARY KEY,org TEXT NOT NULL,project TEXT NOT NULL,attempt TEXT NOT NULL,job TEXT NOT NULL,peer TEXT NOT NULL,capability TEXT NOT NULL,command TEXT NOT NULL,command_blake3 TEXT NOT NULL,state TEXT NOT NULL,requested_ms INTEGER NOT NULL,started_ms INTEGER,ended_ms INTEGER,result TEXT);
         CREATE TABLE IF NOT EXISTS hosted_capacity_pressure(singleton INTEGER PRIMARY KEY CHECK(singleton=1),observed_ms INTEGER NOT NULL);",
    )
    .map_err(|e| e.to_string())
}

/// The job a hold belongs to.
pub(crate) struct HeldJob<'a> {
    pub(crate) organization: &'a str,
    pub(crate) project: &'a str,
    pub(crate) attempt: &'a str,
    pub(crate) job: &'a str,
    pub(crate) runner: &'a str,
}

fn storage(error: rusqlite::Error) -> String {
    format!("debug hold storage: {error}")
}

fn integer(value: u64) -> Result<i64, String> {
    crate::sqlite_integer(value)
}

fn unsigned(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

/// Record an admission denied for capacity; holds that started earlier end.
pub(crate) fn record_capacity_pressure(db: &Connection) -> Result<(), String> {
    let now = integer(crate::now_ms()?)?;
    db.execute(
        "INSERT INTO hosted_capacity_pressure VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET observed_ms=MAX(observed_ms,excluded.observed_ms)",
        params![now],
    )
    .map(|_| ())
    .map_err(storage)
}

/// Outcome of trying to start a hold.
pub(crate) enum Begin {
    Held { started_ms: u64, expires_ms: u64 },
    Refused(&'static str),
}

// r[impl kiln.hosted.microvm.debug_hold.lifecycle]
/// Count current holds, ask the lifecycle for `Held` and reserve the hold
/// row in one immediate transaction, so concurrent jobs never exceed the
/// runner's `max_concurrent`.
pub(crate) fn begin(
    db: &Connection,
    held: &HeldJob<'_>,
    facts: impl FnOnce(u32) -> HoldFacts,
    grace_ms: u64,
    advance: impl FnOnce(HoldFacts) -> Result<(), String>,
) -> Result<Begin, String> {
    let tx = Transaction::new_unchecked(db, TransactionBehavior::Immediate).map_err(storage)?;
    let now = crate::now_ms()?;
    let current: u32 = tx
        .query_row(
            "SELECT count(*) FROM hosted_microvm_hold WHERE runner=?1 AND state IN ('held','release_requested') AND expires_ms>?2",
            params![held.runner, integer(now)?],
            |r| r.get(0),
        )
        .map_err(storage)?;
    let facts = facts(current);
    if let Err(refusal) = debug_hold::eligibility(&facts) {
        return Ok(Begin::Refused(refusal.as_str()));
    }
    if advance(facts).is_err() {
        return Ok(Begin::Refused("session_incomplete"));
    }
    let expires_ms = now.saturating_add(grace_ms);
    let connect =
        debug_hold::connection_instructions(held.project, held.attempt, held.job, expires_ms);
    tx.execute(
        "INSERT INTO hosted_microvm_hold VALUES(?1,?2,?3,?4,?5,'held',?6,?7,?8,0,NULL,NULL)",
        params![
            held.organization,
            held.project,
            held.attempt,
            held.job,
            held.runner,
            integer(now)?,
            integer(expires_ms)?,
            connect
        ],
    )
    .map_err(storage)?;
    tx.commit().map_err(storage)?;
    Ok(Begin::Held {
        started_ms: now,
        expires_ms,
    })
}

/// Release requests and capacity pressure observed since the hold started.
pub(crate) fn signals(
    db: &Connection,
    held: &HeldJob<'_>,
    started_ms: u64,
) -> Result<(bool, bool), String> {
    let state: String = db
        .query_row(
            "SELECT state FROM hosted_microvm_hold WHERE org=?1 AND attempt=?2 AND job=?3",
            params![held.organization, held.attempt, held.job],
            |r| r.get(0),
        )
        .map_err(storage)?;
    let pressure: Option<i64> = db
        .query_row(
            "SELECT observed_ms FROM hosted_capacity_pressure WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(storage)?;
    Ok((
        state != "held",
        pressure.is_some_and(|observed| unsigned(observed) >= started_ms),
    ))
}

/// Claim the oldest queued session of this hold for relay.
pub(crate) fn next_request(
    db: &Connection,
    held: &HeldJob<'_>,
) -> Result<Option<(i64, String)>, String> {
    let queued: Option<(i64, String)> = db
        .query_row(
            "SELECT ordinal,command FROM hosted_microvm_debug_session WHERE org=?1 AND attempt=?2 AND job=?3 AND state='requested' ORDER BY ordinal LIMIT 1",
            params![held.organization, held.attempt, held.job],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    let Some((ordinal, command)) = queued else {
        return Ok(None);
    };
    let claimed = db
        .execute(
            "UPDATE hosted_microvm_debug_session SET state='running',started_ms=?2 WHERE ordinal=?1 AND state='requested'",
            params![ordinal, integer(crate::now_ms()?)?],
        )
        .map_err(storage)?;
    Ok((claimed == 1).then_some((ordinal, command)))
}

/// Write the bounded report of one relayed session.
pub(crate) fn finish_request(
    db: &Connection,
    ordinal: i64,
    completed: bool,
    result: &Value,
) -> Result<(), String> {
    db.execute(
        "UPDATE hosted_microvm_debug_session SET state=?2,ended_ms=?3,result=?4 WHERE ordinal=?1 AND state='running'",
        params![
            ordinal,
            if completed { "completed" } else { "failed" },
            integer(crate::now_ms()?)?,
            result.to_string()
        ],
    )
    .map(|_| ())
    .map_err(storage)
}

// r[impl kiln.hosted.microvm.debug_hold.end]
/// Close the hold; queued sessions are refused and capabilities lapse.
pub(crate) fn end(db: &Connection, held: &HeldJob<'_>, reason: HoldEnd) -> Result<(), String> {
    let now = integer(crate::now_ms()?)?;
    db.execute(
        "UPDATE hosted_microvm_hold SET state='ended',end_reason=?4,ended_ms=?5 WHERE org=?1 AND attempt=?2 AND job=?3 AND state<>'ended'",
        params![held.organization, held.attempt, held.job, reason.as_str(), now],
    )
    .map_err(storage)?;
    db.execute(
        "UPDATE hosted_microvm_debug_session SET state='refused',ended_ms=?4 WHERE org=?1 AND attempt=?2 AND job=?3 AND state='requested'",
        params![held.organization, held.attempt, held.job, now],
    )
    .map(|_| ())
    .map_err(storage)
}

/// `execution.recover` retires every hold of a reaped attempt.
pub(crate) fn end_attempt(
    db: &Connection,
    organization: &str,
    attempt: &str,
) -> Result<(), String> {
    let now = integer(crate::now_ms()?)?;
    db.execute(
        "UPDATE hosted_microvm_hold SET state='ended',end_reason='recovered',ended_ms=?3 WHERE org=?1 AND attempt=?2 AND state<>'ended'",
        params![organization, attempt, now],
    )
    .map(|_| ())
    .map_err(storage)
}

/// Sessions relayed for one hold.
pub(crate) fn session_count(db: &Connection, held: &HeldJob<'_>) -> Result<u64, String> {
    db.query_row(
        "SELECT count(*) FROM hosted_microvm_debug_session WHERE org=?1 AND attempt=?2 AND job=?3 AND state IN ('completed','failed')",
        params![held.organization, held.attempt, held.job],
        |r| crate::sqlite_u64(r, 0),
    )
    .map_err(storage)
}

// r[impl kiln.hosted.microvm.debug_hold.sessions]
/// Active holds of one attempt with their connection instructions, for
/// `execution.get` while the attempt is still running.
pub(crate) fn active(
    db: &Connection,
    organization: &str,
    attempt: &str,
) -> Result<Vec<Value>, String> {
    let mut statement = db
        .prepare("SELECT job,expires_ms,connect FROM hosted_microvm_hold WHERE org=?1 AND attempt=?2 AND state='held' ORDER BY job LIMIT ?3")
        .map_err(storage)?;
    statement
        .query_map(params![organization, attempt, MAX_LISTED], |r| {
            Ok(json!({"job": r.get::<_, String>(0)?, "expires_ms": crate::sqlite_u64(r, 1)?, "connect": r.get::<_, String>(2)?}))
        })
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    project: String,
    attempt_id: String,
    job: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Exec {
    project: String,
    attempt_id: String,
    job: String,
    capability: String,
    command: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Listing {
    project: String,
    attempt_id: String,
}

fn write_access(db: &Connection, actor: &crate::Actor, project: &str) -> bool {
    crate::accounts::require_project(db, actor, project, WRITE_ROLES).is_ok()
}

fn hold_row(
    tx: &Connection,
    organization: &str,
    target: (&str, &str, &str),
) -> Result<Option<(String, u64, u32)>, String> {
    let (project, attempt, job) = target;
    tx.query_row(
        "SELECT state,expires_ms,issued FROM hosted_microvm_hold WHERE org=?1 AND project=?2 AND attempt=?3 AND job=?4",
        params![organization, project, attempt, job],
        |r| Ok((r.get(0)?, crate::sqlite_u64(r, 1)?, r.get(2)?)),
    )
    .optional()
    .map_err(storage)
}

fn fresh_capability() -> Result<zeroize::Zeroizing<String>, String> {
    let mut bytes = zeroize::Zeroizing::new([0_u8; debug_hold::CAPABILITY_BYTES]);
    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(bytes.as_mut()))
        .map_err(|_| "capability entropy unavailable")?;
    Ok(zeroize::Zeroizing::new(
        bytes.iter().map(|b| format!("{b:02x}")).collect(),
    ))
}

// r[impl kiln.hosted.microvm.debug_hold.capability]
// r[impl kiln.hosted.microvm.debug_hold.capability.scenario.write_access]
fn issue(db: &mut Connection, actor: &crate::Actor, input: Value) -> Result<Value, String> {
    let target: Target =
        serde_json::from_value(input).map_err(|_| "debug issue request rejected")?;
    let write = write_access(db, actor, &target.project);
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    let now = crate::now_ms()?;
    let row = hold_row(
        &tx,
        &actor.organization,
        (&target.project, &target.attempt_id, &target.job),
    )?;
    let (active, hold_expires, issued) = match &row {
        Some((state, expires, issued)) => (state == "held", *expires, *issued),
        None => (false, 0, MAX_CAPABILITIES_PER_HOLD),
    };
    let expires_ms = debug_hold::issue(now, active, hold_expires, write, issued)?;
    let capability = fresh_capability()?;
    tx.execute(
        "INSERT INTO hosted_microvm_debug_capability VALUES(?1,?2,?3,?4,?5,?6,?7,NULL)",
        params![
            debug_hold::capability_digest(&capability),
            actor.organization,
            target.attempt_id,
            target.job,
            actor.peer,
            integer(now)?,
            integer(expires_ms)?
        ],
    )
    .map_err(storage)?;
    tx.execute(
        "UPDATE hosted_microvm_hold SET issued=issued+1 WHERE org=?1 AND attempt=?2 AND job=?3",
        params![actor.organization, target.attempt_id, target.job],
    )
    .map_err(storage)?;
    tx.commit().map_err(storage)?;
    Ok(json!({
        "capability": capability.as_str(),
        "expires_ms": expires_ms,
        "single_use": true,
        "attempt_id": target.attempt_id,
        "job": target.job,
    }))
}

// r[impl kiln.hosted.microvm.debug_hold.capability.scenario.single_use]
/// Redeem a capability for exactly one command, then wait for the relay.
fn exec(db: &mut Connection, actor: &crate::Actor, input: Value) -> Result<Value, String> {
    let request: Exec = serde_json::from_value(input).map_err(|_| "debug exec request rejected")?;
    debug_hold::admit_command(&request.command)?;
    if !debug_hold::capability_well_formed(&request.capability) {
        return Err("debug capability malformed".into());
    }
    let write = write_access(db, actor, &request.project);
    let digest = debug_hold::capability_digest(&request.capability);
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    let now = crate::now_ms()?;
    let hold = hold_row(
        &tx,
        &actor.organization,
        (&request.project, &request.attempt_id, &request.job),
    )?;
    let active = hold
        .as_ref()
        .is_some_and(|(state, expires, _)| state == "held" && now < *expires);
    let record = tx
        .query_row(
            "SELECT peer,expires_ms,used_ms IS NOT NULL FROM hosted_microvm_debug_capability WHERE digest=?1 AND org=?2 AND attempt=?3 AND job=?4",
            params![digest, actor.organization, request.attempt_id, request.job],
            |r| {
                Ok(CapabilityRecord {
                    peer: r.get(0)?,
                    expires_ms: crate::sqlite_u64(r, 1)?,
                    used: r.get(2)?,
                })
            },
        )
        .optional()
        .map_err(storage)?;
    debug_hold::redeem(record.as_ref(), &actor.peer, write, active, now)?;
    let consumed = tx
        .execute(
            "UPDATE hosted_microvm_debug_capability SET used_ms=?2 WHERE digest=?1 AND used_ms IS NULL",
            params![digest, integer(now)?],
        )
        .map_err(storage)?;
    if consumed != 1 {
        return Err("debug capability already used".into());
    }
    tx.execute(
        "INSERT INTO hosted_microvm_debug_session(org,project,attempt,job,peer,capability,command,command_blake3,state,requested_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'requested',?9)",
        params![
            actor.organization,
            request.project,
            request.attempt_id,
            request.job,
            actor.peer,
            digest,
            request.command,
            blake3::hash(request.command.as_bytes()).to_hex().to_string(),
            integer(now)?
        ],
    )
    .map_err(storage)?;
    let ordinal = tx.last_insert_rowid();
    tx.commit().map_err(storage)?;
    let deadline = std::time::Instant::now()
        + Duration::from_millis(DEBUG_COMMAND_TIMEOUT_MS + DEBUG_RELAY_MARGIN_MS);
    loop {
        if let Some(session) = session(db, ordinal)? {
            return Ok(session);
        }
        if std::time::Instant::now() >= deadline {
            db.execute(
                "UPDATE hosted_microvm_debug_session SET state='abandoned',ended_ms=?2 WHERE ordinal=?1 AND state='requested'",
                params![ordinal, integer(crate::now_ms()?)?],
            )
            .map_err(storage)?;
            return session(db, ordinal)?.ok_or_else(|| {
                "debug session outcome unknown; inspect execution.debug_list".into()
            });
        }
        std::thread::sleep(HOLD_POLL);
    }
}

/// A session in a terminal state, with who, when and how long.
fn session(db: &Connection, ordinal: i64) -> Result<Option<Value>, String> {
    let row: (String, String, i64, Option<i64>, Option<i64>, Option<String>) = db
        .query_row(
            "SELECT state,peer,requested_ms,started_ms,ended_ms,result FROM hosted_microvm_debug_session WHERE ordinal=?1",
            params![ordinal],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .map_err(storage)?;
    let (state, peer, requested, started, ended, result) = row;
    if !matches!(
        state.as_str(),
        "completed" | "failed" | "refused" | "abandoned"
    ) {
        return Ok(None);
    }
    let duration = match (started, ended) {
        (Some(started), Some(ended)) => Some(unsigned(ended.saturating_sub(started))),
        _ => None,
    };
    Ok(Some(json!({
        "session": ordinal,
        "state": state,
        "peer": peer,
        "requested_ms": unsigned(requested),
        "started_ms": started.map(unsigned),
        "ended_ms": ended.map(unsigned),
        "duration_ms": duration,
        "result": result.and_then(|r| serde_json::from_str::<Value>(&r).ok()),
    })))
}

fn release(db: &Connection, actor: &crate::Actor, input: Value) -> Result<Value, String> {
    let target: Target =
        serde_json::from_value(input).map_err(|_| "debug release request rejected")?;
    crate::accounts::require_project(db, actor, &target.project, WRITE_ROLES)?;
    let released = db
        .execute(
            "UPDATE hosted_microvm_hold SET state='release_requested' WHERE org=?1 AND project=?2 AND attempt=?3 AND job=?4 AND state='held'",
            params![actor.organization, target.project, target.attempt_id, target.job],
        )
        .map_err(storage)?;
    Ok(json!({"release_requested": released == 1}))
}

fn list(db: &Connection, actor: &crate::Actor, input: Value) -> Result<Value, String> {
    let listing: Listing =
        serde_json::from_value(input).map_err(|_| "debug list request rejected")?;
    crate::accounts::require_project(db, actor, &listing.project, READ_ROLES)?;
    let mut holds = db
        .prepare("SELECT job,runner,state,started_ms,expires_ms,end_reason,ended_ms,issued,connect FROM hosted_microvm_hold WHERE org=?1 AND project=?2 AND attempt=?3 ORDER BY job LIMIT ?4")
        .map_err(storage)?;
    let holds = holds
        .query_map(
            params![actor.organization, listing.project, listing.attempt_id, MAX_LISTED],
            |r| {
                Ok(json!({
                    "job": r.get::<_, String>(0)?, "runner": r.get::<_, String>(1)?,
                    "state": r.get::<_, String>(2)?, "started_ms": crate::sqlite_u64(r, 3)?,
                    "expires_ms": crate::sqlite_u64(r, 4)?, "end_reason": r.get::<_, Option<String>>(5)?,
                    "ended_ms": r.get::<_, Option<i64>>(6)?.map(unsigned), "capabilities_issued": r.get::<_, u32>(7)?,
                    "connect": r.get::<_, String>(8)?,
                }))
            },
        )
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    let mut ordinals = db
        .prepare("SELECT ordinal FROM hosted_microvm_debug_session WHERE org=?1 AND project=?2 AND attempt=?3 ORDER BY ordinal LIMIT ?4")
        .map_err(storage)?;
    let ordinals = ordinals
        .query_map(
            params![
                actor.organization,
                listing.project,
                listing.attempt_id,
                MAX_LISTED
            ],
            |r| r.get::<_, i64>(0),
        )
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    let mut sessions = Vec::with_capacity(ordinals.len());
    for ordinal in ordinals {
        let (job, state, peer, command_blake3, requested, started, ended): (
            String,
            String,
            String,
            String,
            i64,
            Option<i64>,
            Option<i64>,
        ) = db
            .query_row(
                "SELECT job,state,peer,command_blake3,requested_ms,started_ms,ended_ms FROM hosted_microvm_debug_session WHERE ordinal=?1",
                params![ordinal],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )
            .map_err(storage)?;
        sessions.push(json!({
            "session": ordinal, "job": job, "state": state, "peer": peer, "command_blake3": command_blake3,
            "requested_ms": unsigned(requested), "started_ms": started.map(unsigned), "ended_ms": ended.map(unsigned),
            "duration_ms": started.zip(ended).map(|(s, e)| unsigned(e.saturating_sub(s))),
        }));
    }
    Ok(json!({"holds": holds, "sessions": sessions, "authority": "none"}))
}

/// Customer actions `execution.debug_*`.
pub fn handle(
    db: &mut Connection,
    actor: &crate::Actor,
    action: &str,
    input: Value,
) -> Result<Value, String> {
    match action {
        "debug_issue" => issue(db, actor, input),
        "debug_exec" => exec(db, actor, input),
        "debug_release" => release(db, actor, input),
        "debug_list" => list(db, actor, input),
        _ => Err("unknown execution action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_microvm::debug_hold::Grants;

    const ORG: &str = "org";

    fn peer(digit: char) -> String {
        digit.to_string().repeat(64)
    }

    fn open(path: &std::path::Path) -> Connection {
        let db = Connection::open(path).unwrap_or_else(|e| unreachable!("{e}"));
        db.busy_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| unreachable!("{e}"));
        db
    }

    fn database() -> (tempfile::TempDir, Connection) {
        let directory = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
        let db = open(&directory.path().join("hosted.sqlite"));
        crate::accounts::init(&db).unwrap_or_else(|e| unreachable!("{e}"));
        init(&db).unwrap_or_else(|e| unreachable!("{e}"));
        db.execute(
            "INSERT INTO hosted_organizations VALUES(?1,?2)",
            params![ORG, peer('0')],
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        (directory, db)
    }

    fn member(db: &Connection, peer: &str, role: &str) -> crate::Actor {
        db.execute(
            "INSERT INTO hosted_memberships VALUES(?1,?2,?3,'[\"alpha\"]',1,1,?2)",
            params![ORG, peer, role],
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        crate::Actor {
            peer: peer.into(),
            organization: ORG.into(),
            role: role.into(),
        }
    }

    fn held() -> HeldJob<'static> {
        HeldJob {
            organization: ORG,
            project: "alpha",
            attempt: "a-1",
            job: "check[0]",
            runner: "linux",
        }
    }

    fn facts(held: u32) -> HoldFacts {
        HoldFacts {
            enabled: true,
            failed: true,
            grants: Grants::default(),
            held,
            max_concurrent: 1,
        }
    }

    fn target() -> Value {
        json!({"project": "alpha", "attempt_id": "a-1", "job": "check[0]"})
    }

    fn exec_input(capability: &str) -> Value {
        json!({"project": "alpha", "attempt_id": "a-1", "job": "check[0]", "capability": capability, "command": "id"})
    }

    fn start(db: &Connection) {
        assert!(matches!(
            begin(db, &held(), facts, 60_000, |_| Ok(())),
            Ok(Begin::Held { .. })
        ));
    }

    fn at(value: &Value, pointer: &str) -> Value {
        value.pointer(pointer).cloned().unwrap_or(Value::Null)
    }

    fn capability(issued: &Value) -> String {
        issued
            .get("capability")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }

    // r[verify kiln.hosted.microvm.debug_hold.lifecycle]
    #[test]
    fn concurrent_holds_are_bounded_per_runner_and_end_closes_queued_sessions() {
        let (_directory, db) = database();
        start(&db);
        let second = HeldJob {
            job: "other[0]",
            ..held()
        };
        assert!(matches!(
            begin(&db, &second, facts, 60_000, |_| Ok(())),
            Ok(Begin::Refused("concurrency_limit"))
        ));
        let other_runner = HeldJob {
            runner: "other",
            ..second
        };
        assert!(matches!(
            begin(&db, &other_runner, facts, 60_000, |_| Err("illegal".into())),
            Ok(Begin::Refused("session_incomplete"))
        ));
        end(&db, &held(), HoldEnd::Expired).unwrap_or_else(|e| unreachable!("{e}"));
        let state: (String, String) = db
            .query_row(
                "SELECT state,end_reason FROM hosted_microvm_hold WHERE job='check[0]'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(state, ("ended".into(), "expired".into()));
        assert!(matches!(
            begin(&db, &second, facts, 60_000, |_| Ok(())),
            Ok(Begin::Held { .. })
        ));
    }

    // r[verify kiln.hosted.microvm.debug_hold.capability]
    // r[verify kiln.hosted.microvm.debug_hold.capability.scenario.single_use]
    // r[verify kiln.hosted.microvm.debug_hold.capability.scenario.write_access]
    // r[verify kiln.hosted.microvm.debug_hold.sessions]
    #[test]
    fn capabilities_need_write_access_are_single_use_and_lapse_with_the_hold() {
        let (directory, mut db) = database();
        let owner = member(&db, &peer('a'), "owner");
        let writer = member(&db, &peer('b'), "member");
        let viewer = member(&db, &peer('c'), "viewer");
        assert!(issue(&mut db, &owner, target()).is_err(), "no hold yet");
        start(&db);
        assert!(issue(&mut db, &viewer, target()).is_err(), "viewer");
        let issued = issue(&mut db, &owner, target()).unwrap_or_else(|e| unreachable!("{e}"));
        let token = capability(&issued);
        assert!(debug_hold::capability_well_formed(&token));
        let stored: String = db
            .query_row(
                "SELECT digest FROM hosted_microvm_debug_capability",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_ne!(stored, token, "only the digest is stored");
        // Another principal, with or without write access, cannot redeem it.
        assert_eq!(
            exec(&mut db, &writer, exec_input(&token)),
            Err("debug capability was issued to another principal".into())
        );
        assert_eq!(
            exec(&mut db, &viewer, exec_input(&token)),
            Err("debug capability requires project write access".into())
        );
        // The relay answers the queued session on its own connection.
        let relay_path = directory.path().join("hosted.sqlite");
        let relay = std::thread::spawn(move || {
            let db = open(&relay_path);
            loop {
                if let Some((ordinal, command)) =
                    next_request(&db, &held()).unwrap_or_else(|e| unreachable!("{e}"))
                {
                    assert_eq!(command, "id");
                    finish_request(&db, ordinal, true, &json!({"exit_code": 0}))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    return;
                }
                std::thread::sleep(HOLD_POLL);
            }
        });
        let first =
            exec(&mut db, &owner, exec_input(&token)).unwrap_or_else(|e| unreachable!("{e}"));
        relay.join().unwrap_or_else(|_| unreachable!("relay"));
        assert_eq!(at(&first, "/state"), "completed");
        assert_eq!(at(&first, "/peer"), peer('a'));
        assert_eq!(at(&first, "/result/exit_code"), 0);
        assert!(at(&first, "/duration_ms").is_u64());
        assert_eq!(
            exec(&mut db, &owner, exec_input(&token)),
            Err("debug capability already used".into())
        );
        let listed = list(
            &db,
            &viewer,
            json!({"project": "alpha", "attempt_id": "a-1"}),
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(at(&listed, "/sessions").as_array().map(Vec::len), Some(1));
        assert_eq!(at(&listed, "/holds/0/capabilities_issued"), 1);
        assert_eq!(at(&listed, "/authority"), "none");
        // A capability issued before release lapses with the hold.
        let pending = issue(&mut db, &owner, target()).unwrap_or_else(|e| unreachable!("{e}"));
        assert!(release(&db, &viewer, target()).is_err(), "viewer release");
        let released = release(&db, &writer, target()).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(at(&released, "/release_requested"), true);
        assert_eq!(signals(&db, &held(), 0), Ok((true, false)));
        assert_eq!(
            exec(&mut db, &owner, exec_input(&capability(&pending))),
            Err("debug capability expired with its hold".into())
        );
        assert!(
            issue(&mut db, &owner, target()).is_err(),
            "no new capability"
        );
        assert!(
            active(&db, ORG, "a-1")
                .unwrap_or_else(|e| unreachable!("{e}"))
                .is_empty()
        );
    }

    // r[verify kiln.hosted.microvm.debug_hold.capability.scenario.single_use]
    #[test]
    fn expired_capabilities_are_refused() {
        let (_directory, mut db) = database();
        let owner = member(&db, &peer('a'), "owner");
        start(&db);
        let issued = issue(&mut db, &owner, target()).unwrap_or_else(|e| unreachable!("{e}"));
        db.execute(
            "UPDATE hosted_microvm_debug_capability SET expires_ms=1",
            [],
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            exec(&mut db, &owner, exec_input(&capability(&issued))),
            Err("debug capability expired with its hold".into())
        );
        let unknown = "0".repeat(64);
        assert_eq!(
            exec(&mut db, &owner, exec_input(&unknown)),
            Err("debug capability unknown".into())
        );
        assert!(exec(&mut db, &owner, exec_input("not-hex")).is_err());
        let sessions: i64 = db
            .query_row(
                "SELECT count(*) FROM hosted_microvm_debug_session",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(sessions, 0, "refused capabilities queue nothing");
    }

    // r[verify kiln.hosted.microvm.debug_hold.end]
    #[test]
    fn capacity_pressure_after_the_hold_started_ends_it() {
        let (_directory, db) = database();
        start(&db);
        let started: i64 = db
            .query_row("SELECT started_ms FROM hosted_microvm_hold", [], |r| {
                r.get(0)
            })
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(signals(&db, &held(), unsigned(started)), Ok((false, false)));
        record_capacity_pressure(&db).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(signals(&db, &held(), unsigned(started)), Ok((false, true)));
        assert_eq!(signals(&db, &held(), u64::MAX), Ok((false, false)));
    }
}
