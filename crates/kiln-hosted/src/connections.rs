//! Durable hosted repository bindings. Only startup admission accepts filesystem paths.
use crate::{Actor, accounts};
use base64::Engine;
use kiln_adapter_github::{
    events::{DeliveryHeaders, admit_event},
    profile::{
        AdmittedGithubConfig, GithubConfig, MAXIMUM_PROFILE_BYTES, admit_config, load_config,
    },
    secret::resolve_secret,
    server::{open_store, runtime},
    service::plan_event,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

const MAX_PROFILES: i64 = 128;
const MAX_CONNECTIONS: i64 = 1024;
const MAX_GENERATIONS: i64 = 1024;
const MAX_ROWS: usize = 128;
const MAX_HOSTED_RUNS: usize = 128;
const MAX_HOSTED_BODY_BYTES: usize = 1_048_576;
const MAX_HOSTED_RECORD_BYTES: usize = 1_048_576;

fn storage(_: rusqlite::Error) -> String {
    "connection storage failure".into()
}
fn provider(error: kiln_provider_api::PortFailure) -> String {
    use kiln_provider_api::PortFailure;
    match error {
        PortFailure::Domain(e) => e.code,
        PortFailure::Infrastructure(e)
        | PortFailure::Retryable(e)
        | PortFailure::TimedOut(e)
        | PortFailure::Cancelled(e)
        | PortFailure::Unknown(e) => e.code,
    }
}
fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn names(project: &str, connection: &str) -> Result<(), String> {
    if !crate::identifier(project) || !crate::identifier(connection) {
        return Err("invalid project or connection identity".into());
    }
    Ok(())
}
fn profile_bounds(profile: &AdmittedGithubConfig) -> Result<(), String> {
    if profile.config.maximum_run_records > MAX_HOSTED_RUNS
        || profile.config.maximum_delivery_records > MAX_HOSTED_RUNS * 16
        || profile.config.maximum_body_bytes > MAX_HOSTED_BODY_BYTES
        || profile.config.maximum_record_bytes > MAX_HOSTED_RECORD_BYTES
    {
        return Err("GitHub profile exceeds hosted runtime bounds".into());
    }
    Ok(())
}

pub fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_source_profiles(blake3 TEXT PRIMARY KEY,path TEXT NOT NULL,bytes BLOB NOT NULL,adapter_identity TEXT NOT NULL,active INTEGER NOT NULL CHECK(active IN(0,1)));
    CREATE TABLE IF NOT EXISTS hosted_source_profile_scopes(blake3 TEXT NOT NULL REFERENCES hosted_source_profiles(blake3),organization TEXT NOT NULL,project TEXT NOT NULL,active INTEGER NOT NULL CHECK(active IN(0,1)),PRIMARY KEY(blake3,organization,project));
    CREATE TABLE IF NOT EXISTS hosted_connections(organization TEXT NOT NULL,project TEXT NOT NULL,connection TEXT NOT NULL,repository_id TEXT NOT NULL,owner TEXT NOT NULL,name TEXT NOT NULL,generation INTEGER NOT NULL,revision INTEGER NOT NULL,active INTEGER NOT NULL,changed_by TEXT NOT NULL,PRIMARY KEY(organization,project,connection));
    CREATE TABLE IF NOT EXISTS hosted_connection_generations(organization TEXT NOT NULL,project TEXT NOT NULL,connection TEXT NOT NULL,generation INTEGER NOT NULL,profile_blake3 TEXT NOT NULL REFERENCES hosted_source_profiles(blake3),config BLOB NOT NULL,adapter_identity TEXT NOT NULL,PRIMARY KEY(organization,project,connection,generation));
    CREATE TABLE IF NOT EXISTS hosted_source_deliveries(organization TEXT NOT NULL,project TEXT NOT NULL,connection TEXT NOT NULL,delivery TEXT NOT NULL,body_identity TEXT NOT NULL,generation INTEGER NOT NULL,run_identity TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN('reserved','admitted')),PRIMARY KEY(organization,project,connection,delivery));") .map_err(storage)
}

/// Call once at startup before admitting the current operator profile list.
pub fn begin_profile_configuration(db: &Connection) -> Result<(), String> {
    db.execute_batch("UPDATE hosted_source_profiles SET active=0; UPDATE hosted_source_profile_scopes SET active=0;").map_err(storage)?;
    Ok(())
}

/// Startup-only credential grant: knowing a digest never grants another tenant's profile.
pub fn admit_profile_scope(
    db: &Connection,
    digest: &str,
    organization: &str,
    project: &str,
) -> Result<(), String> {
    if !digest_valid(digest) || !crate::identifier(organization) || !crate::identifier(project) {
        return Err("invalid source profile scope".into());
    }
    admitted_profile(db, digest, false)?;
    db.execute("INSERT INTO hosted_source_profile_scopes VALUES(?1,?2,?3,1) ON CONFLICT(blake3,organization,project) DO UPDATE SET active=1",params![digest,organization,project]).map_err(storage)?;
    Ok(())
}

fn profile_scope(
    db: &Connection,
    digest: &str,
    organization: &str,
    project: &str,
) -> Result<(), String> {
    let allowed:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM hosted_source_profile_scopes WHERE blake3=?1 AND organization=?2 AND project=?3 AND active=1)",params![digest,organization,project],|r|r.get(0)).map_err(storage)?;
    if !allowed {
        return Err("source credentials not admitted for organization/project".into());
    }
    Ok(())
}

fn read_profile(path: &Path, digest: &str) -> Result<(Vec<u8>, AdmittedGithubConfig), String> {
    if !path.is_absolute() || !digest_valid(digest) {
        return Err("invalid operator source profile binding".into());
    }
    let loaded = load_config(path).map_err(provider)?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "source profile unavailable")?
        .take(MAXIMUM_PROFILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "source profile unavailable")?;
    if bytes.len() > MAXIMUM_PROFILE_BYTES || blake3::hash(&bytes).to_hex().as_str() != digest {
        return Err("source profile exact bytes changed".into());
    }
    let config: GithubConfig =
        serde_json::from_slice(&bytes).map_err(|_| "invalid source profile JSON")?;
    let admitted = admit_config(config).map_err(provider)?;
    if loaded.identity != admitted.identity {
        return Err("source profile changed during admission".into());
    }
    profile_bounds(&admitted)?;
    Ok((bytes, admitted))
}

