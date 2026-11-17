//! Real-process lifecycle qualification of the hosted fleet driver boundary.
//!
//! `run` admits the fake provider below through the unchanged immutable
//! executable verification, drives the real account, finance and fleet code
//! in separate coordinator processes over SQLite, interrupts coordinators
//! during driver effects and writes a receipt. `driver` is the fake provider
//! and `coordinator` performs exactly one action. Nothing here is a cloud API
//! or production provider qualification.
//!
//! ```text
//! fleet_qualification run --nix-store ABS_NIX_STORE --bash ABS_BASH --out NEW_DIR
//! ```
// r[impl kiln.hosted.fleet.lifecycle_qualification]

use kiln_adapter_github::secret::SecretReference;
use kiln_hosted::{
    Actor, accounts, finance,
    fleet::{self, Config, Executable, Pricing},
    secrets,
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type Result<T, E = String> = std::result::Result<T, E>;

const DRIVER_SCHEMA: &str = "kiln.hosted.fleet-driver.v1";
const RECEIPT_SCHEMA: &str = "kiln.hosted.fleet-qualification-receipt.v1";
const DRIVER_FLAG: &str = "--kiln-fleet-v1";
const MAX_DRIVER_REQUEST_BYTES: u64 = 16 * 1024;
const MAX_FAULT_BYTES: u64 = 4 * 1024;
/// Exit status for a deliberately missing driver reply (EX_TEMPFAIL).
const NO_REPLY_EXIT: i32 = 75;
const MALFORMED_EXIT: i32 = 2;
const BARRIER_POLL: Duration = Duration::from_millis(20);
const DRIVER_BARRIER_MAX_WAIT: Duration = Duration::from_secs(30);
const HARNESS_BARRIER_MAX_WAIT: Duration = Duration::from_secs(30);
const DRIVER_EXIT_MAX_WAIT: Duration = Duration::from_secs(30);
/// Fleet driver deadline; longer than the barrier wait so only kills end a hang.
const DRIVER_TIMEOUT_MS: u64 = 60_000;
const ORGANIZATION: &str = "fleet-qualification";
const PEER_BYTE: &str = "a";
const PEER_LENGTH: usize = 64;
const PRICE_CLASS: &str = "linux";
const PRICE_CURRENCY: &str = "usd";
const ACCOUNT_CAP_MINOR: u64 = 1_000_000;
const ACCOUNT_CONCURRENCY: u64 = 64;
const FLEET_SOURCE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/fleet.rs"));
const HARNESS_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/fleet_qualification.rs"
));
const SECRETS_SOURCE: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/secrets.rs"));
const SECRET_AUDIENCE: &str = "https://fleet-resource.example.test";
const SECRET_NAME: &str = "WORKER_TOKEN";
const SECRET_LIFETIME_SECONDS: u64 = 120;
const SECRET_REFERENCE_MS: u64 = 600_000;
const SECRET_JOB_MS: u64 = 300_000;
const SECRET_VALUE: &str = "fleet-qualification-value";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("driver") => driver(args.get(1..).unwrap_or_default()),
        Some("secret-inspect") => match inspect_secret(args.get(1..).unwrap_or_default()) {
            Ok(observation) => {
                println!("{observation}");
                0
            }
            Err(error) => {
                eprintln!("fake provider secret inspection failed: {error}");
                1
            }
        },
        Some("coordinator") => coordinator(args.get(1..).unwrap_or_default()),
        Some("run") => match run(args.get(1..).unwrap_or_default()) {
            Ok(passed) => i32::from(!passed),
            Err(error) => {
                eprintln!("fleet qualification failed: {error}");
                1
            }
        },
        _ => {
            eprintln!(
                "usage: fleet_qualification run --nix-store ABS_NIX_STORE --bash ABS_BASH --out NEW_DIR"
            );
            MALFORMED_EXIT
        }
    };
    std::process::exit(code);
}

