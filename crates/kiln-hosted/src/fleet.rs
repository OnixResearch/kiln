use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use bounded_exec::{
    CommandSpec, Completion, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Only a reviewed immutable local executable is admitted. This is a protocol
/// boundary, not a built-in cloud provider or permission to purchase capacity.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Executable {
    pub path: PathBuf,
    pub blake3: String,
}
impl Executable {
    pub fn verify(&self) -> Result<PathBuf, String> {
        let path = self
            .path
            .canonicalize()
            .map_err(|_| "reviewed executable unavailable")?;
        if !self.path.is_absolute()
            || !self.path.starts_with("/nix/store")
            || self.path.components().any(|part| {
                !matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
            || !path.starts_with("/nix/store")
            || self.blake3.len() != 64
        {
            return Err("executable must have an immutable Nix-store path and exact BLAKE3".into());
        }
        let metadata = std::fs::metadata(&path).map_err(|_| "executable unavailable")?;
        if !metadata.is_file() || metadata.len() > 268_435_456 {
            return Err("executable size/type rejected".into());
        }
        use std::io::Read;
        let mut source = std::fs::File::open(&path).map_err(|_| "executable unavailable")?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0_u8; 65536];
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|_| "executable read failed")?;
            if count == 0 {
                break;
            }
            hasher.update(buffer.get(..count).ok_or("executable read count invalid")?);
        }
        if hasher.finalize().to_hex().as_str() != self.blake3 {
            return Err("reviewed executable changed".into());
        }
        // Preserve the immutable admitted basename: multicall binaries select
        // their operation from argv[0], not the canonical target's filename.
        Ok(self.path.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pricing {
    pub price_id: String,
    pub class: String,
    pub max_units: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub driver: Executable,
    pub organizations: Vec<String>,
    pub pricing: Option<Pricing>,
    pub minimum: u32,
    pub maximum: u32,
    pub slots_per_instance: u32,
    pub timeout_ms: u64,
}
impl Config {
    fn validate(&self) -> Result<(), String> {
        if self.minimum > self.maximum
            || self.maximum == 0
            || self.maximum > 128
            || self.slots_per_instance == 0
            || self.slots_per_instance > 256
            || !(100..=300_000).contains(&self.timeout_ms)
        {
            return Err("fleet policy outside finite bounds".into());
        }
        if self.organizations.is_empty()
            || self.organizations.len() > 128
            || self.organizations.iter().any(|v| !crate::identifier(v))
        {
            return Err("operator fleet organization allowlist required".into());
        }
        if self.pricing.as_ref().is_some_and(|p| {
            !crate::identifier(&p.price_id)
                || !crate::identifier(&p.class)
                || p.max_units == 0
                || p.max_units > 1_000_000_000
        }) {
            return Err("invalid operator fleet pricing".into());
        }
        self.driver.verify()?;
        Ok(())
    }
}

pub fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_fleet_config(org TEXT PRIMARY KEY, revision INTEGER NOT NULL, config TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS hosted_fleet_operator(singleton INTEGER PRIMARY KEY CHECK(singleton=1),config TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS hosted_fleet_instances(org TEXT NOT NULL,id TEXT NOT NULL,request_id TEXT NOT NULL,identity TEXT NOT NULL,state TEXT NOT NULL,operation TEXT NOT NULL,revision INTEGER NOT NULL,receipt TEXT,PRIMARY KEY(org,id),UNIQUE(org,request_id));
      CREATE TABLE IF NOT EXISTS hosted_fleet_requests(org TEXT NOT NULL,request_id TEXT NOT NULL,identity TEXT NOT NULL,result TEXT,PRIMARY KEY(org,request_id));
      CREATE TABLE IF NOT EXISTS hosted_fleet_timing(org TEXT NOT NULL,job TEXT NOT NULL,queued_ms INTEGER NOT NULL,cache_ms INTEGER,observed_ms INTEGER NOT NULL,cache_outcome TEXT NOT NULL,PRIMARY KEY(org,job));")
        .map_err(|e| e.to_string())?;
    let current:u32=db.query_row("SELECT count(*) FROM pragma_table_info('hosted_fleet_timing') WHERE name='cache_outcome'",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    if current == 0 {
        let tx = db.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute_batch("ALTER TABLE hosted_fleet_timing RENAME TO hosted_fleet_timing_previous;
          CREATE TABLE hosted_fleet_timing(org TEXT NOT NULL,job TEXT NOT NULL,queued_ms INTEGER NOT NULL,cache_ms INTEGER,observed_ms INTEGER NOT NULL,cache_outcome TEXT NOT NULL,PRIMARY KEY(org,job));
          INSERT INTO hosted_fleet_timing SELECT org,job,queued_ms,NULL,observed_ms,'disabled_legacy' FROM hosted_fleet_timing_previous;
          DROP TABLE hosted_fleet_timing_previous;").map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Operator-only startup admission; customers cannot choose executable paths.
pub fn configure_trusted(db: &mut Connection, config: &Config) -> Result<(), String> {
    config.validate()?;
    let old: Option<String> = db
        .query_row(
            "SELECT config FROM hosted_fleet_operator WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let encoded = serde_json::to_string(config).map_err(|e| e.to_string())?;
    if old.as_ref().is_some_and(|v| v != &encoded) {
        let occupied: u64 = db
            .query_row(
                "SELECT count(*) FROM hosted_fleet_instances WHERE state!='retired'",
                [],
                |r| crate::sqlite_u64(r, 0),
            )
            .map_err(|e| e.to_string())?;
        if occupied > 0 {
            return Err("fleet operator profile pinned by active/uncertain instances".into());
        }
    }
    db.execute("INSERT INTO hosted_fleet_operator VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET config=excluded.config",[encoded]).map_err(|e|e.to_string())?;
    Ok(())
}

pub fn configure(
    db: &mut Connection,
    actor: &crate::Actor,
    config: Config,
    expected_revision: u64,
) -> Result<Value, String> {
    crate::require(actor, &["owner", "admin"])?;
    config.validate()?;
    let raw: String = db
        .query_row(
            "SELECT config FROM hosted_fleet_operator WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "operator fleet driver not configured")?;
    let admitted: Config = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if !admitted.organizations.contains(&actor.organization) {
        return Err("organization is not operator-admitted for fleet driver".into());
    }
    if config.driver.path != admitted.driver.path
        || config.driver.blake3 != admitted.driver.blake3
        || config.minimum < admitted.minimum
        || config.maximum > admitted.maximum
        || config.slots_per_instance > admitted.slots_per_instance
        || config.timeout_ms != admitted.timeout_ms
    {
        return Err("fleet selection exceeds operator profile".into());
    }
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let revision: u64 = tx
        .query_row(
            "SELECT revision FROM hosted_fleet_config WHERE org=?1",
            [&actor.organization],
            |r| crate::sqlite_u64(r, 0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    if revision != expected_revision {
        return Err("stale fleet revision".into());
    }
    let occupied: u32 = tx
        .query_row(
            "SELECT count(*) FROM hosted_fleet_instances WHERE org=?1 AND state!='retired'",
            [&actor.organization],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if occupied > config.maximum {
        return Err("unknown and active instances retain capacity".into());
    }
    let next = revision.checked_add(1).ok_or("revision overflow")?;
    tx.execute("INSERT INTO hosted_fleet_config VALUES(?1,?2,?3) ON CONFLICT(org) DO UPDATE SET revision=excluded.revision,config=excluded.config", params![actor.organization,crate::sqlite_integer(next)?,serde_json::to_string(&config).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(json!({"revision":next}))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    instance_id: String,
    request_id: String,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverReceipt {
    schema: String,
    organization: String,
    request_id: String,
    instance_id: String,
    state: String,
}

fn mutate(
    db: &mut Connection,
    actor: &crate::Actor,
    operation: &str,
    input: Value,
    now_ms: u64,
) -> Result<Value, String> {
    crate::require(actor, &["owner", "admin"])?;
    let request: Mutation = serde_json::from_value(input).map_err(|e| e.to_string())?;
    if !crate::identifier(&request.instance_id) || !crate::identifier(&request.request_id) {
        return Err("invalid fleet identity".into());
    }
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let raw: String = tx
        .query_row(
            "SELECT config FROM hosted_fleet_config WHERE org=?1",
            [&actor.organization],
            |r| r.get(0),
        )
        .map_err(|_| "fleet not configured")?;
    let config: Config = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    config.validate()?;
    let operator_raw: String = tx
        .query_row(
            "SELECT config FROM hosted_fleet_operator WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "operator fleet profile missing")?;
    let operator: Config = serde_json::from_str(&operator_raw).map_err(|e| e.to_string())?;
    if !operator.organizations.contains(&actor.organization)
        || operator.driver.path != config.driver.path
        || operator.driver.blake3 != config.driver.blake3
    {
        return Err("operator fleet authority changed; reconfigure before new effects".into());
    }
    if !config.organizations.contains(&actor.organization) {
        return Err("organization is not operator-admitted for fleet driver".into());
    }
    if operation == "provision" && config.pricing.is_none() {
        return Err("operator fleet price required before provisioning".into());
    }
    let prior: Option<(String,String,String,u64,Option<String>)> = tx.query_row("SELECT request_id,identity,state,revision,receipt FROM hosted_fleet_instances WHERE org=?1 AND id=?2", params![actor.organization,request.instance_id], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,crate::sqlite_u64(r,3)?,r.get(4)?))).optional().map_err(|e|e.to_string())?;
    let identity = crate::identity(
        &json!({"operation":operation,"instance":request.instance_id,"request":request.request_id,"revision":request.expected_revision}),
    )?;
    if operation != "reconcile" {
        let retained: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT identity,result FROM hosted_fleet_requests WHERE org=?1 AND request_id=?2",
                params![actor.organization, request.request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((old, result)) = retained {
            if old != identity {
                return Err("fleet request identity conflict".into());
            }
            return result
                .map(|r| serde_json::from_str(&r).map_err(|e| e.to_string()))
                .unwrap_or_else(|| Ok(json!({"state":"unknown","rerun":false})));
        }
    }
    let occupied: u32 = tx
        .query_row(
            "SELECT count(*) FROM hosted_fleet_instances WHERE org=?1 AND state!='retired'",
            [&actor.organization],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let global: u32 = tx
        .query_row(
            "SELECT count(*) FROM hosted_fleet_instances WHERE state!='retired'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let active: u32 = tx
        .query_row(
            "SELECT count(*) FROM hosted_fleet_instances WHERE org=?1 AND state='active'",
            [&actor.organization],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let (driver_key, next) = match operation {
        "provision" => {
            if prior.is_some() || request.expected_revision != 0 {
                return Err("fleet new-instance revision denied".into());
            }
            admit_capacity(occupied, config.maximum, global, operator.maximum)?;
            tx.execute("INSERT INTO hosted_fleet_instances VALUES(?1,?2,?3,?4,'provisioning','provision',1,NULL)", params![actor.organization,request.instance_id,request.request_id,identity]).map_err(|e|e.to_string())?;
            (request.request_id.clone(), 1)
        }
        "deprovision" => {
            let (_, _, state, revision, _) = prior.as_ref().ok_or("instance absent")?;
            if state != "active"
                || *revision != request.expected_revision
                || active <= config.minimum
            {
                return Err("fleet minimum, uncertain state or stale revision denied".into());
            }
            let next = revision.checked_add(1).ok_or("revision overflow")?;
            tx.execute("UPDATE hosted_fleet_instances SET request_id=?3,identity=?4,state='deprovisioning',operation='deprovision',revision=?5,receipt=NULL WHERE org=?1 AND id=?2", params![actor.organization,request.instance_id,request.request_id,identity,crate::sqlite_integer(next)?]).map_err(|e|e.to_string())?;
            (request.request_id.clone(), next)
        }
        "reconcile" => {
            let (key, _, state, revision, _) = prior.as_ref().ok_or("instance absent")?;
            if !["unknown", "provisioning", "deprovisioning"].contains(&state.as_str())
                || *revision != request.expected_revision
            {
                return Err("no uncertain current fleet request".into());
            }
            (key.clone(), *revision)
        }
        _ => return Err("unknown fleet operation".into()),
    };
    if operation != "reconcile" {
        tx.execute(
            "INSERT INTO hosted_fleet_requests VALUES(?1,?2,?3,NULL)",
            params![actor.organization, request.request_id, identity],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    if operation == "provision" {
        let price = config
            .pricing
            .as_ref()
            .ok_or("operator fleet pricing absent")?;
        let attempt = crate::identity(
            &json!({"kind":"fleet","organization":actor.organization,"instance":request.instance_id,"revision":next}),
        )?;
        let admission = crate::finance::reserve(
            db,
            actor,
            json!({"attempt_id":attempt,"price_id":price.price_id,"class":price.class,"max_units":price.max_units,"binding":{"fleet_instance":request.instance_id,"revision":next}}),
            now_ms,
        );
        if let Err(error) = admission {
            let result =
                json!({"instance_id":request.instance_id,"state":"not_dispatched","reason":error});
            db.execute("UPDATE hosted_fleet_instances SET state='retired',receipt=?3 WHERE org=?1 AND id=?2",params![actor.organization,request.instance_id,result.to_string()]).map_err(|e|e.to_string())?;
            db.execute(
                "UPDATE hosted_fleet_requests SET result=?3 WHERE org=?1 AND request_id=?2",
                params![actor.organization, request.request_id, result.to_string()],
            )
            .map_err(|e| e.to_string())?;
            return Err(error);
        }
        crate::finance::observe_unknown_trusted(
            db,
            &actor.organization,
            &attempt,
            &identity,
            &identity,
            now_ms,
        )?;
    }
    let payload = json!({"schema":"kiln.hosted.fleet-driver.v1","organization":actor.organization,"request_id":driver_key,"instance_id":request.instance_id,"operation":if operation=="reconcile" {"inspect"} else {operation},"slots":config.slots_per_instance});
    let started = Instant::now();
    let observed = bounded_exec::run(RunRequest {
        command: CommandSpec {
            program: config.driver.verify()?,
            args: vec![OsString::from("--kiln-fleet-v1")],
            current_dir: Path::new("/").to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment: vec![],
            input: Input::Bytes(serde_json::to_vec(&payload).map_err(|e| e.to_string())?),
        },
        limits: ExecutionLimits {
            timeout_ms: config.timeout_ms,
            stdin_max_bytes: 16384,
            stdout_max_bytes: 16384,
            stderr_max_bytes: 4096,
            poll_interval_ms: 10,
            teardown_timeout_ms: 2000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true).map_err(|e| format!("{e:?}"))?,
    });
    let receipt = observed
        .ok()
        .filter(|o| {
            o.completion == Completion::Exited
                && o.exit_code == Some(0)
                && !o.stdout.truncated
                && !o.stderr.truncated
        })
        .and_then(|o| serde_json::from_slice::<DriverReceipt>(&o.stdout.bytes).ok())
        .filter(|r| {
            r.schema == "kiln.hosted.fleet-driver.v1"
                && r.organization == actor.organization
                && r.request_id == driver_key
                && r.instance_id == request.instance_id
                && ["present", "absent", "unknown"].contains(&r.state.as_str())
        });
    let state = match receipt.as_ref().map(|r| r.state.as_str()) {
        Some("present") if operation != "deprovision" => "active",
        Some("absent") => "retired",
        _ => "unknown",
    };
    let result = json!({"instance_id":request.instance_id,"request_id":driver_key,"state":state,"revision":next,"observation_ms":u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),"driver_state":receipt.as_ref().map(|r|&r.state)});
    db.execute("UPDATE hosted_fleet_instances SET state=?3,receipt=?4 WHERE org=?1 AND id=?2 AND revision=?5",params![actor.organization,request.instance_id,state,result.to_string(),crate::sqlite_integer(next)?]).map_err(|e|e.to_string())?;
    if operation != "reconcile" {
        db.execute(
            "UPDATE hosted_fleet_requests SET result=?3 WHERE org=?1 AND request_id=?2",
            params![actor.organization, request.request_id, result.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(result)
}

// r[impl kiln.hosted.execution]
fn admit_capacity(
    occupied: u32,
    maximum: u32,
    global: u32,
    operator_maximum: u32,
) -> Result<(), String> {
    if occupied >= maximum || global >= operator_maximum {
        Err("fleet capacity exhausted; uncertain instances retain slots".into())
    } else {
        Ok(())
    }
}
/// Host-measured timing only; not exposed as a customer mutation.
pub fn record_timing(
    db: &Connection,
    organization: &str,
    job: &str,
    queued_ms: u64,
    cache_ms: Option<u64>,
    now_ms: u64,
    cache_outcome: &str,
) -> Result<(), String> {
    if !crate::identifier(job)
        || cache_outcome.is_empty()
        || cache_outcome.len() > 64
        || cache_ms.is_some_and(|v| v > i64::MAX as u64)
        || [queued_ms, now_ms].iter().any(|v| *v > i64::MAX as u64)
    {
        return Err("timing bounds rejected".into());
    }
    db.execute("INSERT INTO hosted_fleet_timing VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(org,job) DO UPDATE SET queued_ms=excluded.queued_ms,cache_ms=excluded.cache_ms,observed_ms=excluded.observed_ms,cache_outcome=excluded.cache_outcome",params![organization,job,crate::sqlite_integer(queued_ms)?,cache_ms.map(crate::sqlite_integer).transpose()?,crate::sqlite_integer(now_ms)?,cache_outcome]).map_err(|e|e.to_string())?;
    Ok(())
}

pub fn handle(
    db: &mut Connection,
    actor: &crate::Actor,
    action: &str,
    input: Value,
    now_ms: u64,
) -> Result<Value, String> {
    match action {
        "configure" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Request {
                minimum: u32,
                maximum: u32,
                slots_per_instance: u32,
                expected_revision: u64,
            }
            let r: Request = serde_json::from_value(input).map_err(|e| e.to_string())?;
            let raw: String = db
                .query_row(
                    "SELECT config FROM hosted_fleet_operator WHERE singleton=1",
                    [],
                    |r| r.get(0),
                )
                .map_err(|_| "operator fleet driver not configured")?;
            let mut config: Config = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            config.minimum = r.minimum;
            config.maximum = r.maximum;
            config.slots_per_instance = r.slots_per_instance;
            configure(db, actor, config, r.expected_revision)
        }
        "provision" | "deprovision" | "reconcile" => mutate(db, actor, action, input, now_ms),
        // r[impl kiln.hosted.qualification]
        "status" => {
            crate::require(actor, &["owner", "admin", "member", "viewer", "billing"])?;
            let mut stmt=db.prepare("SELECT id,state,revision FROM hosted_fleet_instances WHERE org=?1 ORDER BY id LIMIT 129").map_err(|e|e.to_string())?;
            let rows=stmt.query_map([&actor.organization],|r|Ok(json!({"instance_id":r.get::<_,String>(0)?,"state":r.get::<_,String>(1)?,"revision":crate::sqlite_u64(r,2)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
            if rows.len() > 128 {
                return Err("fleet history requires archival before listing".into());
            }
            let mut timing=db.prepare("SELECT job,queued_ms,cache_ms,cache_outcome FROM hosted_fleet_timing WHERE org=?1 ORDER BY observed_ms DESC LIMIT 100").map_err(|e|e.to_string())?;
            let timings=timing.query_map([&actor.organization],|r|Ok(json!({"job":r.get::<_,String>(0)?,"queued_ms":crate::sqlite_u64(r,1)?,"cache_ms":if r.get_ref(2)?.data_type()==rusqlite::types::Type::Null {None} else {Some(crate::sqlite_u64(r,2)?)},"cache_outcome":r.get::<_,String>(3)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
            Ok(json!({"instances":rows,"timings":timings,"production_provider_qualified":false}))
        }
        _ => Err("unknown fleet action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_capacity_and_immutable_driver_are_required() {
        let config = Config {
            driver: Executable {
                path: "/tmp/unreviewed".into(),
                blake3: "0".repeat(64),
            },
            organizations: vec!["org".into()],
            pricing: None,
            minimum: 2,
            maximum: 1,
            slots_per_instance: 1,
            timeout_ms: 100,
        };
        assert!(config.validate().is_err());
        let mut c = config;
        c.minimum = 0;
        assert!(c.validate().is_err());
        assert!(admit_capacity(0, 2, 1, 2).is_ok());
        assert!(admit_capacity(2, 2, 2, 4).is_err());
        assert!(admit_capacity(0, 2, 2, 2).is_err());
    }
    #[test]
    fn timing_is_tenant_scoped_and_bounded() {
        let db = Connection::open_in_memory()
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        init(&db).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        record_timing(&db, "a", "job", 10, Some(20), 100, "hit")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        record_timing(&db, "b", "job", 30, None, 100, "disabled")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let first: u64 = db
            .query_row(
                "SELECT queued_ms FROM hosted_fleet_timing WHERE org='a'",
                [],
                |r| crate::sqlite_u64(r, 0),
            )
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(first, 10);
        assert!(record_timing(&db, "a", "job", u64::MAX, None, 0, "disabled").is_err());
        let disabled: Option<i64> = db
            .query_row(
                "SELECT cache_ms FROM hosted_fleet_timing WHERE org='b'",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|error| unreachable!("fixture observation exists: {error:?}"));
        assert_eq!(disabled, None);
    }
    #[test]
    fn legacy_cache_zero_is_migrated_to_explicit_unmeasured_timing() {
        let db = Connection::open_in_memory()
            .unwrap_or_else(|error| unreachable!("fixture database: {error:?}"));
        db.execute_batch("CREATE TABLE hosted_fleet_timing(org TEXT NOT NULL,job TEXT NOT NULL,queued_ms INTEGER NOT NULL,cache_ms INTEGER NOT NULL,observed_ms INTEGER NOT NULL,PRIMARY KEY(org,job)); INSERT INTO hosted_fleet_timing VALUES('org','job',23,0,100);").unwrap_or_else(|error|unreachable!("legacy fixture: {error:?}"));
        init(&db).unwrap_or_else(|error| unreachable!("timing migration: {error:?}"));
        let observed:(i64,Option<i64>,String)=db.query_row("SELECT queued_ms,cache_ms,cache_outcome FROM hosted_fleet_timing WHERE org='org' AND job='job'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap_or_else(|error|unreachable!("migrated observation: {error:?}"));
        assert_eq!(observed, (23, None, "disabled_legacy".into()));
    }
}