/// Operator startup only. Customer RPCs select a digest, never executable/credential paths.
pub fn admit_profile(db: &mut Connection, path: &Path, exact_blake3: &str) -> Result<(), String> {
    let (bytes, admitted) = read_profile(path, exact_blake3)?;
    let path = path.to_str().ok_or("source profile path is not UTF-8")?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    let prior: Option<(Vec<u8>, String)> = tx
        .query_row(
            "SELECT bytes,adapter_identity FROM hosted_source_profiles WHERE blake3=?1",
            [exact_blake3],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    if let Some((prior, identity)) = prior {
        if prior != bytes || identity != admitted.identity.as_str() {
            return Err("source profile identity conflict".into());
        }
        tx.execute(
            "UPDATE hosted_source_profiles SET path=?2,active=1 WHERE blake3=?1",
            params![exact_blake3, path],
        )
        .map_err(storage)?;
    } else {
        let count: i64 = tx
            .query_row("SELECT COUNT(*) FROM hosted_source_profiles", [], |r| {
                r.get(0)
            })
            .map_err(storage)?;
        if count >= MAX_PROFILES {
            return Err("source profile retention limit".into());
        }
        tx.execute(
            "INSERT INTO hosted_source_profiles VALUES(?1,?2,?3,?4,1)",
            params![exact_blake3, path, bytes, admitted.identity.as_str()],
        )
        .map_err(storage)?;
    }
    tx.commit().map_err(storage)
}

fn admitted_profile(
    db: &Connection,
    digest: &str,
    check_file: bool,
) -> Result<AdmittedGithubConfig, String> {
    if !digest_valid(digest) {
        return Err("invalid profile digest".into());
    }
    let row:Option<(String,Vec<u8>,String)>=db.query_row("SELECT path,bytes,adapter_identity FROM hosted_source_profiles WHERE blake3=?1 AND active=1",[digest],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(storage)?;
    let (path, bytes, identity) = row.ok_or("source profile is not operator admitted")?;
    if bytes.len() > MAXIMUM_PROFILE_BYTES || blake3::hash(&bytes).to_hex().as_str() != digest {
        return Err("retained source profile integrity failure".into());
    }
    let admitted = if check_file {
        read_profile(Path::new(&path), digest)?.1
    } else {
        admit_config(serde_json::from_slice(&bytes).map_err(|_| "invalid retained profile")?)
            .map_err(provider)?
    };
    if admitted.identity.as_str() != identity {
        return Err("retained adapter profile identity changed".into());
    }
    Ok(admitted)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Connect {
    project: String,
    connection: String,
    repository_id: u64,
    owner: String,
    name: String,
    profile_blake3: String,
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    project: String,
    connection: String,
    generation: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Disconnect {
    project: String,
    connection: String,
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ingest {
    project: String,
    connection: String,
    generation: i64,
    headers: DeliveryHeaders,
    body_base64: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    project: String,
    #[serde(default)]
    after: Option<String>,
}

fn generation_profile(
    db: &Connection,
    org: &str,
    project: &str,
    connection: &str,
    generation: i64,
    new_admission: bool,
) -> Result<AdmittedGithubConfig, String> {
    names(project, connection)?;
    if generation <= 0 || !crate::identifier(org) {
        return Err("invalid connection generation".into());
    }
    let current:Option<(i64,bool)>=db.query_row("SELECT generation,active FROM hosted_connections WHERE organization=?1 AND project=?2 AND connection=?3",params![org,project,connection],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
    let (current, active) = current.ok_or("connection unavailable")?;
    if new_admission && (!active || current != generation) {
        return Err("connection disconnected or stale generation".into());
    }
    let row:Option<(String,Vec<u8>,String)>=db.query_row("SELECT profile_blake3,config,adapter_identity FROM hosted_connection_generations WHERE organization=?1 AND project=?2 AND connection=?3 AND generation=?4",params![org,project,connection,generation],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(storage)?;
    let (digest, bytes, identity) = row.ok_or("connection generation unavailable")?;
    if new_admission {
        profile_scope(db, &digest, org, project)?;
        admitted_profile(db, &digest, true)?;
    }
    if bytes.len() > MAXIMUM_PROFILE_BYTES {
        return Err("retained connection profile exceeds bound".into());
    }
    let admitted = admit_config(
        serde_json::from_slice(&bytes).map_err(|_| "invalid retained connection profile")?,
    )
    .map_err(provider)?;
    if admitted.identity.as_str() != identity {
        return Err("connection profile integrity failure".into());
    }
    profile_bounds(&admitted)?;
    Ok(admitted)
}

/// OPERATOR ONLY. Inventory every organization's retained native replay database,
/// including disconnected generations. Never expose this through customer RPCs.
/// Bounded inventory fails rather than silently omitting recovery obligations.
pub fn replay_paths(db: &Connection) -> Result<Vec<std::path::PathBuf>, String> {
    const MAX_BACKUP_REPLAY_STORES: usize = 4096;
    let service: String = db
        .query_row(
            "SELECT service_id FROM hosted_meta WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(storage)?;
    let mut statement=db.prepare("SELECT g.organization,g.project,g.connection,g.generation,g.profile_blake3,g.config,g.adapter_identity,p.bytes FROM hosted_connection_generations g JOIN hosted_source_profiles p ON p.blake3=g.profile_blake3 ORDER BY g.organization,g.project,g.connection,g.generation LIMIT 4097").map_err(storage)?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Vec<u8>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Vec<u8>>(7)?,
            ))
        })
        .map_err(storage)?;
    let total: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM hosted_connection_generations",
            [],
            |r| r.get(0),
        )
        .map_err(storage)?;
    if total
        > i64::try_from(MAX_BACKUP_REPLAY_STORES).map_err(|_| "backup inventory bound overflow")?
    {
        return Err("native replay backup inventory exceeds bound".into());
    }
    let mut paths = Vec::new();
    for row in rows {
        let (org, project, connection, generation, digest, bytes, identity, original) =
            row.map_err(storage)?;
        if bytes.len() > MAXIMUM_PROFILE_BYTES
            || original.len() > MAXIMUM_PROFILE_BYTES
            || !digest_valid(&digest)
            || blake3::hash(&original).to_hex().as_str() != digest
        {
            return Err("backup source profile integrity failure".into());
        }
        let mut expected: GithubConfig =
            serde_json::from_slice(&original).map_err(|_| "invalid retained original profile")?;
        admit_config(expected.clone()).map_err(provider)?;
        let namespace = crate::identity(&(
            "kiln.hosted.github-store.v1",
            &service,
            &org,
            &project,
            &connection,
            generation,
            &digest,
        ))?;
        let root = expected
            .replay_database_path
            .parent()
            .ok_or("native replay root unavailable")?;
        expected.replay_database_path = root.join(format!("hosted-{namespace}.sqlite"));
        expected.workflow.policy_id = format!("hosted-{namespace}");
        let admitted = admit_config(
            serde_json::from_slice(&bytes).map_err(|_| "invalid retained generation profile")?,
        )
        .map_err(provider)?;
        if admitted.config != expected || admitted.identity.as_str() != identity {
            return Err("backup native replay binding differs from pinned generation".into());
        }
        let native = Connection::open_with_flags(
            &admitted.config.replay_database_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        let metadata: (String, String) = native
            .query_row(
                "SELECT schema,profile_identity FROM github_meta WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(storage)?;
        if metadata.0 != "kiln.github-replay.v1" || metadata.1 != identity {
            return Err("backup native store profile identity mismatch".into());
        }
        paths.push(admitted.config.replay_database_path);
    }
    if i64::try_from(paths.len()).map_err(|_| "backup inventory overflow")? != total {
        return Err("native replay inventory has missing pinned profiles".into());
    }
    Ok(paths)
}

/// Raw exact operator-profile bytes digest, for parent activation/price policy lookup.
pub fn connection_profile_identity(
    db: &Connection,
    actor: &Actor,
    project: &str,
    connection: &str,
    generation: i64,
) -> Result<String, String> {
    accounts::require_project(db, actor, project, &["owner", "admin", "member"])?;
    generation_profile(
        db,
        &actor.organization,
        project,
        connection,
        generation,
        false,
    )?;
    db.query_row("SELECT profile_blake3 FROM hosted_connection_generations WHERE organization=?1 AND project=?2 AND connection=?3 AND generation=?4",params![actor.organization,project,connection,generation],|r|r.get(0)).map_err(storage)
}

/// False is an explicit connection/profile retirement, not a swallowed read error.
pub fn dispatch_allowed(
    db: &Connection,
    actor: &Actor,
    project: &str,
    connection: &str,
    generation: i64,
) -> Result<bool, String> {
    let digest = connection_profile_identity(db, actor, project, connection, generation)?;
    let current:(i64,bool)=db.query_row("SELECT generation,active FROM hosted_connections WHERE organization=?1 AND project=?2 AND connection=?3",params![actor.organization,project,connection],|r|Ok((r.get(0)?,r.get(1)?))).map_err(storage)?;
    let profile_active:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM hosted_source_profiles p JOIN hosted_source_profile_scopes s ON s.blake3=p.blake3 WHERE p.blake3=?1 AND p.active=1 AND s.active=1 AND s.organization=?2 AND s.project=?3)",params![digest,actor.organization,project],|r|r.get(0)).map_err(storage)?;
    if current.0 != generation || !current.1 || !profile_active {
        return Ok(false);
    }
    admitted_profile(db, &digest, true)?;
    Ok(true)
}

#[derive(Clone, Copy)]
struct SourceScope<'a> {
    organization: &'a str,
    project: &'a str,
    connection: &'a str,
    generation: i64,
}
#[derive(Clone, Copy)]
struct Delivery<'a> {
    headers: &'a DeliveryHeaders,
    body: &'a [u8],
}

/// Native webhook ingress: no customer peer is asserted. Exact signature, repository,
/// live generation and pinned operator profile establish source-event authority.
pub fn ingest(
    db: &mut Connection,
    org: &str,
    project: &str,
    connection: &str,
    generation: i64,
    headers: &DeliveryHeaders,
    body: &[u8],
) -> Result<Value, String> {
    ingest_authorized(
        db,
        None,
        SourceScope {
            organization: org,
            project,
            connection,
            generation,
        },
        Delivery { headers, body },
    )
}

fn ingest_authorized(
    db: &mut Connection,
    actor: Option<&Actor>,
    scope: SourceScope<'_>,
    delivery: Delivery<'_>,
) -> Result<Value, String> {
    let SourceScope {
        organization: org,
        project,
        connection,
        generation,
    } = scope;
    let Delivery { headers, body } = delivery;
    if body.len() > MAX_HOSTED_BODY_BYTES
        || headers.delivery.len() > 64
        || headers.event.len() > 64
        || headers.signature.len() > 128
    {
        return Err("webhook exceeds hosted bounds".into());
    }
    // Serialize membership/connection revocation with reservation and native replay admission.
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    if let Some(actor) = actor {
        accounts::require_project(&tx, actor, project, &["owner", "admin", "member"])?;
    }
    let admitted = generation_profile(&tx, org, project, connection, generation, true)?;
    let secret = resolve_secret(
        &admitted.config.webhook_secret,
        "authenticate hosted GitHub webhook",
    )
    .map_err(provider)?;
    let result = ingest_admitted(&tx, scope, delivery, secret.as_bytes(), &admitted)?;
    tx.commit().map_err(storage)?;
    Ok(result)
}

fn ingest_admitted(
    db: &Connection,
    scope: SourceScope<'_>,
    delivery: Delivery<'_>,
    secret: &[u8],
    admitted: &AdmittedGithubConfig,
) -> Result<Value, String> {
    let SourceScope {
        organization: org,
        project,
        connection,
        generation,
    } = scope;
    let Delivery { headers, body } = delivery;
    let Some(event) = admit_event(
        &admitted.config.event_policy,
        headers,
        body,
        secret,
        admitted.config.maximum_body_bytes,
    )
    .map_err(provider)?
    else {
        return Ok(json!({"accepted":false,"ignored":true}));
    };
    let record = plan_event(admitted, &event).map_err(provider)?;
    let prior:Option<(String,i64,String)>=db.query_row("SELECT body_identity,generation,run_identity FROM hosted_source_deliveries WHERE organization=?1 AND project=?2 AND connection=?3 AND delivery=?4",params![org,project,connection,event.delivery_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(storage)?;
    if let Some((body, prior_generation, run)) = prior {
        if body != event.body_identity.as_str()
            || prior_generation != generation
            || run != record.run_identity.as_str()
        {
            return Err(
                "delivery conflicts with retained connection generation or candidate".into(),
            );
        }
    } else {
        let count:i64=db.query_row("SELECT COUNT(*) FROM hosted_source_deliveries WHERE organization=?1 AND project=?2 AND connection=?3",params![org,project,connection],|r|r.get(0)).map_err(storage)?;
        if count
            >= i64::try_from(admitted.config.maximum_delivery_records)
                .map_err(|_| "delivery count overflow")?
        {
            return Err("connection delivery retention limit".into());
        }
        db.execute(
            "INSERT INTO hosted_source_deliveries VALUES(?1,?2,?3,?4,?5,?6,?7,'reserved')",
            params![
                org,
                project,
                connection,
                event.delivery_id,
                event.body_identity.as_str(),
                generation,
                record.run_identity.as_str()
            ],
        )
        .map_err(storage)?;
    }
    // The adapter commits its own native delivery and run record before acknowledgement.
    // A host rollback/crash can retry this exact native reservation without dispatch.
    let reservation = open_store(admitted)
        .map_err(provider)?
        .reserve(&event, record)
        .map_err(provider)?;
    db.execute("UPDATE hosted_source_deliveries SET state='admitted' WHERE organization=?1 AND project=?2 AND connection=?3 AND delivery=?4",params![org,project,connection,event.delivery_id]).map_err(storage)?;
    Ok(
        json!({"accepted":true,"run":reservation.run_identity.as_str(),"inserted":reservation.inserted,"generation":generation}),
    )
}

#[derive(Debug, serde::Serialize)]
pub struct PendingDispatch {
    pub run: String,
    pub attempt: String,
    pub plan: kiln_core::workflow::WorkflowEffectPlan,
}

fn work_records(
    db: &Connection,
    org: &str,
    project: &str,
    connection: &str,
    generation: i64,
    admitted: &AdmittedGithubConfig,
) -> Result<Vec<kiln_adapter_github::service::RunRecord>, String> {
    let store = open_store(admitted).map_err(provider)?;
    let mut records = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..=admitted.config.maximum_run_records {
        let Some(record) = store.next_work(cursor.as_deref()).map_err(provider)? else {
            break;
        };
        if records
            .iter()
            .any(|r: &kiln_adapter_github::service::RunRecord| {
                r.run_identity == record.run_identity
            })
        {
            break;
        }
        let indexed:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM hosted_source_deliveries WHERE organization=?1 AND project=?2 AND connection=?3 AND generation=?4 AND run_identity=?5 AND state='admitted')",params![org,project,connection,generation,record.run_identity.as_str()],|r|r.get(0)).map_err(storage)?;
        if !indexed {
            return Err(
                "native admission awaiting exact webhook redelivery after interrupted host commit"
                    .into(),
            );
        }
        cursor = Some(record.run_identity.as_str().to_string());
        records.push(record);
    }
    Ok(records)
}

pub fn pending_dispatches(
    db: &Connection,
    actor: &Actor,
    project: &str,
    connection: &str,
    generation: i64,
) -> Result<Vec<PendingDispatch>, String> {
    accounts::require_project(db, actor, project, &["owner", "admin", "member"])?;
    let admitted = generation_profile(
        db,
        &actor.organization,
        project,
        connection,
        generation,
        true,
    )?;
    let records = work_records(
        db,
        &actor.organization,
        project,
        connection,
        generation,
        &admitted,
    )?;
    Ok(records
        .into_iter()
        .filter(|r| r.phase == kiln_adapter_github::service::RunPhase::Queued)
        .map(|r| PendingDispatch {
            run: r.run_identity.as_str().into(),
            attempt: r.plan.attempt_id.as_str().into(),
            plan: r.plan,
        })
        .collect())
}

/// Operator-enabled provider effects only, after parent finance/incident admission.
/// Every currently queued attempt must have a durable parent reservation. During
/// a hold/disconnect, allow_dispatch=false only reconciles already started runs.
pub fn tick(
    db: &mut Connection,
    actor: &Actor,
    project: &str,
    connection: &str,
    generation: i64,
    approved_attempts: &[String],
    allow_dispatch: bool,
) -> Result<Value, String> {
    if approved_attempts.len() > MAX_HOSTED_RUNS {
        return Err("dispatch approval bound exceeded".into());
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    accounts::require_project(&tx, actor, project, &["owner", "admin", "member"])?;
    let admitted = generation_profile(
        &tx,
        &actor.organization,
        project,
        connection,
        generation,
        allow_dispatch,
    )?;
    let records = work_records(
        &tx,
        &actor.organization,
        project,
        connection,
        generation,
        &admitted,
    )?;
    if allow_dispatch
        && records.iter().any(|r| {
            r.phase == kiln_adapter_github::service::RunPhase::Queued
                && !approved_attempts
                    .iter()
                    .any(|a| a == r.plan.attempt_id.as_str())
        })
    {
        return Err("queued attempt lacks durable finance reservation".into());
    }
    let mut service = runtime(&admitted).map_err(provider)?;
    let mut progressed = 0usize;
    if allow_dispatch {
        for _ in 0..records.len() {
            if !service.tick().map_err(provider)? {
                break;
            }
            progressed = progressed.checked_add(1).ok_or("tick counter overflow")?;
        }
    } else {
        for record in records {
            if matches!(
                record.phase,
                kiln_adapter_github::service::RunPhase::Queued
                    | kiln_adapter_github::service::RunPhase::Unresolved
            ) {
                continue;
            }
            service
                .reconcile_run(&record.run_identity)
                .map_err(provider)?;
            progressed = progressed.checked_add(1).ok_or("tick counter overflow")?;
        }
    }
    tx.commit().map_err(storage)?;
    Ok(json!({"generation":generation,"work_steps":progressed}))
}

pub fn status(
    db: &Connection,
    actor: &Actor,
    project: &str,
    connection: &str,
    generation: i64,
) -> Result<Value, String> {
    accounts::require_project(db, actor, project, &["owner", "admin", "member", "viewer"])?;
    let admitted = generation_profile(
        db,
        &actor.organization,
        project,
        connection,
        generation,
        false,
    )?;
    let mut statement=db.prepare("SELECT DISTINCT run_identity FROM hosted_source_deliveries WHERE organization=?1 AND project=?2 AND connection=?3 AND generation=?4 AND state='admitted' ORDER BY run_identity LIMIT 128").map_err(storage)?;
    let rows = statement
        .query_map(
            params![actor.organization, project, connection, generation],
            |r| r.get::<_, String>(0),
        )
        .map_err(storage)?;
    let store = open_store(&admitted).map_err(provider)?;
    let mut runs = Vec::new();
    for row in rows {
        let run = kiln_core::types::ContentId::from_blake3_hex(row.map_err(storage)?)
            .map_err(|_| "invalid retained run identity")?;
        let record = store
            .load(&run)
            .map_err(provider)?
            .ok_or("native adapter run record missing")?;
        let observation=record.observation.as_ref().map(|o|json!({"state":o.state,"terminal":o.terminal,"successful":o.successful,"sequence":o.sequence,"workflow":o.workflow,"execution":o.execution}));
        let statuses=record.statuses.iter().map(|s|json!({"state":s.plan.state,"phase":s.phase,"publication":s.publication,"error_code":s.error_code})).collect::<Vec<_>>();
        runs.push(json!({"run":record.run_identity.as_str(),"phase":record.phase,"outcome":record.outcome().map_err(provider)?,"observation":observation,"statuses":statuses,"error_code":record.error_code}));
    }
    Ok(
        json!({"project":project,"connection":connection,"generation":generation,"effective_adapter_identity":admitted.identity.as_str(),"pipeline_identity":admitted.pipeline_identity.as_str(),"workflow_policy_id":admitted.config.workflow.policy_id,"route_admission_required":true,"runs":runs}),
    )
}

pub fn handle(
    db: &mut Connection,
    actor: &Actor,
    action: &str,
    input: Value,
    _now_ms: u64,
) -> Result<Value, String> {
    match action {
        "connect" => {
            let input: Connect =
                serde_json::from_value(input).map_err(|_| "invalid repository connection")?;
            names(&input.project, &input.connection)?;
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            let actor = accounts::require_project(&tx, actor, &input.project, &["owner", "admin"])?;
            profile_scope(
                &tx,
                &input.profile_blake3,
                &actor.organization,
                &input.project,
            )?;
            let mut admitted = admitted_profile(&tx, &input.profile_blake3, true)?;
            if input.repository_id != admitted.config.event_policy.repository_id
                || !input
                    .owner
                    .eq_ignore_ascii_case(&admitted.config.event_policy.owner)
                || !input
                    .name
                    .eq_ignore_ascii_case(&admitted.config.event_policy.repository)
            {
                return Err("repository identity does not match operator profile".into());
            }
            let prior:Option<(String,i64,i64)>=tx.query_row("SELECT repository_id,generation,revision FROM hosted_connections WHERE organization=?1 AND project=?2 AND connection=?3",params![actor.organization,input.project,input.connection],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(storage)?;
            let (generation, revision) = if let Some((repository_id, generation, revision)) = prior
            {
                if input.expected_revision != revision
                    || repository_id != input.repository_id.to_string()
                {
                    return Err("stale revision or replacement repository denied".into());
                }
                (
                    generation
                        .checked_add(1)
                        .ok_or("connection generation overflow")?,
                    revision
                        .checked_add(1)
                        .ok_or("connection revision overflow")?,
                )
            } else {
                if input.expected_revision != 0 {
                    return Err("new connection requires revision zero".into());
                }
                let count: i64 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM hosted_connections WHERE organization=?1",
                        [&actor.organization],
                        |r| r.get(0),
                    )
                    .map_err(storage)?;
                if count >= MAX_CONNECTIONS {
                    return Err("organization connection limit".into());
                }
                (1, 1)
            };
            if generation > MAX_GENERATIONS {
                return Err("connection generation retention limit".into());
            }
            let service: String = tx
                .query_row(
                    "SELECT service_id FROM hosted_meta WHERE singleton=1",
                    [],
                    |r| r.get(0),
                )
                .map_err(storage)?;
            let namespace = crate::identity(&(
                "kiln.hosted.github-store.v1",
                service,
                &actor.organization,
                &input.project,
                &input.connection,
                generation,
                &input.profile_blake3,
            ))?;
            let root = admitted
                .config
                .replay_database_path
                .parent()
                .ok_or("source replay root unavailable")?;
            admitted.config.replay_database_path = root.join(format!("hosted-{namespace}.sqlite"));
            // Policy participates in native pipeline/run/attempt identities. A private
            // replay file alone must not alias another tenant's provider effect keys.
            admitted.config.workflow.policy_id = format!("hosted-{namespace}");
            let admitted = admit_config(admitted.config).map_err(provider)?;
            let bytes = serde_json::to_vec(&admitted.config)
                .map_err(|_| "connection profile encoding failed")?;
            // Establish native replay namespace before publishing connection activation.
            open_store(&admitted).map_err(provider)?;
            tx.execute(
                "INSERT INTO hosted_connection_generations VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    actor.organization,
                    input.project,
                    input.connection,
                    generation,
                    input.profile_blake3,
                    bytes,
                    admitted.identity.as_str()
                ],
            )
            .map_err(storage)?;
            tx.execute("INSERT INTO hosted_connections VALUES(?1,?2,?3,?4,?5,?6,?7,?8,1,?9) ON CONFLICT(organization,project,connection) DO UPDATE SET owner=excluded.owner,name=excluded.name,generation=excluded.generation,revision=excluded.revision,active=1,changed_by=excluded.changed_by",params![actor.organization,input.project,input.connection,input.repository_id.to_string(),admitted.config.event_policy.owner,admitted.config.event_policy.repository,generation,revision,actor.peer]).map_err(storage)?;
            tx.commit().map_err(storage)?;
            Ok(
                json!({"connection":input.connection,"project":input.project,"generation":generation,"revision":revision,"profile_blake3":input.profile_blake3,"effective_adapter_identity":admitted.identity.as_str(),"pipeline_identity":admitted.pipeline_identity.as_str(),"workflow_policy_id":admitted.config.workflow.policy_id,"route_admission_required":true}),
            )
        }
        "disconnect" => {
            let input: Disconnect =
                serde_json::from_value(input).map_err(|_| "invalid disconnect")?;
            names(&input.project, &input.connection)?;
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            let actor = accounts::require_project(&tx, actor, &input.project, &["owner", "admin"])?;
            let revision = input
                .expected_revision
                .checked_add(1)
                .filter(|r| *r > 1)
                .ok_or("invalid connection revision")?;
            let changed=tx.execute("UPDATE hosted_connections SET active=0,revision=?4,changed_by=?5 WHERE organization=?1 AND project=?2 AND connection=?3 AND revision=?6 AND active=1",params![actor.organization,input.project,input.connection,revision,actor.peer,input.expected_revision]).map_err(storage)?;
            if changed != 1 {
                return Err("stale connection revision or already disconnected".into());
            }
            tx.commit().map_err(storage)?;
            Ok(json!({"disconnected":true,"revision":revision,"replay_retained":true}))
        }
        "profiles" => {
            let input: List =
                serde_json::from_value(input).map_err(|_| "invalid source profile list")?;
            accounts::require_project(db, actor, &input.project, &["owner", "admin"])?;
            let mut statement=db.prepare("SELECT p.blake3,p.bytes FROM hosted_source_profiles p JOIN hosted_source_profile_scopes s ON s.blake3=p.blake3 WHERE s.organization=?1 AND s.project=?2 AND s.active=1 AND p.active=1 ORDER BY p.blake3 LIMIT 128").map_err(storage)?;
            let rows = statement
                .query_map(params![actor.organization, input.project], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
                })
                .map_err(storage)?;
            let mut profiles = Vec::new();
            for row in rows {
                let (digest, bytes) = row.map_err(storage)?;
                if bytes.len() > MAXIMUM_PROFILE_BYTES {
                    return Err("profile integrity failure".into());
                }
                let config: GithubConfig =
                    serde_json::from_slice(&bytes).map_err(|_| "invalid retained profile")?;
                profiles.push(json!({"profile_blake3":digest,"repository_id":config.event_policy.repository_id,"owner":config.event_policy.owner,"name":config.event_policy.repository}));
            }
            Ok(json!({"profiles":profiles}))
        }
        "list" => {
            let input: List =
                serde_json::from_value(input).map_err(|_| "invalid connection list")?;
            accounts::require_project(
                db,
                actor,
                &input.project,
                &["owner", "admin", "member", "viewer"],
            )?;
            if input.after.as_ref().is_some_and(|a| !crate::identifier(a)) {
                return Err("invalid page cursor".into());
            }
            let mut statement=db.prepare("SELECT connection,repository_id,owner,name,generation,revision,active FROM hosted_connections WHERE organization=?1 AND project=?2 AND connection>?3 ORDER BY connection LIMIT 129").map_err(storage)?;
            let rows = statement
                .query_map(
                    params![
                        actor.organization,
                        input.project,
                        input.after.unwrap_or_default()
                    ],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, i64>(5)?,
                            r.get::<_, bool>(6)?,
                        ))
                    },
                )
                .map_err(storage)?;
            let mut connections = Vec::new();
            for row in rows {
                let (connection, id, owner, name, generation, revision, active) =
                    row.map_err(storage)?;
                connections.push(json!({"connection":connection,"repository_id":id,"owner":owner,"name":name,"generation":generation,"revision":revision,"active":active}));
            }
            let next = if connections.len() > MAX_ROWS {
                connections.pop();
                connections
                    .last()
                    .map(|c| c.get("connection").unwrap_or(&Value::Null).clone())
            } else {
                None
            };
            Ok(json!({"connections":connections,"next":next}))
        }
        "ingest" => {
            let input: Ingest =
                serde_json::from_value(input).map_err(|_| "invalid webhook relay")?;
            accounts::require_project(db, actor, &input.project, &["owner", "admin", "member"])?;
            if input.body_base64.len() > MAX_HOSTED_BODY_BYTES.div_ceil(3) * 4 {
                return Err("webhook body exceeds hosted bound".into());
            }
            let body = base64::engine::general_purpose::STANDARD
                .decode(&input.body_base64)
                .map_err(|_| "invalid webhook base64")?;
            ingest_authorized(
                db,
                Some(actor),
                SourceScope {
                    organization: &actor.organization,
                    project: &input.project,
                    connection: &input.connection,
                    generation: input.generation,
                },
                Delivery {
                    headers: &input.headers,
                    body: &body,
                },
            )
        }
        "status" => {
            let input: Target =
                serde_json::from_value(input).map_err(|_| "invalid connection target")?;
            status(
                db,
                actor,
                &input.project,
                &input.connection,
                input.generation,
            )
        }
        "tick" => Err(
            "provider effects require operator activation and server-side finance admission".into(),
        ),
        _ => Err("unknown connection action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    struct Fixture {
        root: tempfile::TempDir,
        db: Connection,
        actor: Actor,
        path: std::path::PathBuf,
        digest: String,
    }
    fn fixture() -> Fixture {
        let root = tempfile::tempdir()
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        // The actual SecretSpec subprocess executes immutable /bin/sh, reading the
        // isolated protocol script named `get` (no host credentials or network).
        std::fs::write(
            root.path().join("get"),
            "printf '%s' 'hosted-webhook-test-secret'\n",
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let interpreter = std::fs::read("/bin/sh")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let secret = json!({"program_path":"/bin/sh","program_blake3":blake3::hash(&interpreter).to_hex().as_str(),"project_root":root.path(),"profile":"fixture","provider":null,"secret_key":"WEBHOOK_SECRET"});
        let config = json!({
            "schema":"kiln.github-ci.v1","event_policy":{"owner":"octocat","repository":"hello-world","repository_id":1296269,"allow_fork_pull_requests":false},
            "listen_address":"127.0.0.1:19091","webhook_path":"/github/webhook","replay_database_path":root.path().join("github.sqlite"),
            "maximum_run_records":4,"maximum_delivery_records":8,"maximum_record_bytes":65536,"maximum_body_bytes":65536,"maximum_header_bytes":8192,
            "read_timeout_ms":500,"write_timeout_ms":500,"maximum_connections":2,"worker_poll_interval_ms":10,"maximum_workflow_polls":2,"maximum_active_runs":1,
            "webhook_secret":secret,"status_secret":secret,"status":{"owner":"octocat","repository":"hello-world","context":"kiln/ci","maximum_response_bytes":65536,"maximum_pages":2,"timeout_ms":500},
            "workflow":{"socket_path":root.path().join("lattice.sock"),"allowed_user_ids":[1000],"transport_limits":{"maximum_frame_bytes":65536,"maximum_log_bytes":4096,"maximum_connections":64,"read_deadline_milliseconds":500,"write_deadline_milliseconds":500},
            "adapter_limits":{"maximum_request_bytes":65536,"maximum_response_bytes":65536},"policy_id":"github-ci-v1","provider_profile_id":"lattice-v1","platform":"linux-x86_64",
            "nodes":[{"id":"test","kind":"test","effect_class":"test","adapter_kind":"local","operation_identity":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}],"edges":[],"controls":{"cancel":true,"pause":false,"resume":false},
            "bounds":{"max_nodes":4,"max_edges":4,"max_log_references":4,"max_log_read_bytes":1024,"max_total_log_bytes":4096}},
            "non_claims":kiln_adapter_github::profile::REQUIRED_NON_CLAIMS
        });
        let bytes = serde_json::to_vec(&config)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let digest = blake3::hash(&bytes).to_hex().to_string();
        let path = root.path().join("github.json");
        std::fs::write(&path, &bytes)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let mut db = Connection::open(root.path().join("hosted.sqlite"))
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        db.pragma_update(None, "foreign_keys", true)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        db.execute_batch("CREATE TABLE hosted_meta(singleton INTEGER PRIMARY KEY,schema TEXT,service_id TEXT); INSERT INTO hosted_meta VALUES(1,'kiln.hosted.v1','test-service');").unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        accounts::init(&db)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        init(&db).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let peer = "01".repeat(32);
        accounts::bootstrap(&mut db, &peer, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let actor = accounts::authorize(&db, &peer, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        admit_profile(&mut db, &path, &digest)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        admit_profile_scope(&db, &digest, "acme", "app")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        Fixture {
            root,
            db,
            actor,
            path,
            digest,
        }
    }
    fn connect(f: &mut Fixture, revision: i64) -> Value {
        handle(&mut f.db,&f.actor,"connect",json!({"project":"app","connection":"origin","repository_id":1296269,"owner":"octocat","name":"hello-world","profile_blake3":f.digest,"expected_revision":revision}),1).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"))
    }
    fn body(id: u64) -> Vec<u8> {
        serde_json::to_vec(&json!({"repository":{"id":id,"name":"hello-world","full_name":"octocat/hello-world","owner":{"login":"octocat"}},"ref":"refs/heads/main","after":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","deleted":false})).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"))
    }
    fn headers(body: &[u8], delivery: u64) -> DeliveryHeaders {
        let mut mac = Hmac::<Sha256>::new_from_slice(b"hosted-webhook-test-secret")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        mac.update(body);
        let signature = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        DeliveryHeaders {
            delivery: format!("00000000-0000-4000-8000-{delivery:012x}"),
            event: "push".into(),
            signature: format!("sha256={signature}"),
        }
    }
    #[test]
    fn signed_ingress_replay_signature_repository_and_scope() {
        let mut f = fixture();
        connect(&mut f, 0);
        let body = body(1296269);
        let headers = headers(&body, 1);
        let first = ingest(&mut f.db, "acme", "app", "origin", 1, &headers, &body)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(first.get("inserted"), Some(&json!(true)));
        let replay = ingest(&mut f.db, "acme", "app", "origin", 1, &headers, &body)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(replay.get("run"), first.get("run"));
        assert_eq!(replay.get("inserted"), Some(&json!(false)));
        let mut bad = headers.clone();
        bad.signature = format!("sha256={}", "0".repeat(64));
        assert!(ingest(&mut f.db, "acme", "app", "origin", 1, &bad, &body).is_err());
        let other = super::tests::body(42);
        assert!(
            ingest(
                &mut f.db,
                "acme",
                "app",
                "origin",
                1,
                &super::tests::headers(&other, 2),
                &other
            )
            .is_err()
        );
        assert!(ingest(&mut f.db, "other", "app", "origin", 1, &headers, &body).is_err());
        assert!(ingest(&mut f.db, "acme", "private", "origin", 1, &headers, &body).is_err());
        let view = status(&f.db, &f.actor, "app", "origin", 1)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(view.pointer("/runs/0/phase"), Some(&json!("queued")));
        assert_eq!(view.pointer("/runs/0/run"), first.get("run"));
        let pending = pending_dispatches(&f.db, &f.actor, "app", "origin", 1)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(
            pending.first().map(|p| p.run.as_str()),
            first.get("run").and_then(Value::as_str)
        );
        // Unfunded dispatch fails before resolving a status token or opening a workflow socket.
        assert!(tick(&mut f.db, &f.actor, "app", "origin", 1, &[], true).is_err());
        assert!(
            handle(
                &mut f.db,
                &f.actor,
                "tick",
                json!({"project":"app","connection":"origin","generation":1}),
                1
            )
            .is_err()
        );
    }
    #[test]
    fn disconnect_generations_preserve_replay_and_namespace() {
        let mut f = fixture();
        connect(&mut f, 0);
        let body = body(1296269);
        let headers = headers(&body, 1);
        let first = ingest(&mut f.db, "acme", "app", "origin", 1, &headers, &body)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let original = generation_profile(&f.db, "acme", "app", "origin", 1, false)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        handle(
            &mut f.db,
            &f.actor,
            "disconnect",
            json!({"project":"app","connection":"origin","expected_revision":1}),
            1,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(ingest(&mut f.db, "acme", "app", "origin", 1, &headers, &body).is_err());
        assert_eq!(
            status(&f.db, &f.actor, "app", "origin", 1)
                .unwrap_or_else(|error| unreachable!("fixture status succeeds: {error:?}"))
                .pointer("/runs/0/run"),
            first.get("run")
        );
        assert_eq!(connect(&mut f, 2).get("generation"), Some(&json!(2)));
        let next = generation_profile(&f.db, "acme", "app", "origin", 2, false)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_ne!(
            original.config.replay_database_path,
            next.config.replay_database_path
        );
        assert_ne!(original.pipeline_identity, next.pipeline_identity);
        assert!(ingest(&mut f.db, "acme", "app", "origin", 1, &headers, &body).is_err());
        assert!(ingest(&mut f.db, "acme", "app", "origin", 2, &headers, &body).is_err());
        let second = ingest(
            &mut f.db,
            "acme",
            "app",
            "origin",
            2,
            &super::tests::headers(&body, 2),
            &body,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_ne!(second.get("run"), first.get("run"));
        assert!(original.config.replay_database_path.exists());
        assert!(handle(&mut f.db,&f.actor,"connect",json!({"project":"app","connection":"origin","repository_id":1296269,"owner":"octocat","name":"hello-world","profile_blake3":f.digest,"expected_revision":1}),1).is_err());
    }
    #[test]
    fn profile_drift_deactivation_and_restart_fail_closed() {
        let mut f = fixture();
        connect(&mut f, 0);
        let body = body(1296269);
        let headers = headers(&body, 1);
        let first = ingest(&mut f.db, "acme", "app", "origin", 1, &headers, &body)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let retained = std::fs::read(&f.path)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let mut changed = retained.clone();
        changed.push(b' ');
        std::fs::write(&f.path, &changed)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(
            ingest(
                &mut f.db,
                "acme",
                "app",
                "origin",
                1,
                &super::tests::headers(&body, 2),
                &body
            )
            .is_err()
        );
        std::fs::write(&f.path, &retained)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        begin_profile_configuration(&f.db)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(ingest(&mut f.db, "acme", "app", "origin", 1, &headers, &body).is_err());
        admit_profile(&mut f.db, &f.path, &f.digest)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        admit_profile_scope(&f.db, &f.digest, "acme", "app")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let database = f.root.path().join("hosted.sqlite");
        drop(f.db);
        f.db = Connection::open(database)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let replay = ingest(&mut f.db, "acme", "app", "origin", 1, &headers, &body)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(replay.get("run"), first.get("run"));
        assert_eq!(replay.get("inserted"), Some(&json!(false)));
        f.db.execute("UPDATE hosted_source_profile_scopes SET active=0", [])
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(ingest(&mut f.db, "acme", "app", "origin", 1, &headers, &body).is_err());
        assert!(pending_dispatches(&f.db, &f.actor, "app", "origin", 1).is_err());
        assert_eq!(
            status(&f.db, &f.actor, "app", "origin", 1)
                .unwrap_or_else(|error| unreachable!("fixture status succeeds: {error:?}"))
                .pointer("/runs/0/run"),
            first.get("run")
        );
        assert!(handle(&mut f.db,&f.actor,"connect",json!({"project":"private","connection":"stolen","repository_id":1296269,"owner":"octocat","name":"hello-world","profile_blake3":f.digest,"expected_revision":0}),1).is_err());
        assert_eq!(
            connection_profile_identity(&f.db, &f.actor, "app", "origin", 1).unwrap_or_else(
                |error| unreachable!("fixture profile identity succeeds: {error:?}")
            ),
            f.digest
        );
    }
    #[test]
    fn revoked_relay_peer_and_cross_project_access_fail_before_native_admission() {
        let mut f = fixture();
        connect(&mut f, 0);
        let peer = "02".repeat(32);
        let invitation=accounts::handle(&mut f.db,&f.actor,"invite",json!({"peer":peer,"role":"member","projects":["app"],"expires_at_ms":5000,"nonce":"relay"}),1000).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}")).get("invitation").unwrap_or(&Value::Null).clone();
        accounts::accept_invitation(
            &mut f.db,
            &peer,
            "acme",
            json!({"invitation":invitation}),
            1001,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let member = accounts::authorize(&f.db, &peer, "acme")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let body = body(1296269);
        let signed = headers(&body, 1);
        let input = json!({"project":"app","connection":"origin","generation":1,"headers":signed,"body_base64":base64::engine::general_purpose::STANDARD.encode(&body)});
        let accepted = handle(&mut f.db, &member, "ingest", input.clone(), 1002)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(accepted.get("accepted"), Some(&json!(true)));
        assert!(
            handle(
                &mut f.db,
                &member,
                "list",
                json!({"project":"private"}),
                1003
            )
            .is_err()
        );
        accounts::handle(&mut f.db,&f.actor,"set_membership",json!({"peer":peer,"expected_revision":1,"role":"member","projects":["app"],"active":false}),1003).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(handle(&mut f.db, &member, "ingest", input, 1004).is_err());
        assert!(status(&f.db, &member, "app", "origin", 1).is_err());
        assert_eq!(
            status(&f.db, &f.actor, "app", "origin", 1)
                .unwrap_or_else(|error| unreachable!("fixture status succeeds: {error:?}"))
                .pointer("/runs/0/run"),
            accepted.get("run")
        );
    }
    #[test]
    fn backup_inventory_retains_retired_generations_and_rejects_native_identity_drift() {
        let mut f = fixture();
        connect(&mut f, 0);
        let first = generation_profile(&f.db, "acme", "app", "origin", 1, false)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        handle(
            &mut f.db,
            &f.actor,
            "disconnect",
            json!({"project":"app","connection":"origin","expected_revision":1}),
            1,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        connect(&mut f, 2);
        let second = generation_profile(&f.db, "acme", "app", "origin", 2, false)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        begin_profile_configuration(&f.db)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let paths = replay_paths(&f.db)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(
            paths,
            vec![
                first.config.replay_database_path.clone(),
                second.config.replay_database_path
            ]
        );
        let native = Connection::open(&first.config.replay_database_path)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        native
            .execute(
                "UPDATE github_meta SET profile_identity=?1 WHERE singleton=1",
                ["0".repeat(64)],
            )
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(replay_paths(&f.db).is_err());
    }
}