// ---------------------------------------------------------------------------
// Fake provider driver.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverRequest {
    schema: String,
    organization: String,
    request_id: String,
    instance_id: String,
    operation: String,
    slots: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FaultKind {
    BeforeEffect,
    AfterEffectNoReply,
    HangBeforeEffect,
    HangAfterEffect,
    StaleReply,
    DuplicateCreate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fault {
    operation: String,
    instance_id: String,
    kind: FaultKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LogEntry {
    seq: u64,
    operation: String,
    organization: String,
    instance_id: String,
    request_id: String,
    effect: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BarrierOutcome {
    Released,
    Aborted,
}

fn driver(args: &[String]) -> i32 {
    match driver_result(args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("fake fleet driver refused: {error}");
            MALFORMED_EXIT
        }
    }
}

fn driver_result(args: &[String]) -> Result<i32> {
    let [flag, inventory, protocol] = args else {
        return Err("expected --inventory DIR --kiln-fleet-v1".into());
    };
    if flag != "--inventory" || protocol != DRIVER_FLAG || !Path::new(inventory).is_absolute() {
        return Err("expected --inventory ABS_DIR --kiln-fleet-v1".into());
    }
    let inventory = PathBuf::from(inventory);
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(MAX_DRIVER_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() as u64 > MAX_DRIVER_REQUEST_BYTES {
        return Err("driver request exceeds its bound".into());
    }
    let request: DriverRequest = serde_json::from_slice(&bytes).map_err(text)?;
    if request.schema != DRIVER_SCHEMA
        || request.slots == 0
        || !kiln_hosted::identifier(&request.organization)
        || !kiln_hosted::identifier(&request.request_id)
        || !kiln_hosted::identifier(&request.instance_id)
        || !matches!(
            request.operation.as_str(),
            "provision" | "deprovision" | "inspect"
        )
    {
        return Err("driver request is not an admitted fleet operation".into());
    }
    let fault = take_fault(&inventory, &request)?;
    if fault == Some(FaultKind::BeforeEffect) {
        return Ok(NO_REPLY_EXIT);
    }
    if fault == Some(FaultKind::HangBeforeEffect)
        && wait_at_barrier(&inventory, &request, "before_effect")? == BarrierOutcome::Aborted
    {
        return Ok(NO_REPLY_EXIT);
    }
    let resources = inventory.join("resources");
    let record = resources.join(resource_name(&request.organization, &request.instance_id));
    let effect = match request.operation.as_str() {
        "provision" if record.exists() => "already_present",
        "provision" => {
            write_atomic(
                &record,
                &serde_json::to_vec(&json!({"organization": request.organization, "instance_id": request.instance_id, "request_id": request.request_id}))
                    .map_err(text)?,
            )?;
            if fault == Some(FaultKind::DuplicateCreate) {
                write_atomic(
                    &resources.join(format!(
                        "{}.duplicate",
                        resource_name(&request.organization, &request.instance_id)
                    )),
                    &serde_json::to_vec(&json!({"organization": request.organization, "instance_id": request.instance_id, "request_id": request.request_id}))
                        .map_err(text)?,
                )?;
            }
            "created"
        }
        "deprovision" if record.exists() => {
            fs::remove_file(&record).map_err(text)?;
            sync_directory(&resources)?;
            "deleted"
        }
        "deprovision" => "already_absent",
        _ => "inspected",
    };
    append_log(&inventory, &request, effect)?;
    match fault {
        Some(FaultKind::AfterEffectNoReply) => return Ok(NO_REPLY_EXIT),
        Some(FaultKind::HangAfterEffect)
            if wait_at_barrier(&inventory, &request, "after_effect")?
                == BarrierOutcome::Aborted =>
        {
            return Ok(NO_REPLY_EXIT);
        }
        _ => {}
    }
    let request_id = if fault == Some(FaultKind::StaleReply) {
        format!("{}-stale", request.request_id)
    } else {
        request.request_id.clone()
    };
    let reply = json!({
        "schema": DRIVER_SCHEMA,
        "organization": request.organization,
        "request_id": request_id,
        "instance_id": request.instance_id,
        "state": if record.exists() { "present" } else { "absent" },
    });
    // The coordinator may have been killed: a closed pipe is a lost reply.
    let mut stdout = std::io::stdout();
    if stdout
        .write_all(reply.to_string().as_bytes())
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return Ok(NO_REPLY_EXIT);
    }
    Ok(0)
}

fn resource_name(organization: &str, instance: &str) -> String {
    format!("{organization}--{instance}.json")
}

fn take_fault(inventory: &Path, request: &DriverRequest) -> Result<Option<FaultKind>> {
    let path = inventory.join("faults.json");
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_FAULT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() as u64 > MAX_FAULT_BYTES {
        return Err("fault plan exceeds its bound".into());
    }
    let fault: Fault = serde_json::from_slice(&bytes).map_err(text)?;
    if fault.operation != request.operation || fault.instance_id != request.instance_id {
        return Ok(None);
    }
    // One-shot: a renamed fault can never trigger again.
    fs::rename(&path, inventory.join("faults.consumed.json")).map_err(text)?;
    sync_directory(inventory)?;
    Ok(Some(fault.kind))
}

fn wait_at_barrier(
    inventory: &Path,
    request: &DriverRequest,
    phase: &str,
) -> Result<BarrierOutcome> {
    let barrier = inventory.join("barrier.json");
    write_atomic(
        &barrier,
        &serde_json::to_vec(&json!({"pid": std::process::id(), "operation": request.operation, "instance_id": request.instance_id, "phase": phase}))
            .map_err(text)?,
    )?;
    let started = Instant::now();
    let outcome = loop {
        if inventory.join("barrier.abort").exists() {
            break BarrierOutcome::Aborted;
        }
        if inventory.join("barrier.release").exists() {
            break BarrierOutcome::Released;
        }
        if started.elapsed() >= DRIVER_BARRIER_MAX_WAIT {
            break BarrierOutcome::Aborted;
        }
        std::thread::sleep(BARRIER_POLL);
    };
    for name in ["barrier.release", "barrier.abort", "barrier.json"] {
        match fs::remove_file(inventory.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    sync_directory(inventory)?;
    Ok(outcome)
}

fn append_log(inventory: &Path, request: &DriverRequest, effect: &str) -> Result<()> {
    let path = inventory.join("operations.jsonl");
    let seq = read_log(inventory)?.len() as u64 + 1;
    let entry = LogEntry {
        seq,
        operation: request.operation.clone(),
        organization: request.organization.clone(),
        instance_id: request.instance_id.clone(),
        request_id: request.request_id.clone(),
        effect: effect.into(),
    };
    let mut line = serde_json::to_vec(&entry).map_err(text)?;
    line.push(b'\n');
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(text)?;
    file.write_all(&line).map_err(text)?;
    file.sync_all().map_err(text)
}

fn read_log(inventory: &Path) -> Result<Vec<LogEntry>> {
    match fs::read_to_string(inventory.join("operations.jsonl")) {
        Ok(contents) => contents
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).map_err(text))
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("record has no parent directory")?;
    let staged = path.with_extension("staged");
    {
        let mut file = fs::File::create(&staged).map_err(text)?;
        file.write_all(bytes).map_err(text)?;
        file.sync_all().map_err(text)?;
    }
    fs::rename(&staged, path).map_err(text)?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(text)
}

// ---------------------------------------------------------------------------
// Coordinator: one real action per process.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupInput {
    driver_path: PathBuf,
    driver_blake3: String,
    minimum: u32,
    maximum: u32,
}

fn coordinator(args: &[String]) -> i32 {
    let outcome = coordinator_result(args);
    let document = match &outcome {
        Ok(Ok(value)) => json!({"ok": value}),
        Ok(Err(error)) => json!({"error": error}),
        Err(error) => {
            eprintln!("coordinator infrastructure failure: {error}");
            return 1;
        }
    };
    println!("{document}");
    0
}

/// Outer error: the coordinator itself failed. Inner error: the real action refused.
fn coordinator_result(args: &[String]) -> Result<Result<Value>> {
    let [db_flag, database, action, input] = args else {
        return Err("expected --db PATH ACTION JSON".into());
    };
    if db_flag != "--db" {
        return Err("expected --db PATH ACTION JSON".into());
    }
    let input: Value = serde_json::from_str(input).map_err(text)?;
    let mut db = Connection::open(database).map_err(text)?;
    accounts::init(&db)?;
    finance::init(&db)?;
    fleet::init(&db)?;
    secrets::init(&db)?;
    let actor = Actor {
        peer: PEER_BYTE.repeat(PEER_LENGTH),
        organization: ORGANIZATION.into(),
        role: "owner".into(),
    };
    let now = now_ms()?;
    Ok(match action.as_str() {
        "setup" => setup(&mut db, &actor, input, now),
        "operator" => serde_json::from_value::<SetupInput>(input)
            .map_err(text)
            .and_then(|setup| operator_config(&db, setup))
            .and_then(|config| fleet::configure_trusted(&mut db, &config).map(|()| json!({}))),
        "configure" | "provision" | "deprovision" | "reconcile" | "status" => {
            fleet::handle(&mut db, &actor, action, input, now)
        }
        "secret-setup" => secret_setup(&mut db, &actor, input, now),
        "secret-issue" => secret_issue(&mut db, &actor, input),
        _ => Err("unknown coordinator action".into()),
    })
}

fn setup(db: &mut Connection, actor: &Actor, input: Value, now: u64) -> Result<Value> {
    let setup: SetupInput = serde_json::from_value(input).map_err(text)?;
    accounts::bootstrap(db, &actor.peer, &actor.organization)?;
    finance::handle(
        db,
        actor,
        "account.configure",
        json!({"request_id": "qualification-account", "expected_revision": 0, "currency": PRICE_CURRENCY, "cap_minor": ACCOUNT_CAP_MINOR, "request_max_minor": ACCOUNT_CAP_MINOR, "max_concurrent": ACCOUNT_CONCURRENCY}),
        now,
    )?;
    let price = finance::handle(
        db,
        actor,
        "price.admit",
        json!({"class": PRICE_CLASS, "currency": PRICE_CURRENCY, "unit_minor": 1, "unit_ms": 1000, "max_units": 1}),
        now,
    )?;
    let price_id = price
        .get("price_id")
        .and_then(Value::as_str)
        .ok_or("price admission returned no identity")?
        .to_owned();
    let minimum = setup.minimum;
    let maximum = setup.maximum;
    let mut config = operator_config(db, setup)?;
    config.pricing = Some(Pricing {
        price_id: price_id.clone(),
        class: PRICE_CLASS.into(),
        max_units: 1,
    });
    fleet::configure_trusted(db, &config)?;
    fleet::handle(
        db,
        actor,
        "configure",
        json!({"minimum": minimum, "maximum": maximum, "slots_per_instance": 1, "expected_revision": 0}),
        now,
    )?;
    Ok(json!({"price_id": price_id}))
}

// r[impl kiln.hosted.fleet.secret_generation]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretSetup {
    directory: PathBuf,
}

fn secret_setup(db: &mut Connection, actor: &Actor, input: Value, now: u64) -> Result<Value> {
    let input: SecretSetup = serde_json::from_value(input).map_err(text)?;
    let key = input.directory.join("issuer.seed");
    fs::write(&key, [17_u8; 32]).map_err(text)?;
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).map_err(text)?;
    let program = input.directory.join("secret-resolver");
    let script = format!(
        "#!/bin/sh\n[ \"$1\" = get ] && [ \"$2\" = {SECRET_NAME} ] || exit 9\nprintf '{SECRET_VALUE}\\n'\n"
    );
    fs::write(&program, &script).map_err(text)?;
    fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).map_err(text)?;
    secrets::configure(
        db,
        &secrets::Config {
            issuer: "https://fleet-issuer.example.test".into(),
            signing_key_path: key,
            audiences: vec![SECRET_AUDIENCE.into()],
            token_lifetime_seconds: SECRET_LIFETIME_SECONDS,
            resolvers: vec![secrets::Resolver {
                id: "fake-provider".into(),
                organization: actor.organization.clone(),
                project: "qualification".into(),
                reference: SecretReference {
                    program_path: program,
                    program_blake3: blake3::hash(script.as_bytes()).to_hex().to_string(),
                    project_root: input.directory,
                    profile: "qualification".into(),
                    provider: None,
                    secret_key: SECRET_NAME.into(),
                },
            }],
        },
    )?;
    secrets::handle(
        db,
        actor,
        "register",
        json!({"project":"qualification","name":SECRET_NAME,"resolver":"fake-provider","expected_revision":0,"expires_ms":now.checked_add(SECRET_REFERENCE_MS).ok_or("secret time overflow")?}),
        now,
    )?;
    Ok(json!({"configured":true}))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretIssue {
    job: String,
    instance_id: String,
    revision: u64,
    request_id: String,
    token_path: PathBuf,
    now_ms: u64,
}

