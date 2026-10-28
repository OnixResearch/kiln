use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use kiln_adapter_github::secret::{
    ResolvedSecret, SecretReference, admit_secret_reference, resolve_secret,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroizing;

const MAX_ITEMS: usize = 128;
const MAX_TOKEN_BYTES: usize = 8192;
const MAX_JOB_MS: u64 = 3_600_000;
const MAX_REFERENCE_MS: u64 = 30 * 24 * 3_600_000;
const CAPABILITY_MS: u64 = 60_000;
const DELIVERY_MS: u64 = 60_000;
const MAX_TOKEN_SECONDS: u64 = 300;
const STORAGE: &str = "hosted secret storage unavailable";
const DENIED: &str = "workload authority absent, expired, consumed or revoked";

type Result<T, E = String> = std::result::Result<T, E>;

/// Host-owned policy: never accepted through member RPC.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub issuer: String,
    pub signing_key_path: PathBuf,
    pub audiences: Vec<String>,
    pub token_lifetime_seconds: u64,
    pub resolvers: Vec<Resolver>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolver {
    pub id: String,
    pub organization: String,
    pub project: String,
    pub reference: SecretReference,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SecretGrant {
    pub name: String,
    pub revision: u64,
}

/// Only the trusted executor may construct/admit this from an already admitted job.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JobBinding {
    pub organization: String,
    pub project: String,
    pub job: String,
    pub admission_identity: String,
    pub expires_ms: u64,
    pub audiences: Vec<String>,
    pub secrets: Vec<SecretGrant>,
}

pub struct JobCapability(Zeroizing<String>);
impl JobCapability {
    /// Private executor channel only; do not serialize into customer RPC receipts/logs.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for JobCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JobCapability([REDACTED])")
    }
}
pub struct WorkloadToken(Zeroizing<String>);
impl WorkloadToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for WorkloadToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WorkloadToken([REDACTED])")
    }
}

pub struct DeliveredSecret {
    pub value: ResolvedSecret,
    pub expires_ms: u64,
    pub reference_revision: u64,
}
impl std::fmt::Debug for DeliveredSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeliveredSecret([REDACTED])")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub org: String,
    pub project: String,
    pub job: String,
    pub admission_identity: String,
    pub scope: String,
    pub iat: u64,
    pub nbf: u64,
    pub exp: u64,
    pub jti: String,
}