fn secret_issue(db: &mut Connection, actor: &Actor, input: Value) -> Result<Value> {
    let input: SecretIssue = serde_json::from_value(input).map_err(text)?;
    let selected = secrets::FleetGeneration {
        instance_id: input.instance_id,
        revision: input.revision,
        request_id: input.request_id,
    };
    let observed = fleet::require_active_generation(db, &actor.organization, &selected)?;
    let binding = secrets::JobBinding {
        organization: actor.organization.clone(),
        project: "qualification".into(),
        job: input.job,
        admission_identity: "fake-provider-admission".into(),
        fleet: observed.clone(),
        expires_ms: input
            .now_ms
            .checked_add(SECRET_JOB_MS)
            .ok_or("secret time overflow")?,
        audiences: vec![SECRET_AUDIENCE.into()],
        secrets: vec![secrets::SecretGrant {
            name: SECRET_NAME.into(),
            revision: 1,
        }],
    };
    let cap = secrets::begin_job(db, &binding, input.now_ms)?;
    let delivered = secrets::resolve_for_job(db, cap.as_str(), SECRET_NAME, input.now_ms)?;
    if delivered
        .value
        .as_str()
        .map_err(|_| "secret value is not UTF-8")?
        != SECRET_VALUE
    {
        return Err("fake provider received wrong secret value".into());
    }
    let token = secrets::exchange(db, cap.as_str(), SECRET_AUDIENCE, input.now_ms)?;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(input.token_path)
        .map_err(text)?;
    file.write_all(token.as_str().as_bytes()).map_err(text)?;
    file.sync_all().map_err(text)?;
    Ok(
        json!({"fleet":observed,"delivery_expires_ms":delivered.expires_ms,"token_expires_ms":input.now_ms / 1000 + SECRET_LIFETIME_SECONDS}),
    )
}

/// Separate fake-provider process checks the retained resource-server boundary.
fn inspect_secret(args: &[String]) -> Result<Value> {
    let [
        db_flag,
        database,
        token_flag,
        token_path,
        job_flag,
        job,
        at_flag,
        at,
    ] = args
    else {
        return Err("expected --db PATH --token PATH --job ID --at MS".into());
    };
    if db_flag != "--db" || token_flag != "--token" || job_flag != "--job" || at_flag != "--at" {
        return Err("invalid secret inspection arguments".into());
    }
    let db = Connection::open(database).map_err(text)?;
    secrets::init(&db)?;
    fleet::init(&db)?;
    let token = fs::read_to_string(token_path).map_err(text)?;
    let now = at.parse::<u64>().map_err(text)?;
    let accepted = secrets::verify_token(&db, &token, SECRET_AUDIENCE, now);
    let active: bool = db
        .query_row(
            "SELECT active FROM hosted_secret_jobs WHERE org=?1 AND job=?2",
            [ORGANIZATION, job.as_str()],
            |row| row.get(0),
        )
        .map_err(text)?;
    Ok(
        json!({"accepted":accepted.is_ok(),"fleet":accepted.ok().map(|claims| claims.fleet),"job_active":active}),
    )
}

/// Operator profile for a driver generation; pricing is retained from the current profile.
fn operator_config(db: &Connection, setup: SetupInput) -> Result<Config> {
    let pricing = db
        .query_row(
            "SELECT config FROM hosted_fleet_operator WHERE singleton=1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(text)?
        .map(|raw| serde_json::from_str::<Config>(&raw).map_err(text))
        .transpose()?
        .and_then(|config| config.pricing);
    Ok(Config {
        driver: Executable {
            path: setup.driver_path,
            blake3: setup.driver_blake3,
        },
        organizations: vec![ORGANIZATION.into()],
        pricing,
        minimum: setup.minimum,
        maximum: setup.maximum,
        slots_per_instance: 1,
        timeout_ms: DRIVER_TIMEOUT_MS,
    })
}

fn now_ms() -> Result<u64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(text)?;
    u64::try_from(elapsed.as_millis()).map_err(text)
}