pub fn init(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_secret_policy(singleton INTEGER PRIMARY KEY CHECK(singleton=1),config TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS hosted_secret_refs(org TEXT NOT NULL,project TEXT NOT NULL,name TEXT NOT NULL,resolver TEXT NOT NULL,revision INTEGER NOT NULL,expires_ms INTEGER NOT NULL,revoked INTEGER NOT NULL,PRIMARY KEY(org,project,name));
    CREATE TABLE IF NOT EXISTS hosted_secret_jobs(org TEXT NOT NULL,project TEXT NOT NULL,job TEXT NOT NULL,binding TEXT NOT NULL,cap_hash TEXT NOT NULL UNIQUE,cap_expires_ms INTEGER NOT NULL,exchanged INTEGER NOT NULL,active INTEGER NOT NULL,PRIMARY KEY(org,project,job));
    CREATE TABLE IF NOT EXISTS hosted_secret_tokens(org TEXT NOT NULL,project TEXT NOT NULL,job TEXT NOT NULL,jti TEXT PRIMARY KEY,claims TEXT NOT NULL,consumed INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS hosted_secret_deliveries(org TEXT NOT NULL,project TEXT NOT NULL,job TEXT NOT NULL,name TEXT NOT NULL,revision INTEGER NOT NULL,expires_ms INTEGER NOT NULL,state TEXT NOT NULL,PRIMARY KEY(org,project,job,name));")
        .map_err(|_| STORAGE.to_owned())
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
}
fn integer(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| "time or revision overflow".to_owned())
}
fn encode(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|_| "secret metadata encoding failed".to_owned())
}
fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| "secret metadata invalid".to_owned())
}
fn digest(value: &str) -> String {
    blake3::hash(value.as_bytes()).to_hex().to_string()
}
fn distinct(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn signing_key(path: &Path) -> Result<SigningKey> {
    if !path.is_absolute()
        || path
            .components()
            .any(|p| matches!(p, std::path::Component::ParentDir))
    {
        return Err("signing key requires an absolute private file path".into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "workload signing key unavailable")?;
    let metadata = file
        .metadata()
        .map_err(|_| "workload signing key unavailable")?;
    let process_uid = std::fs::metadata("/proc/self")
        .map_err(|_| "workload process identity unavailable")?
        .uid();
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != process_uid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.len() != 32
    {
        return Err("workload signing key must be owned mode0600 regular raw32 file".into());
    }
    let mut seed = Zeroizing::new([0u8; 32]);
    file.read_exact(&mut *seed)
        .map_err(|_| "workload signing key unreadable")?;
    Ok(SigningKey::from_bytes(&seed))
}
fn kid(key: &VerifyingKey) -> String {
    blake3::hash(key.as_bytes()).to_hex().to_string()
}

pub fn configure(db: &mut Connection, config: &Config) -> Result<()> {
    if !config.issuer.starts_with("https://")
        || !bounded_text(&config.issuer)
        || config.issuer.ends_with('/')
        || config.issuer.contains(['?', '#', '@'])
        || config.audiences.is_empty()
        || config.audiences.len() > MAX_ITEMS
        || !distinct(&config.audiences)
        || config.audiences.iter().any(|a| !bounded_text(a))
        || !(1..=MAX_TOKEN_SECONDS).contains(&config.token_lifetime_seconds)
        || config.resolvers.len() > MAX_ITEMS
    {
        return Err("invalid workload issuer, audience or lifetime policy".into());
    }
    let mut ids = BTreeSet::new();
    for resolver in &config.resolvers {
        if !crate::identifier(&resolver.id)
            || !crate::identifier(&resolver.organization)
            || !crate::identifier(&resolver.project)
            || !ids.insert(&resolver.id)
        {
            return Err("invalid or duplicate scoped resolver".into());
        }
        admit_secret_reference(&resolver.reference)
            .map_err(|_| "invalid pinned SecretSpec reference")?;
    }
    signing_key(&config.signing_key_path)?;
    // Immutable policy per service database. A deliberate key/policy cutover needs a new
    // separately reviewed database migration, not a member-controlled update.
    let encoded = encode(config)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| STORAGE)?;
    let old: Option<String> = tx
        .query_row(
            "SELECT config FROM hosted_secret_policy WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| STORAGE)?;
    if old.as_ref().is_some_and(|old| old != &encoded) {
        return Err("host secret policy differs from retained policy".into());
    }
    tx.execute(
        "INSERT OR IGNORE INTO hosted_secret_policy(singleton,config) VALUES(1,?1)",
        [&encoded],
    )
    .map_err(|_| STORAGE)?;
    tx.commit().map_err(|_| STORAGE.to_owned())
}
fn policy(db: &Connection) -> Result<Config> {
    let data: String = db
        .query_row(
            "SELECT config FROM hosted_secret_policy WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "host secret policy not configured")?;
    decode(&data)
}

pub fn discovery(db: &Connection) -> Result<Value> {
    let config = policy(db)?;
    Ok(
        json!({"issuer":config.issuer,"jwks_uri":format!("{}/jwks.json",config.issuer),"id_token_signing_alg_values_supported":["EdDSA"],"subject_types_supported":["public"],"response_types_supported":["id_token"],"claims_supported":["iss","aud","sub","org","project","job","admission_identity","scope","iat","nbf","exp","jti"]}),
    )
}
pub fn jwks(db: &Connection) -> Result<Value> {
    let config = policy(db)?;
    let key = signing_key(&config.signing_key_path)?.verifying_key();
    Ok(
        json!({"keys":[{"kty":"OKP","crv":"Ed25519","use":"sig","alg":"EdDSA","kid":kid(&key),"x":URL_SAFE_NO_PAD.encode(key.as_bytes())}]}),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Register {
    project: String,
    name: String,
    resolver: String,
    expected_revision: u64,
    expires_ms: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revoke {
    project: String,
    name: String,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeJob {
    project: String,
    job: String,
    admission_identity: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    project: String,
}

/// Customer metadata/admin surface. Deliberately has no value resolution or mint operation.
pub fn handle(
    db: &mut Connection,
    actor: &crate::Actor,
    action: &str,
    input: Value,
    now_ms: u64,
) -> Result<Value> {
    integer(now_ms)?;
    let current = crate::accounts::authorize(db, &actor.peer, &actor.organization)?;
    let actor = &current;
    match action {
        "discovery" => discovery(db),
        "jwks" => jwks(db),
        "register" => {
            crate::require(actor, &["owner", "admin"])?;
            let request: Register =
                serde_json::from_value(input).map_err(|_| "invalid secret registration")?;
            crate::accounts::require_project(db, actor, &request.project, &["owner", "admin"])?;
            if !crate::identifier(&request.project)
                || !crate::identifier(&request.name)
                || request.expires_ms <= now_ms
                || request.expires_ms
                    > now_ms
                        .checked_add(MAX_REFERENCE_MS)
                        .ok_or("time overflow")?
            {
                return Err("invalid secret scope or lease expiry".into());
            }
            let config = policy(db)?;
            if !config.resolvers.iter().any(|r| {
                r.id == request.resolver
                    && r.organization == actor.organization
                    && r.project == request.project
            }) {
                return Err("resolver not provisioned for organization/project".into());
            }
            let revision = request
                .expected_revision
                .checked_add(1)
                .ok_or("revision overflow")?;
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| STORAGE)?;
            let old: Option<i64> = tx.query_row("SELECT revision FROM hosted_secret_refs WHERE org=?1 AND project=?2 AND name=?3", params![actor.organization,request.project,request.name], |r| r.get(0)).optional().map_err(|_| STORAGE)?;
            if old.unwrap_or(0) != integer(request.expected_revision)? {
                return Err("stale secret revision".into());
            }
            let count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM hosted_secret_refs WHERE org=?1",
                    [&actor.organization],
                    |r| r.get(0),
                )
                .map_err(|_| STORAGE)?;
            if old.is_none() && count >= MAX_ITEMS as i64 {
                return Err("secret reference limit reached".into());
            }
            tx.execute("INSERT INTO hosted_secret_refs(org,project,name,resolver,revision,expires_ms,revoked) VALUES(?1,?2,?3,?4,?5,?6,0) ON CONFLICT(org,project,name) DO UPDATE SET resolver=excluded.resolver,revision=excluded.revision,expires_ms=excluded.expires_ms,revoked=0", params![actor.organization,request.project,request.name,request.resolver,integer(revision)?,integer(request.expires_ms)?]).map_err(|_| STORAGE)?;
            tx.commit().map_err(|_| STORAGE)?;
            Ok(
                json!({"project":request.project,"name":request.name,"revision":revision,"expires_ms":request.expires_ms}),
            )
        }
        "revoke" => {
            crate::require(actor, &["owner", "admin"])?;
            let request: Revoke =
                serde_json::from_value(input).map_err(|_| "invalid secret revocation")?;
            crate::accounts::require_project(db, actor, &request.project, &["owner", "admin"])?;
            let revision = request
                .expected_revision
                .checked_add(1)
                .ok_or("revision overflow")?;
            let changed = db.execute("UPDATE hosted_secret_refs SET revoked=1,revision=?1 WHERE org=?2 AND project=?3 AND name=?4 AND revision=?5 AND revoked=0", params![integer(revision)?,actor.organization,request.project,request.name,integer(request.expected_revision)?]).map_err(|_| STORAGE)?;
            if changed != 1 {
                return Err("secret absent, revoked or revision stale".into());
            }
            Ok(json!({"name":request.name,"revision":revision,"revoked":true}))
        }
        "revoke_job" => {
            crate::require(actor, &["owner", "admin"])?;
            let request: RevokeJob =
                serde_json::from_value(input).map_err(|_| "invalid workload revocation")?;
            crate::accounts::require_project(db, actor, &request.project, &["owner", "admin"])?;
            finish_job(
                db,
                &actor.organization,
                &request.project,
                &request.job,
                &request.admission_identity,
            )?;
            Ok(json!({"job":request.job,"revoked":true}))
        }
        "list" => {
            let request: Project =
                serde_json::from_value(input).map_err(|_| "secret listing requires a project")?;
            crate::accounts::require_project(
                db,
                actor,
                &request.project,
                &["owner", "admin", "member", "viewer"],
            )?;
            let mut statement = db.prepare("SELECT project,name,revision,expires_ms,revoked FROM hosted_secret_refs WHERE org=?1 AND project=?2 ORDER BY name LIMIT 128").map_err(|_| STORAGE)?;
            let rows = statement.query_map(params![actor.organization,request.project], |r| Ok(json!({"project":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"revision":r.get::<_,i64>(2)?,"expires_ms":r.get::<_,i64>(3)?,"revoked":r.get::<_,bool>(4)?}))).map_err(|_| STORAGE)?;
            let values: std::result::Result<Vec<_>, _> = rows.collect();
            Ok(json!({"references":values.map_err(|_| STORAGE)?}))
        }
        _ => Err(
            "unsupported secrets action; job credentials require private executor binding".into(),
        ),
    }
}

fn current_job(db: &Connection, cap: &str, now_ms: u64) -> Result<JobBinding> {
    if cap.len() > MAX_TOKEN_BYTES || cap.is_empty() {
        return Err(DENIED.into());
    }
    integer(now_ms)?;
    let row: Option<(String, bool)> = db
        .query_row(
            "SELECT binding,active FROM hosted_secret_jobs WHERE cap_hash=?1",
            [digest(cap)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| STORAGE)?;
    let (data, active) = row.ok_or(DENIED)?;
    let binding: JobBinding = decode(&data)?;
    if !active || binding.expires_ms <= now_ms {
        return Err(DENIED.into());
    }
    Ok(binding)
}

/// Call immediately before actual admitted dispatch, not from any customer RPC.
/// Job IDs are one-use permanently, including after revoke/restart.
pub fn begin_job(db: &mut Connection, binding: &JobBinding, now_ms: u64) -> Result<JobCapability> {
    let config = policy(db)?;
    integer(now_ms)?;
    if [
        &binding.organization,
        &binding.project,
        &binding.job,
        &binding.admission_identity,
    ]
    .iter()
    .any(|v| !crate::identifier(v))
        || binding.expires_ms <= now_ms
        || binding.expires_ms > now_ms.checked_add(MAX_JOB_MS).ok_or("time overflow")?
        || binding.audiences.len() > MAX_ITEMS
        || !distinct(&binding.audiences)
        || binding
            .audiences
            .iter()
            .any(|a| !config.audiences.contains(a))
        || binding.secrets.len() > MAX_ITEMS
        || binding
            .secrets
            .iter()
            .any(|s| !crate::identifier(&s.name) || s.revision == 0)
        || binding
            .secrets
            .iter()
            .map(|s| &s.name)
            .collect::<BTreeSet<_>>()
            .len()
            != binding.secrets.len()
    {
        return Err("invalid admitted workload binding".into());
    }
    let key = signing_key(&config.signing_key_path)?;
    let encoded = encode(binding)?;
    let cap = URL_SAFE_NO_PAD.encode(
        key.sign(format!("kiln.hosted.job-capability.v1:{encoded}").as_bytes())
            .to_bytes(),
    );
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| STORAGE)?;
    for grant in &binding.secrets {
        reference(&tx, binding, grant, now_ms)?;
    }
    tx.execute("INSERT INTO hosted_secret_jobs(org,project,job,binding,cap_hash,cap_expires_ms,exchanged,active) VALUES(?1,?2,?3,?4,?5,?6,0,1)", params![binding.organization,binding.project,binding.job,encoded,digest(&cap),integer(binding.expires_ms.min(now_ms.checked_add(CAPABILITY_MS).ok_or("time overflow")?))?]).map_err(|_| "job identity already admitted or storage unavailable")?;
    tx.commit().map_err(|_| STORAGE)?;
    Ok(JobCapability(Zeroizing::new(cap)))
}

pub fn finish_job(
    db: &Connection,
    organization: &str,
    project: &str,
    job: &str,
    admission_identity: &str,
) -> Result<()> {
    let encoded: String = db
        .query_row(
            "SELECT binding FROM hosted_secret_jobs WHERE org=?1 AND project=?2 AND job=?3",
            params![organization, project, job],
            |r| r.get(0),
        )
        .map_err(|_| DENIED)?;
    let binding: JobBinding = decode(&encoded)?;
    if binding.admission_identity != admission_identity {
        return Err(DENIED.into());
    }
    db.execute(
        "UPDATE hosted_secret_jobs SET active=0 WHERE org=?1 AND project=?2 AND job=?3",
        params![organization, project, job],
    )
    .map_err(|_| STORAGE)?;
    Ok(())
}

fn reference(
    db: &Connection,
    binding: &JobBinding,
    grant: &SecretGrant,
    now_ms: u64,
) -> Result<(String, u64)> {
    let row: Option<(String,i64,i64,bool)> = db.query_row("SELECT resolver,revision,expires_ms,revoked FROM hosted_secret_refs WHERE org=?1 AND project=?2 AND name=?3", params![binding.organization,binding.project,grant.name], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_| STORAGE)?;
    let (resolver, revision, expires, revoked) = row.ok_or(DENIED)?;
    if revoked || revision != integer(grant.revision)? || expires <= integer(now_ms)? {
        return Err(DENIED.into());
    }
    Ok((resolver, u64::try_from(expires).map_err(|_| DENIED)?))
}

/// One private delivery per admitted job/name. Reserve before resolver process;
/// failed/uncertain delivery is not retried, and no value is persisted.
pub fn resolve_for_job(
    db: &mut Connection,
    capability: &str,
    name: &str,
    now_ms: u64,
) -> Result<DeliveredSecret> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| STORAGE)?;
    let binding = current_job(&tx, capability, now_ms)?;
    let grant = binding
        .secrets
        .iter()
        .find(|g| g.name == name)
        .ok_or(DENIED)?;
    let (resolver_id, reference_expiry) = reference(&tx, &binding, grant, now_ms)?;
    let expires_ms = binding
        .expires_ms
        .min(reference_expiry)
        .min(now_ms.checked_add(DELIVERY_MS).ok_or("time overflow")?);
    let config = policy(&tx)?;
    let resolver = config
        .resolvers
        .iter()
        .find(|r| {
            r.id == resolver_id
                && r.organization == binding.organization
                && r.project == binding.project
        })
        .ok_or(DENIED)?;
    tx.execute("INSERT INTO hosted_secret_deliveries(org,project,job,name,revision,expires_ms,state) VALUES(?1,?2,?3,?4,?5,?6,'reserved')", params![binding.organization,binding.project,binding.job,name,integer(grant.revision)?,integer(expires_ms)?]).map_err(|_| "secret delivery already reserved or unavailable")?;
    tx.commit().map_err(|_| STORAGE)?;
    let started = std::time::Instant::now();
    let value = resolve_secret(
        &resolver.reference,
        "admitted hosted CI job secret delivery",
    )
    .map_err(|_| "pinned secret resolution failed")?;
    let elapsed = u64::try_from(started.elapsed().as_millis()).map_err(|_| "time overflow")?;
    let after = now_ms.checked_add(elapsed).ok_or("time overflow")?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| STORAGE)?;
    current_job(&tx, capability, after)?;
    reference(&tx, &binding, grant, after)?;
    if after >= expires_ms {
        return Err(DENIED.into());
    }
    tx.execute("UPDATE hosted_secret_deliveries SET state='delivered' WHERE org=?1 AND project=?2 AND job=?3 AND name=?4 AND state='reserved'", params![binding.organization,binding.project,binding.job,name]).map_err(|_| STORAGE)?;
    tx.commit().map_err(|_| STORAGE)?;
    Ok(DeliveredSecret {
        value,
        expires_ms,
        reference_revision: grant.revision,
    })
}

/// Consume the executor-issued capability once, durably before returning a JWT.
pub fn exchange(
    db: &mut Connection,
    capability: &str,
    audience: &str,
    now_ms: u64,
) -> Result<WorkloadToken> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| STORAGE)?;
    let binding = current_job(&tx, capability, now_ms)?;
    let config = policy(&tx)?;
    if !binding.audiences.iter().any(|a| a == audience)
        || !config.audiences.iter().any(|a| a == audience)
    {
        return Err("workload audience not approved".into());
    }
    let key = signing_key(&config.signing_key_path)?;
    let iat = now_ms / 1000;
    let exp = iat
        .checked_add(config.token_lifetime_seconds)
        .ok_or("time overflow")?
        .min(binding.expires_ms / 1000);
    if exp <= iat {
        return Err(DENIED.into());
    }
    let claims = Claims {
        iss: config.issuer,
        aud: audience.to_owned(),
        sub: format!(
            "org:{}:project:{}:job:{}",
            binding.organization, binding.project, binding.job
        ),
        org: binding.organization.clone(),
        project: binding.project.clone(),
        job: binding.job.clone(),
        admission_identity: binding.admission_identity,
        scope: "workload".into(),
        iat,
        nbf: iat,
        exp,
        jti: crate::identity(&("kiln.hosted.oidc.v1", digest(capability), audience))?,
    };
    let header = URL_SAFE_NO_PAD.encode(encode(
        &json!({"alg":"EdDSA","typ":"JWT","kid":kid(&key.verifying_key())}),
    )?);
    let payload = URL_SAFE_NO_PAD.encode(encode(&claims)?);
    let signing_input = format!("{header}.{payload}");
    let token = format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(key.sign(signing_input.as_bytes()).to_bytes())
    );
    let changed = tx.execute("UPDATE hosted_secret_jobs SET exchanged=1 WHERE org=?1 AND project=?2 AND job=?3 AND exchanged=0 AND cap_expires_ms>?4 AND active=1", params![binding.organization,binding.project,binding.job,integer(now_ms)?]).map_err(|_| STORAGE)?;
    if changed != 1 {
        return Err(DENIED.into());
    }
    tx.execute("INSERT INTO hosted_secret_tokens(org,project,job,jti,claims,consumed) VALUES(?1,?2,?3,?4,?5,0)",params![claims.org,claims.project,claims.job,claims.jti,encode(&claims)?]).map_err(|_| STORAGE)?;
    tx.commit().map_err(|_| STORAGE)?;
    Ok(WorkloadToken(Zeroizing::new(token)))
}