// ---------------------------------------------------------------------------
// Pure inventory oracle.

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct InstanceRow {
    instance_id: String,
    state: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum Violation {
    DuplicateCreate,
    RetiredWhilePresent,
    UntrackedResource,
    CapacityExceeded,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct OracleReport {
    driver_invocations: BTreeMap<String, u64>,
    created: u64,
    deleted: u64,
    present: u64,
    rows_by_state: BTreeMap<String, u64>,
    violations: BTreeSet<Violation>,
}

/// Compare retained fleet rows with provider records and the provider's own log.
fn oracle(
    rows: &[InstanceRow],
    resources: &[String],
    log: &[LogEntry],
    maximum: u32,
) -> OracleReport {
    let mut report = OracleReport::default();
    let mut records: BTreeMap<&str, u64> = BTreeMap::new();
    for instance in resources {
        *records.entry(instance.as_str()).or_default() += 1;
    }
    let mut live: BTreeMap<&str, i64> = BTreeMap::new();
    for entry in log {
        *report
            .driver_invocations
            .entry(entry.operation.clone())
            .or_default() += 1;
        match entry.effect.as_str() {
            "created" => {
                report.created += 1;
                let count = live.entry(entry.instance_id.as_str()).or_default();
                *count += 1;
                if *count > 1 {
                    report.violations.insert(Violation::DuplicateCreate);
                }
            }
            "deleted" => {
                report.deleted += 1;
                *live.entry(entry.instance_id.as_str()).or_default() -= 1;
            }
            _ => {}
        }
    }
    for row in rows {
        *report.rows_by_state.entry(row.state.clone()).or_default() += 1;
    }
    for (instance, count) in &records {
        if *count > 1 {
            report.violations.insert(Violation::DuplicateCreate);
        }
        match rows.iter().find(|row| row.instance_id == *instance) {
            Some(row) if row.state == "retired" => {
                report.violations.insert(Violation::RetiredWhilePresent);
            }
            Some(_) => {}
            None => {
                report.violations.insert(Violation::UntrackedResource);
            }
        }
    }
    report.present = records.len() as u64;
    if report.present > u64::from(maximum) {
        report.violations.insert(Violation::CapacityExceeded);
    }
    report
}

// ---------------------------------------------------------------------------
// Harness.

struct Tools {
    harness: PathBuf,
    nix_store: PathBuf,
    bash: PathBuf,
    driver_store_path: PathBuf,
}

struct Scenario<'a> {
    tools: &'a Tools,
    directory: PathBuf,
    inventory: PathBuf,
    database: PathBuf,
    maximum: u32,
    steps: Vec<Value>,
    checks: Vec<Value>,
    drivers: Vec<Value>,
}

impl<'a> Scenario<'a> {
    fn new(tools: &'a Tools, out: &Path, name: &str, maximum: u32) -> Result<Self> {
        let directory = out.join(name);
        let inventory = directory.join("inventory");
        fs::create_dir_all(inventory.join("resources")).map_err(text)?;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).map_err(text)?;
        let mut scenario = Self {
            tools,
            database: directory.join("fleet.sqlite"),
            directory,
            inventory,
            maximum,
            steps: Vec::new(),
            checks: Vec::new(),
            drivers: Vec::new(),
        };
        let (path, blake3) = scenario.admit_driver(1)?;
        let setup = scenario.call(
            "setup",
            json!({"driver_path": path, "driver_blake3": blake3, "minimum": 0, "maximum": maximum}),
        )?;
        scenario.check("setup admitted", setup.get("ok").is_some(), &setup);
        Ok(scenario)
    }

    /// Add one operator-reviewed wrapper generation to the Nix store.
    fn admit_driver(&mut self, generation: u32) -> Result<(String, String)> {
        let wrapper = self
            .directory
            .join(format!("kiln-fleet-fake-driver-g{generation}"));
        let script = format!(
            "#!{}\n# fleet qualification driver generation {generation}\nexec {} driver --inventory {} \"$@\"\n",
            self.tools.bash.display(),
            self.tools.driver_store_path.display(),
            self.inventory.display()
        );
        fs::write(&wrapper, script).map_err(text)?;
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o555)).map_err(text)?;
        let stored = nix_store_add(&self.tools.nix_store, &wrapper)?;
        let blake3 = file_blake3(&stored)?;
        let path = stored.display().to_string();
        self.drivers
            .push(json!({"generation": generation, "path": path, "blake3": blake3}));
        Ok((path, blake3))
    }

    fn call(&mut self, action: &str, input: Value) -> Result<Value> {
        let output = Command::new(&self.tools.harness)
            .args([
                "coordinator".into(),
                "--db".into(),
                self.database.display().to_string(),
                action.into(),
                input.to_string(),
            ])
            .stdin(Stdio::null())
            .output()
            .map_err(text)?;
        if !output.status.success() {
            return Err(format!(
                "coordinator {action} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let result: Value = serde_json::from_slice(&output.stdout).map_err(text)?;
        self.steps
            .push(json!({"action": action, "input": input, "result": result}));
        Ok(result)
    }

    /// Start one action, kill its coordinator while the driver waits at the barrier.
    fn interrupted(&mut self, action: &str, input: Value, release: BarrierOutcome) -> Result<()> {
        let mut child = Command::new(&self.tools.harness)
            .args([
                "coordinator".into(),
                "--db".into(),
                self.database.display().to_string(),
                action.into(),
                input.to_string(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .map_err(text)?;
        let barrier = self.inventory.join("barrier.json");
        let started = Instant::now();
        while !barrier.exists() {
            if started.elapsed() >= HARNESS_BARRIER_MAX_WAIT {
                let _ignored = child.kill();
                return Err("driver never reached its barrier".into());
            }
            std::thread::sleep(BARRIER_POLL);
        }
        let pid = serde_json::from_slice::<Value>(&fs::read(&barrier).map_err(text)?)
            .map_err(text)?
            .get("pid")
            .and_then(Value::as_u64)
            .ok_or("barrier has no driver pid")?;
        child.kill().map_err(text)?;
        let status = child.wait().map_err(text)?;
        let marker = match release {
            BarrierOutcome::Released => "barrier.release",
            BarrierOutcome::Aborted => "barrier.abort",
        };
        fs::write(self.inventory.join(marker), b"").map_err(text)?;
        let process = PathBuf::from(format!("/proc/{pid}"));
        let started = Instant::now();
        while process.exists() {
            if started.elapsed() >= DRIVER_EXIT_MAX_WAIT {
                return Err("orphaned driver did not exit after its barrier".into());
            }
            std::thread::sleep(BARRIER_POLL);
        }
        self.steps.push(json!({
            "action": action,
            "input": input,
            "interrupted": {"coordinator_killed": !status.success(), "driver": marker},
        }));
        Ok(())
    }

    fn fault(&mut self, operation: &str, instance: &str, kind: FaultKind) -> Result<()> {
        let fault = Fault {
            operation: operation.into(),
            instance_id: instance.into(),
            kind,
        };
        write_atomic(
            &self.inventory.join("faults.json"),
            &serde_json::to_vec(&fault).map_err(text)?,
        )?;
        self.steps.push(json!({"fault": fault}));
        Ok(())
    }

    fn mutate(
        &mut self,
        action: &str,
        instance: &str,
        request: &str,
        revision: u64,
    ) -> Result<Value> {
        self.call(
            action,
            json!({"instance_id": instance, "request_id": request, "expected_revision": revision}),
        )
    }

    fn check(&mut self, name: &str, passed: bool, observed: &Value) {
        self.checks
            .push(json!({"check": name, "passed": passed, "observed": observed}));
    }

    fn expect_state(&mut self, name: &str, result: &Value, state: &str) {
        let observed = result
            .get("ok")
            .and_then(|ok| ok.get("state"))
            .and_then(Value::as_str);
        self.check(name, observed == Some(state), result);
    }

    fn expect_refusal(&mut self, name: &str, result: &Value, fragment: &str) {
        let refused = result
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains(fragment));
        self.check(name, refused, result);
    }

    fn setup_secrets(&mut self) -> Result<()> {
        let result = self.call("secret-setup", json!({"directory":self.directory}))?;
        self.check(
            "secret policy admitted",
            result.get("ok").is_some(),
            &result,
        );
        Ok(())
    }

    fn issue_secret(
        &mut self,
        job: &str,
        instance: &str,
        revision: u64,
        request: &str,
        at: u64,
    ) -> Result<Value> {
        let token_path = self.directory.join(format!("{job}.token"));
        self.call("secret-issue", json!({"job":job,"instance_id":instance,"revision":revision,"request_id":request,"now_ms":at,"token_path":token_path}))
    }

    fn inspect_secret(&mut self, job: &str, at: u64) -> Result<Value> {
        let output = Command::new(&self.tools.driver_store_path)
            .args([
                "secret-inspect",
                "--db",
                &self.database.display().to_string(),
                "--token",
                &self
                    .directory
                    .join(format!("{job}.token"))
                    .display()
                    .to_string(),
                "--job",
                job,
                "--at",
                &at.to_string(),
            ])
            .output()
            .map_err(text)?;
        if !output.status.success() {
            return Err(format!(
                "fake provider secret inspection failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let observation: Value = serde_json::from_slice(&output.stdout).map_err(text)?;
        self.steps.push(json!({"fake_provider_secret_inspection":{"job":job,"at_ms":at,"observation":observation}}));
        Ok(observation)
    }

    fn provision_effects(&self) -> Result<usize> {
        Ok(read_log(&self.inventory)?
            .iter()
            .filter(|entry| entry.operation != "inspect")
            .count())
    }

    fn rows(&self) -> Result<Vec<InstanceRow>> {
        let db = Connection::open(&self.database).map_err(text)?;
        let mut statement = db
            .prepare("SELECT id,state FROM hosted_fleet_instances WHERE org=?1 ORDER BY id")
            .map_err(text)?;
        let mut rows = Vec::new();
        for row in statement
            .query_map([ORGANIZATION], |row| {
                Ok(InstanceRow {
                    instance_id: row.get(0)?,
                    state: row.get(1)?,
                })
            })
            .map_err(text)?
        {
            rows.push(row.map_err(text)?);
        }
        Ok(rows)
    }

    fn resources(&self) -> Result<Vec<String>> {
        let prefix = format!("{ORGANIZATION}--");
        let mut instances = Vec::new();
        for entry in fs::read_dir(self.inventory.join("resources")).map_err(text)? {
            let name = entry.map_err(text)?.file_name();
            let name = name.to_str().ok_or("resource name is not UTF-8")?;
            let Some(rest) = name.strip_prefix(&prefix) else {
                continue;
            };
            let instance = rest
                .strip_suffix(".json.duplicate")
                .or_else(|| rest.strip_suffix(".json"));
            if let Some(instance) = instance {
                instances.push(instance.to_owned());
            }
        }
        instances.sort();
        Ok(instances)
    }

    fn finance_states(&self) -> Result<BTreeMap<String, u64>> {
        let db = Connection::open(&self.database).map_err(text)?;
        let mut statement = db
            .prepare("SELECT state,count(*) FROM finance_reservations WHERE org=?1 GROUP BY state")
            .map_err(text)?;
        let mut states = BTreeMap::new();
        for row in statement
            .query_map([ORGANIZATION], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(text)?
        {
            let (state, count) = row.map_err(text)?;
            states.insert(state, u64::try_from(count).map_err(text)?);
        }
        Ok(states)
    }

    fn finish(self, name: &str, kind: &str, expected: &[Violation]) -> Result<Value> {
        let report = oracle(
            &self.rows()?,
            &self.resources()?,
            &read_log(&self.inventory)?,
            self.maximum,
        );
        let expected: BTreeSet<Violation> = expected.iter().cloned().collect();
        let violations_match = report.violations == expected;
        let checks_pass = self
            .checks
            .iter()
            .all(|check| check.get("passed").and_then(Value::as_bool) == Some(true));
        Ok(json!({
            "scenario": name,
            "kind": kind,
            "passed": violations_match && checks_pass,
            "expected_violations": expected,
            "oracle": report,
            "finance_reservations": self.finance_states()?,
            "drivers": self.drivers,
            "checks": self.checks,
            "steps": self.steps,
        }))
    }
}

fn run(args: &[String]) -> Result<bool> {
    let mut options: BTreeMap<&str, &String> = BTreeMap::new();
    for pair in args.chunks(2) {
        let [flag, value] = pair else {
            return Err("arguments must be --flag VALUE pairs".into());
        };
        if !matches!(flag.as_str(), "--nix-store" | "--bash" | "--out")
            || options.insert(flag.as_str(), value).is_some()
        {
            return Err(format!("unexpected or repeated argument {flag}"));
        }
    }
    let required = |flag: &str| -> Result<PathBuf> {
        let value = options
            .get(flag)
            .ok_or_else(|| format!("{flag} is required"))?;
        let path = PathBuf::from(value.as_str());
        if !path.is_absolute() {
            return Err(format!("{flag} must be absolute"));
        }
        Ok(path)
    };
    let nix_store = required("--nix-store")?;
    let bash = required("--bash")?;
    let out = required("--out")?;
    fs::create_dir(&out).map_err(|error| format!("--out must be a new directory: {error}"))?;
    fs::set_permissions(&out, fs::Permissions::from_mode(0o700)).map_err(text)?;
    let harness = std::env::current_exe().map_err(text)?;
    let driver_store_path = nix_store_add(&nix_store, &harness)?;
    let tools = Tools {
        harness: harness.clone(),
        nix_store: nix_store.clone(),
        bash: bash.clone(),
        driver_store_path: driver_store_path.clone(),
    };
    let scenarios = vec![
        happy_path(&tools, &out)?,
        lost_ack_after_create(&tools, &out)?,
        killed_after_create(&tools, &out)?,
        killed_before_create(&tools, &out)?,
        deprovision_lost_ack(&tools, &out)?,
        duplicate_delivery(&tools, &out)?,
        stale_reply(&tools, &out)?,
        operator_generation_change(&tools, &out)?,
        retired_reappearance(&tools, &out)?,
        secret_worker_generations(&tools, &out)?,
        capacity_boundaries(&tools, &out)?,
        duplicate_create_control(&tools, &out)?,
        premature_release_control(&tools, &out)?,
    ];
    let passed = scenarios
        .iter()
        .all(|scenario| scenario.get("passed").and_then(Value::as_bool) == Some(true));
    let receipt = json!({
        "schema": RECEIPT_SCHEMA,
        "issue": 72,
        "passed": passed,
        "production_provider_qualified": false,
        "source_blake3": {
            "crates/kiln-hosted/src/fleet.rs": blake3::hash(FLEET_SOURCE).to_hex().to_string(),
            "crates/kiln-hosted/src/secrets.rs": blake3::hash(SECRETS_SOURCE).to_hex().to_string(),
            "crates/kiln-hosted/examples/fleet_qualification.rs": blake3::hash(HARNESS_SOURCE).to_hex().to_string(),
        },
        "executables": {
            "harness": {"path": harness, "blake3": file_blake3(&harness)?},
            "driver_store_path": driver_store_path,
            "nix_store": nix_store,
            "bash": bash,
        },
        "scenarios": scenarios,
        "gaps": [],
        "non_claims": [
            "The fake provider is a local inventory, not a cloud API or purchased capacity.",
            "Added Nix store paths are unrooted and garbage-collectable.",
            "Provider presence is not usage metering: fleet finance reservations stay unknown until separately settled.",
            "Not production provider, capacity, availability, independent physical placement or third-party immediate JWT revocation qualification.",
        ],
    });
    write_atomic(
        &out.join("receipt.json"),
        &serde_json::to_vec_pretty(&receipt).map_err(text)?,
    )?;
    println!("{}", out.join("receipt.json").display());
    Ok(passed)
}

fn happy_path(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "happy-path", 1)?;
    let provisioned = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("provision becomes active", &provisioned, "active");
    let deprovisioned = s.mutate("deprovision", "w1", "d1", 1)?;
    s.expect_state("deprovision becomes retired", &deprovisioned, "retired");
    s.finish("happy-path", "recovery", &[])
}

fn lost_ack_after_create(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "lost-ack-after-create", 1)?;
    s.fault("provision", "w1", FaultKind::AfterEffectNoReply)?;
    let lost = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("lost acknowledgement stays unknown", &lost, "unknown");
    let blocked = s.mutate("provision", "w2", "p2", 0)?;
    s.expect_refusal(
        "unknown instance retains capacity",
        &blocked,
        "capacity exhausted",
    );
    let reconciled = s.mutate("reconcile", "w1", "c1", 1)?;
    s.expect_state(
        "reconcile observes the created resource",
        &reconciled,
        "active",
    );
    let effects = s.provision_effects()?;
    s.check("exactly one provider effect", effects == 1, &json!(effects));
    s.finish("lost-ack-after-create", "recovery", &[])
}

fn killed_after_create(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "coordinator-killed-after-create", 1)?;
    s.fault("provision", "w1", FaultKind::HangAfterEffect)?;
    let request = json!({"instance_id": "w1", "request_id": "p1", "expected_revision": 0});
    s.interrupted("provision", request, BarrierOutcome::Released)?;
    let status = s.call("status", json!({}))?;
    let provisioning = status_state(&status, "w1") == Some("provisioning".into());
    s.check(
        "killed coordinator leaves the committed intent",
        provisioning,
        &status,
    );
    let redelivered = s.mutate("provision", "w1", "p1", 0)?;
    let retained = redelivered.get("ok") == Some(&json!({"state": "unknown", "rerun": false}));
    s.check(
        "redelivery returns the retained unknown result",
        retained,
        &redelivered,
    );
    let effects = s.provision_effects()?;
    s.check(
        "redelivery never redispatches",
        effects == 1,
        &json!(effects),
    );
    let reconciled = s.mutate("reconcile", "w1", "c1", 1)?;
    s.expect_state(
        "reconcile observes the created resource",
        &reconciled,
        "active",
    );
    s.finish("coordinator-killed-after-create", "recovery", &[])
}

fn killed_before_create(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "coordinator-killed-before-create", 1)?;
    s.fault("provision", "w1", FaultKind::HangBeforeEffect)?;
    let request = json!({"instance_id": "w1", "request_id": "p1", "expected_revision": 0});
    s.interrupted("provision", request, BarrierOutcome::Aborted)?;
    let effects = s.provision_effects()?;
    s.check("no provider effect happened", effects == 0, &json!(effects));
    let reconciled = s.mutate("reconcile", "w1", "c1", 1)?;
    s.expect_state("reconcile observes absence", &reconciled, "retired");
    let next = s.mutate("provision", "w2", "p2", 0)?;
    s.expect_state("first eligible provision after recovery", &next, "active");
    s.finish("coordinator-killed-before-create", "recovery", &[])
}

fn deprovision_lost_ack(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "deprovision-lost-ack", 1)?;
    let provisioned = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("provision becomes active", &provisioned, "active");
    s.fault("deprovision", "w1", FaultKind::AfterEffectNoReply)?;
    let lost = s.mutate("deprovision", "w1", "d1", 1)?;
    s.expect_state(
        "lost teardown acknowledgement stays unknown",
        &lost,
        "unknown",
    );
    let blocked = s.mutate("provision", "w2", "p2", 0)?;
    s.expect_refusal(
        "uncertain teardown retains capacity",
        &blocked,
        "capacity exhausted",
    );
    let reconciled = s.mutate("reconcile", "w1", "c1", 2)?;
    s.expect_state(
        "reconcile observes completed teardown",
        &reconciled,
        "retired",
    );
    let next = s.mutate("provision", "w2", "p2", 0)?;
    s.expect_state("capacity returns after reconciliation", &next, "active");
    s.finish("deprovision-lost-ack", "recovery", &[])
}

fn duplicate_delivery(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "duplicate-delivery", 1)?;
    let first = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("provision becomes active", &first, "active");
    let repeated = s.mutate("provision", "w1", "p1", 0)?;
    s.check(
        "identical redelivery returns the retained result",
        repeated == first,
        &repeated,
    );
    let effects = s.provision_effects()?;
    s.check(
        "redelivery never redispatches",
        effects == 1,
        &json!(effects),
    );
    let conflict = s.mutate("provision", "w2", "p1", 0)?;
    s.expect_refusal(
        "reused request identity conflicts",
        &conflict,
        "identity conflict",
    );
    s.finish("duplicate-delivery", "recovery", &[])
}

fn stale_reply(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "stale-reply", 1)?;
    s.fault("provision", "w1", FaultKind::StaleReply)?;
    let stale = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("stale receipt is not accepted", &stale, "unknown");
    let reconciled = s.mutate("reconcile", "w1", "c1", 1)?;
    s.expect_state(
        "reconcile observes the created resource",
        &reconciled,
        "active",
    );
    s.finish("stale-reply", "recovery", &[])
}

fn operator_generation_change(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "operator-generation-change", 1)?;
    let provisioned = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("provision becomes active", &provisioned, "active");
    let (path, blake3) = s.admit_driver(2)?;
    let generation =
        json!({"driver_path": path, "driver_blake3": blake3, "minimum": 0, "maximum": 1});
    let pinned = s.call("operator", generation.clone())?;
    s.expect_refusal(
        "occupied fleet pins the operator generation",
        &pinned,
        "pinned by active/uncertain",
    );
    let retired = s.mutate("deprovision", "w1", "d1", 1)?;
    s.expect_state("deprovision becomes retired", &retired, "retired");
    let changed = s.call("operator", generation)?;
    s.check(
        "empty fleet admits a new generation",
        changed.get("ok").is_some(),
        &changed,
    );
    let stale = s.mutate("provision", "w2", "p2", 0)?;
    s.expect_refusal(
        "stale organization selection is refused",
        &stale,
        "authority changed",
    );
    let reconfigured = s.call(
        "configure",
        json!({"minimum": 0, "maximum": 1, "slots_per_instance": 1, "expected_revision": 1}),
    )?;
    s.check(
        "organization reselects the new generation",
        reconfigured.get("ok").is_some(),
        &reconfigured,
    );
    let next = s.mutate("provision", "w2", "p2", 0)?;
    s.expect_state("new generation provisions", &next, "active");
    s.finish("operator-generation-change", "recovery", &[])
}

fn retired_reappearance(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "retired-reappearance", 1)?;
    let provisioned = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("provision becomes active", &provisioned, "active");
    let retired = s.mutate("deprovision", "w1", "d1", 1)?;
    s.expect_state("deprovision becomes retired", &retired, "retired");
    write_atomic(
        &s.inventory
            .join("resources")
            .join(resource_name(ORGANIZATION, "w1")),
        br#"{"reappeared":true}"#,
    )?;
    s.steps
        .push(json!({"provider": "old resource w1 reappeared"}));
    let reconcile = s.mutate("reconcile", "w1", "c1", 2)?;
    s.expect_refusal(
        "retired instance cannot be reconciled back",
        &reconcile,
        "no uncertain current fleet request",
    );
    let reuse = s.mutate("provision", "w1", "p2", 0)?;
    s.expect_refusal(
        "retired instance identity cannot be reprovisioned",
        &reuse,
        "new-instance revision denied",
    );
    let status = s.call("status", json!({}))?;
    let still_retired = status_state(&status, "w1") == Some("retired".into());
    s.check("retired generation stays retired", still_retired, &status);
    s.finish(
        "retired-reappearance",
        "recovery",
        &[Violation::RetiredWhilePresent],
    )
}

// r[verify kiln.hosted.fleet.secret_generation.scenario.current]
// r[verify kiln.hosted.fleet.secret_generation.scenario.stale]
// r[verify kiln.hosted.fleet.secret_generation.scenario.retirement]
// r[verify kiln.hosted.fleet.secret_generation.scenario.restart]
fn secret_worker_generations(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "secret-worker-generations", 1)?;
    s.setup_secrets()?;
    let started = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("worker generation one active", &started, "active");
    let now = now_ms()?;
    let issued = s.issue_secret("job-one", "w1", 1, "p1", now)?;
    s.check(
        "current generation delivers secret and token",
        issued
            .pointer("/ok/fleet/request_id")
            .and_then(Value::as_str)
            == Some("p1")
            && issued
                .pointer("/ok/delivery_expires_ms")
                .and_then(Value::as_u64)
                == now.checked_add(60_000),
        &issued,
    );
    let current = s.inspect_secret("job-one", now)?;
    s.check(
        "fake provider accepts current generation",
        current.get("accepted").and_then(Value::as_bool) == Some(true)
            && current
                .pointer("/fleet/instance_id")
                .and_then(Value::as_str)
                == Some("w1"),
        &current,
    );
    let expired = s.inspect_secret("job-one", now + SECRET_LIFETIME_SECONDS * 1000)?;
    s.check(
        "fake provider refuses expired generation token",
        expired.get("accepted").and_then(Value::as_bool) == Some(false),
        &expired,
    );
    let missing = s.issue_secret("job-missing", "missing", 1, "p1", now)?;
    s.expect_refusal(
        "unknown generation cannot receive secret",
        &missing,
        "generation not active",
    );
    let stale = s.issue_secret("job-stale", "w1", 2, "p1", now)?;
    s.expect_refusal(
        "mismatched revision cannot receive secret",
        &stale,
        "generation not active",
    );
    s.fault("deprovision", "w1", FaultKind::AfterEffectNoReply)?;
    let unknown = s.mutate("deprovision", "w1", "d1", 1)?;
    s.expect_state("lost teardown receipt is unknown", &unknown, "unknown");
    let uncertain = s.inspect_secret("job-one", now)?;
    s.check(
        "uncertain teardown denies but does not claim revocation",
        uncertain.get("accepted").and_then(Value::as_bool) == Some(false)
            && uncertain.get("job_active").and_then(Value::as_bool) == Some(true),
        &uncertain,
    );
    let reconciled = s.mutate("reconcile", "w1", "inspect-one", 2)?;
    s.expect_state("observed absence retires worker", &reconciled, "retired");
    let retired = s.inspect_secret("job-one", now)?;
    s.check(
        "fake provider sees observed retirement revoke",
        retired.get("accepted").and_then(Value::as_bool) == Some(false)
            && retired.get("job_active").and_then(Value::as_bool) == Some(false),
        &retired,
    );
    let replaced = s.mutate("provision", "w2", "p2", 0)?;
    s.expect_state("replacement generation active", &replaced, "active");
    let old_on_new = s.inspect_secret("job-one", now)?;
    s.check(
        "replacement cannot use predecessor token",
        old_on_new.get("accepted").and_then(Value::as_bool) == Some(false),
        &old_on_new,
    );
    let new = s.issue_secret("job-two", "w2", 1, "p2", now)?;
    s.check(
        "replacement receives its own credentials",
        new.pointer("/ok/fleet/request_id").and_then(Value::as_str) == Some("p2"),
        &new,
    );
    let accepted = s.inspect_secret("job-two", now)?;
    s.check(
        "fake provider accepts replacement token",
        accepted.get("accepted").and_then(Value::as_bool) == Some(true)
            && accepted
                .pointer("/fleet/instance_id")
                .and_then(Value::as_str)
                == Some("w2"),
        &accepted,
    );
    let stale_receipt = s.mutate("reconcile", "w1", "old-receipt", 2)?;
    s.expect_refusal(
        "stale retirement receipt cannot restore old generation",
        &stale_receipt,
        "no uncertain current fleet request",
    );
    let predecessor = s.issue_secret("job-predecessor", "w1", 1, "p1", now)?;
    s.expect_refusal(
        "retired worker cannot mint fresh credential",
        &predecessor,
        "generation not active",
    );
    s.finish("secret-worker-generations", "recovery", &[])
}