fn verify(db: &Connection, token: &str, audience: &str, now_ms: u64) -> Result<Claims> {
    if token.len() > MAX_TOKEN_BYTES {
        return Err(DENIED.into());
    }
    let (signing_input, signature) = token.rsplit_once('.').ok_or(DENIED)?;
    let (header, payload) = signing_input.split_once('.').ok_or(DENIED)?;
    let config = policy(db)?;
    let key = signing_key(&config.signing_key_path)?.verifying_key();
    let header: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(header).map_err(|_| DENIED)?)
            .map_err(|_| DENIED)?;
    if header != json!({"alg":"EdDSA","typ":"JWT","kid":kid(&key)}) {
        return Err(DENIED.into());
    }
    let signature = Signature::from_slice(&URL_SAFE_NO_PAD.decode(signature).map_err(|_| DENIED)?)
        .map_err(|_| DENIED)?;
    key.verify_strict(signing_input.as_bytes(), &signature)
        .map_err(|_| DENIED)?;
    let claims: Claims =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).map_err(|_| DENIED)?)
            .map_err(|_| DENIED)?;
    let now = now_ms / 1000;
    if claims.iss != config.issuer
        || claims.aud != audience
        || !config.audiences.iter().any(|a| a == audience)
        || claims.nbf > now
        || claims.iat > now
        || claims.exp <= now
        || claims.exp <= claims.iat
        || claims.exp - claims.iat > config.token_lifetime_seconds
    {
        return Err(DENIED.into());
    }
    let row: Option<(String,bool,String,bool)> = db.query_row("SELECT t.claims,t.consumed,j.binding,j.active FROM hosted_secret_tokens t JOIN hosted_secret_jobs j ON t.org=j.org AND t.project=j.project AND t.job=j.job WHERE t.org=?1 AND t.project=?2 AND t.job=?3 AND t.jti=?4", params![claims.org,claims.project,claims.job,claims.jti], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_| STORAGE)?;
    let (saved, consumed, binding, active) = row.ok_or(DENIED)?;
    let binding: JobBinding = decode(&binding)?;
    if !active
        || consumed
        || binding.expires_ms <= now_ms
        || binding.admission_identity != claims.admission_identity
        || decode::<Claims>(&saved)? != claims
    {
        return Err(DENIED.into());
    }
    Ok(claims)
}