fn capacity_boundaries(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "capacity-boundaries", 2)?;
    let first = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("first slot", &first, "active");
    let second = s.mutate("provision", "w2", "p2", 0)?;
    s.expect_state("second slot", &second, "active");
    let third = s.mutate("provision", "w3", "p3", 0)?;
    s.expect_refusal("maximum is enforced", &third, "capacity exhausted");
    let minimum = s.call(
        "configure",
        json!({"minimum": 1, "maximum": 2, "slots_per_instance": 1, "expected_revision": 1}),
    )?;
    s.check(
        "minimum floor configured",
        minimum.get("ok").is_some(),
        &minimum,
    );
    let above = s.mutate("deprovision", "w1", "d1", 1)?;
    s.expect_state("deprovision above the minimum", &above, "retired");
    let floor = s.mutate("deprovision", "w2", "d2", 1)?;
    s.expect_refusal("minimum floor is preserved", &floor, "minimum");
    let refill = s.mutate("provision", "w3", "p3", 0)?;
    s.expect_state("freed slot is reusable", &refill, "active");
    s.finish("capacity-boundaries", "recovery", &[])
}

fn duplicate_create_control(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "duplicate-create-control", 1)?;
    s.fault("provision", "w1", FaultKind::DuplicateCreate)?;
    let provisioned = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state(
        "faulty provider still reports presence",
        &provisioned,
        "active",
    );
    s.finish(
        "duplicate-create-control",
        "control",
        &[Violation::DuplicateCreate],
    )
}