/// Read-only validation for a resource server; use consume_token for a replay-safe exchange.
pub fn verify_token(db: &Connection, token: &str, audience: &str, now_ms: u64) -> Result<Claims> {
    verify(db, token, audience, now_ms)
}
pub fn consume_token(
    db: &mut Connection,
    token: &str,
    audience: &str,
    now_ms: u64,
) -> Result<Claims> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| STORAGE)?;
    let claims = verify(&tx, token, audience, now_ms)?;
    let changed = tx.execute("UPDATE hosted_secret_tokens SET consumed=1 WHERE org=?1 AND project=?2 AND job=?3 AND jti=?4 AND consumed=0", params![claims.org,claims.project,claims.job,claims.jti]).map_err(|_| STORAGE)?;
    if changed != 1 {
        return Err(DENIED.into());
    }
    tx.commit().map_err(|_| STORAGE)?;
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const NOW: u64 = 1_000_000;

    struct Fixture {
        _root: tempfile::TempDir,
        db: Connection,
        config: Config,
        owner: crate::Actor,
    }
    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir()
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            let key = root.path().join("issuer.seed");
            std::fs::write(&key, [17u8; 32])
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600))
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            // Real local process speaking the existing SecretSpec CLI contract.
            let program = root.path().join("secretspec");
            let script = b"#!/bin/sh\n[ \"$1\" = get ] && [ \"$2\" = CUSTOMER_TOKEN ] || exit 9\nprintf 'customer-secret-fixture\\n'\n";
            std::fs::write(&program, script)
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700))
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            let config = Config {
                issuer: "https://issuer.example.test".into(),
                signing_key_path: key,
                audiences: vec!["https://resource.example.test".into()],
                token_lifetime_seconds: 120,
                resolvers: vec![Resolver {
                    id: "customer-provider".into(),
                    organization: "org".into(),
                    project: "project".into(),
                    reference: SecretReference {
                        program_path: program,
                        program_blake3: blake3::hash(script).to_hex().to_string(),
                        project_root: root.path().to_path_buf(),
                        profile: "customer".into(),
                        provider: None,
                        secret_key: "CUSTOMER_TOKEN".into(),
                    },
                }],
            };
            let mut db = Connection::open_in_memory()
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            crate::accounts::init(&db)
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            crate::accounts::bootstrap(&mut db, &"a".repeat(64), "org")
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            init(&db).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            configure(&mut db, &config)
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            let owner = crate::accounts::authorize(&db, &"a".repeat(64), "org")
                .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            handle(&mut db, &owner, "register", json!({"project":"project","name":"TOKEN","resolver":"customer-provider","expected_revision":0,"expires_ms":NOW+600_000}), NOW).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
            Self {
                _root: root,
                db,
                config,
                owner,
            }
        }
        fn binding(&self, job: &str) -> JobBinding {
            JobBinding {
                organization: "org".into(),
                project: "project".into(),
                job: job.into(),
                admission_identity: format!("admission-{job}"),
                expires_ms: NOW + 600_000,
                audiences: self.config.audiences.clone(),
                secrets: vec![SecretGrant {
                    name: "TOKEN".into(),
                    revision: 1,
                }],
            }
        }
    }

    #[test]
    fn jwt_is_interoperable_scoped_expiring_and_consume_once() {
        let mut f = Fixture::new();
        let binding = f.binding("job-identity");
        let cap = begin_job(&mut f.db, &binding, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(exchange(&mut f.db, cap.as_str(), "unapproved", NOW).is_err());
        let audience = f
            .config
            .audiences
            .first()
            .cloned()
            .unwrap_or_else(|| unreachable!("fixture has an audience"));
        let token = exchange(&mut f.db, cap.as_str(), &audience, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let claims = verify_token(&f.db, token.as_str(), &audience, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(claims.org, "org");
        assert_eq!(claims.project, "project");
        assert_eq!(claims.job, "job-identity");
        assert_eq!(claims.exp, NOW / 1000 + 120);
        // Independently consume the published JWKS and verify the actual JWS bytes.
        let keys = jwks(&f.db)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let bytes = URL_SAFE_NO_PAD
            .decode(
                keys.pointer("/keys/0/x")
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| unreachable!("fixture JWKS x is a string")),
            )
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let public =
            VerifyingKey::from_bytes(bytes.as_slice().try_into().unwrap_or_else(|error| {
                unreachable!("fixture public key has 32 bytes: {error:?}")
            }))
            .unwrap_or_else(|error| unreachable!("fixture public key is valid: {error:?}"));
        let (signed, signature_segment) = token
            .as_str()
            .rsplit_once('.')
            .unwrap_or_else(|| unreachable!("fixture token has signature segment"));
        let (header_segment, _) = signed
            .split_once('.')
            .unwrap_or_else(|| unreachable!("fixture token has header and payload"));
        let signature =
            Signature::from_slice(&URL_SAFE_NO_PAD.decode(signature_segment).unwrap_or_else(
                |error| unreachable!("fixture signature base64 is valid: {error:?}"),
            ))
            .unwrap_or_else(|error| unreachable!("fixture signature has 64 bytes: {error:?}"));
        public
            .verify_strict(signed.as_bytes(), &signature)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(exchange(&mut f.db, cap.as_str(), &audience, NOW).is_err());
        assert!(verify_token(&f.db, token.as_str(), "wrong-audience", NOW).is_err());
        assert!(verify_token(&f.db, token.as_str(), &audience, NOW + 120_000).is_err());
        assert!(verify_token(&f.db, token.as_str(), &audience, NOW - 1_000).is_err());
        let tampered = format!(
            "{}.{}.{}",
            header_segment,
            URL_SAFE_NO_PAD.encode(b"{}"),
            signature_segment
        );
        assert!(verify_token(&f.db, &tampered, &audience, NOW).is_err());
        assert_eq!(
            consume_token(&mut f.db, token.as_str(), &audience, NOW)
                .unwrap_or_else(|error| unreachable!("fixture token consumes once: {error:?}")),
            claims
        );
        assert!(consume_token(&mut f.db, token.as_str(), &audience, NOW).is_err());
        assert!(verify_token(&f.db, token.as_str(), &audience, NOW).is_err());
    }

    #[test]
    fn job_completion_revokes_tokens_capabilities_and_reentry() {
        let mut f = Fixture::new();
        let binding = f.binding("job-revoke");
        let cap = begin_job(&mut f.db, &binding, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let audience = f
            .config
            .audiences
            .first()
            .cloned()
            .unwrap_or_else(|| unreachable!("fixture has an audience"));
        let token = exchange(&mut f.db, cap.as_str(), &audience, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(
            finish_job(
                &f.db,
                "other-org",
                "project",
                &binding.job,
                &binding.admission_identity
            )
            .is_err()
        );
        assert!(finish_job(&f.db, "org", "project", &binding.job, "wrong-admission").is_err());
        verify_token(&f.db, token.as_str(), &audience, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        finish_job(
            &f.db,
            "org",
            "project",
            &binding.job,
            &binding.admission_identity,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(verify_token(&f.db, token.as_str(), &audience, NOW).is_err());
        assert!(resolve_for_job(&mut f.db, cap.as_str(), "TOKEN", NOW).is_err());
        assert!(begin_job(&mut f.db, &binding, NOW).is_err());
        let expired = f.binding("job-cap-expired");
        let cap = begin_job(&mut f.db, &expired, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(exchange(&mut f.db, cap.as_str(), &audience, NOW + CAPABILITY_MS).is_err());
    }

    #[test]
    fn real_pinned_secret_delivery_obeys_scope_revision_and_single_delivery() {
        let mut f = Fixture::new();
        let binding = f.binding("job-secret");
        let cap = begin_job(&mut f.db, &binding, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(resolve_for_job(&mut f.db, "forged-capability", "TOKEN", NOW).is_err());
        assert!(resolve_for_job(&mut f.db, cap.as_str(), "OTHER", NOW).is_err());
        let delivered = resolve_for_job(&mut f.db, cap.as_str(), "TOKEN", NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(
            delivered
                .value
                .as_str()
                .unwrap_or_else(|error| unreachable!("fixture secret is UTF-8: {error:?}")),
            "customer-secret-fixture"
        );
        assert_eq!(delivered.reference_revision, 1);
        assert_eq!(delivered.expires_ms, NOW + DELIVERY_MS);
        assert!(!format!("{delivered:?}").contains("customer-secret-fixture"));
        assert!(!format!("{cap:?}").contains(cap.as_str()));
        assert!(resolve_for_job(&mut f.db, cap.as_str(), "TOKEN", NOW).is_err());
        let mut wrong_project = f.binding("job-cross-project");
        wrong_project.project = "other-project".into();
        assert!(begin_job(&mut f.db, &wrong_project, NOW).is_err());
        let mut wrong_org = f.binding("job-cross-org");
        wrong_org.organization = "other-org".into();
        assert!(begin_job(&mut f.db, &wrong_org, NOW).is_err());
        let pending = f.binding("job-revoked-ref");
        let cap = begin_job(&mut f.db, &pending, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        handle(
            &mut f.db,
            &f.owner,
            "revoke",
            json!({"project":"project","name":"TOKEN","expected_revision":1}),
            NOW,
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(resolve_for_job(&mut f.db, cap.as_str(), "TOKEN", NOW).is_err());
        let stale = f.binding("job-stale");
        assert!(begin_job(&mut f.db, &stale, NOW).is_err());
    }

    #[test]
    fn reference_rotation_and_expiry_deny_old_and_expired_grants() {
        let mut f = Fixture::new();
        let old_binding = f.binding("job-old-revision");
        let old_cap = begin_job(&mut f.db, &old_binding, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(handle(&mut f.db, &f.owner, "register", json!({"project":"project","name":"TOKEN","resolver":"customer-provider","expected_revision":0,"expires_ms":NOW+1000}), NOW).is_err());
        handle(&mut f.db, &f.owner, "register", json!({"project":"project","name":"TOKEN","resolver":"customer-provider","expected_revision":1,"expires_ms":NOW+1000}), NOW).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(resolve_for_job(&mut f.db, old_cap.as_str(), "TOKEN", NOW).is_err());
        let mut current = f.binding("job-new-revision");
        current
            .secrets
            .first_mut()
            .unwrap_or_else(|| unreachable!("fixture has a secret grant"))
            .revision = 2;
        let cap = begin_job(&mut f.db, &current, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(resolve_for_job(&mut f.db, cap.as_str(), "TOKEN", NOW + 1000).is_err());
        current.job = "job-already-expired-ref".into();
        assert!(begin_job(&mut f.db, &current, NOW + 1000).is_err());
    }

    #[test]
    fn resolver_drift_and_private_key_fail_closed() {
        let mut f = Fixture::new();
        let binding = f.binding("job-drift");
        let cap = begin_job(&mut f.db, &binding, NOW)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let resolver = f
            .config
            .resolvers
            .first()
            .unwrap_or_else(|| unreachable!("fixture has a resolver"));
        std::fs::write(
            &resolver.reference.program_path,
            b"#!/bin/sh\nprintf wrong\n",
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(resolve_for_job(&mut f.db, cap.as_str(), "TOKEN", NOW).is_err());
        let state: String = f.db.query_row("SELECT state FROM hosted_secret_deliveries WHERE org='org' AND project='project' AND job='job-drift' AND name='TOKEN'",[],|r|r.get(0)).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert_eq!(state, "reserved");
        std::fs::set_permissions(
            &f.config.signing_key_path,
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(jwks(&f.db).is_err());
        std::fs::set_permissions(
            &f.config.signing_key_path,
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let alias = f._root.path().join("key-link");
        std::os::unix::fs::symlink(&f.config.signing_key_path, &alias)
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(signing_key(&alias).is_err());
    }

    #[test]
    fn customer_rpc_has_no_mint_or_secret_access_and_current_project_scopes_hold() {
        let mut f = Fixture::new();
        let peer = "b".repeat(64);
        f.db.execute(
            "INSERT INTO hosted_memberships VALUES('org',?1,'member','[\"project\"]',1,1,?2)",
            params![peer, f.owner.peer],
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let member = crate::accounts::authorize(&f.db, &peer, "org")
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        let listed = handle(
            &mut f.db,
            &member,
            "list",
            json!({"project":"project"}),
            NOW,
        )
        .unwrap_or_else(|error| unreachable!("fixture member can list granted project: {error:?}"));
        assert_eq!(
            listed.pointer("/references/0/name").and_then(Value::as_str),
            Some("TOKEN")
        );
        assert!(
            handle(
                &mut f.db,
                &member,
                "list",
                json!({"project":"other-project"}),
                NOW
            )
            .is_err()
        );
        assert!(handle(&mut f.db, &member, "list", json!({}), NOW).is_err());
        assert!(
            handle(
                &mut f.db,
                &member,
                "mint",
                json!({"project":"project","job":"invented"}),
                NOW
            )
            .is_err()
        );
        assert!(
            handle(
                &mut f.db,
                &member,
                "resolve",
                json!({"project":"project","name":"TOKEN"}),
                NOW
            )
            .is_err()
        );
        assert!(handle(&mut f.db, &member, "register", json!({"project":"project","name":"NEW","resolver":"customer-provider","expected_revision":0,"expires_ms":NOW+1000}), NOW).is_err());
        f.db.execute(
            "UPDATE hosted_memberships SET active=0 WHERE organization='org' AND peer=?1",
            [peer],
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(
            handle(
                &mut f.db,
                &member,
                "list",
                json!({"project":"project"}),
                NOW
            )
            .is_err()
        );
    }
}