fn premature_release_control(tools: &Tools, out: &Path) -> Result<Value> {
    let mut s = Scenario::new(tools, out, "premature-release-control", 1)?;
    let provisioned = s.mutate("provision", "w1", "p1", 0)?;
    s.expect_state("provision becomes active", &provisioned, "active");
    let db = Connection::open(&s.database).map_err(text)?;
    db.execute(
        "UPDATE hosted_fleet_instances SET state='retired' WHERE org=?1 AND id='w1'",
        [ORGANIZATION],
    )
    .map_err(text)?;
    s.steps
        .push(json!({"control": "slot released while the provider resource is present"}));
    s.finish(
        "premature-release-control",
        "control",
        &[Violation::RetiredWhilePresent],
    )
}

fn status_state(status: &Value, instance: &str) -> Option<String> {
    status
        .get("ok")?
        .get("instances")?
        .as_array()?
        .iter()
        .find(|row| row.get("instance_id").and_then(Value::as_str) == Some(instance))?
        .get("state")?
        .as_str()
        .map(str::to_owned)
}

fn nix_store_add(nix_store: &Path, path: &Path) -> Result<PathBuf> {
    let output = Command::new(nix_store)
        .arg("--add")
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .map_err(text)?;
    if !output.status.success() {
        return Err(format!(
            "nix-store --add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stored = PathBuf::from(String::from_utf8(output.stdout).map_err(text)?.trim());
    if !stored.starts_with("/nix/store") {
        return Err("nix-store returned a path outside /nix/store".into());
    }
    Ok(stored)
}

fn file_blake3(path: &Path) -> Result<String> {
    Ok(blake3::hash(&fs::read(path).map_err(text)?)
        .to_hex()
        .to_string())
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(instance: &str, state: &str) -> InstanceRow {
        InstanceRow {
            instance_id: instance.into(),
            state: state.into(),
        }
    }

    fn entry(seq: u64, operation: &str, instance: &str, effect: &str) -> LogEntry {
        LogEntry {
            seq,
            operation: operation.into(),
            organization: ORGANIZATION.into(),
            instance_id: instance.into(),
            request_id: format!("r{seq}"),
            effect: effect.into(),
        }
    }

    // r[verify kiln.hosted.fleet.lifecycle_qualification]
    #[test]
    fn conserved_lifecycle_reports_no_violation() {
        let log = [
            entry(1, "provision", "w1", "created"),
            entry(2, "inspect", "w1", "inspected"),
            entry(3, "deprovision", "w1", "deleted"),
            entry(4, "provision", "w2", "created"),
        ];
        let report = oracle(
            &[row("w1", "retired"), row("w2", "active")],
            &["w2".into()],
            &log,
            1,
        );
        assert!(report.violations.is_empty());
        assert_eq!((report.created, report.deleted, report.present), (2, 1, 1));
        assert_eq!(report.driver_invocations.get("inspect"), Some(&1));
    }

    // r[verify kiln.hosted.fleet.lifecycle_qualification]
    #[test]
    fn duplicate_creation_is_detected_from_records_or_log() {
        let by_records = oracle(
            &[row("w1", "active")],
            &["w1".into(), "w1".into()],
            &[entry(1, "provision", "w1", "created")],
            2,
        );
        assert!(by_records.violations.contains(&Violation::DuplicateCreate));
        let by_log = oracle(
            &[row("w1", "active")],
            &["w1".into()],
            &[
                entry(1, "provision", "w1", "created"),
                entry(2, "provision", "w1", "created"),
            ],
            2,
        );
        assert!(by_log.violations.contains(&Violation::DuplicateCreate));
        let recreated_after_delete = oracle(
            &[row("w1", "active")],
            &["w1".into()],
            &[
                entry(1, "provision", "w1", "created"),
                entry(2, "deprovision", "w1", "deleted"),
                entry(3, "provision", "w1", "created"),
            ],
            1,
        );
        assert!(recreated_after_delete.violations.is_empty());
    }

    // r[verify kiln.hosted.fleet.lifecycle_qualification]
    #[test]
    fn released_untracked_and_excess_resources_are_violations() {
        let released = oracle(&[row("w1", "retired")], &["w1".into()], &[], 1);
        assert!(
            released
                .violations
                .contains(&Violation::RetiredWhilePresent)
        );
        let unknown_is_not_released = oracle(&[row("w1", "unknown")], &["w1".into()], &[], 1);
        assert!(unknown_is_not_released.violations.is_empty());
        let untracked = oracle(&[], &["w9".into()], &[], 1);
        assert!(untracked.violations.contains(&Violation::UntrackedResource));
        let excess = oracle(
            &[row("w1", "active"), row("w2", "active")],
            &["w1".into(), "w2".into()],
            &[],
            1,
        );
        assert!(excess.violations.contains(&Violation::CapacityExceeded));
    }

    // r[verify kiln.hosted.fleet.lifecycle_qualification]
    #[test]
    fn fault_plans_reject_unknown_fields_and_kinds() {
        let accepted: std::result::Result<Fault, _> = serde_json::from_value(
            json!({"operation": "provision", "instance_id": "w1", "kind": "hang_after_effect"}),
        );
        assert_eq!(
            accepted.map(|fault| fault.kind).ok(),
            Some(FaultKind::HangAfterEffect)
        );
        for rejected in [
            json!({"operation": "provision", "instance_id": "w1", "kind": "explode"}),
            json!({"operation": "provision", "instance_id": "w1", "kind": "before_effect", "extra": 1}),
            json!({"operation": "provision", "kind": "before_effect"}),
        ] {
            assert!(serde_json::from_value::<Fault>(rejected).is_err());
        }
    }
}
