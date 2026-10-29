use crate::{Request, Response, SCHEMA};
use fs2::FileExt;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, Ordering},
};

const MAX_REQUEST_RECORDS: i64 = 100_000;
const SQLITE_BUSY_MS: u64 = 5_000;
type CancellationRegistry = Arc<Mutex<BTreeMap<(String, String), Arc<AtomicBool>>>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceProfile {
    pub path: PathBuf,
    pub blake3: String,
    #[serde(default)]
    pub network_activation: bool,
    pub organization: String,
    pub project: String,
    pub price_id: Option<String>,
    pub class: Option<String>,
    pub max_units: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub service_id: String,
    pub state_directory: PathBuf,
    pub network: crate::transport::NetworkProfile,
    pub bootstrap_peer: String,
    pub bootstrap_organization: String,
    #[serde(default)]
    pub source_profiles: Vec<SourceProfile>,
    #[serde(default)]
    pub operator_peers: Vec<String>,
    #[serde(default)]
    pub operations: Option<crate::operations::OperationsProfile>,
    #[serde(default)]
    pub secrets: Option<crate::secrets::Config>,
    #[serde(default)]
    pub execution: Option<crate::execution::Config>,
    #[serde(default)]
    pub fleet: Option<crate::fleet::Config>,
}
impl Profile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::transport::private_bytes(path, crate::MAX_FRAME_BYTES)?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "hosted profile malformed")?;
        profile.admit()?;
        Ok(profile)
    }
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != SCHEMA
            || !crate::identifier(&self.service_id)
            || !crate::identifier(&self.bootstrap_organization)
            || !self.state_directory.is_absolute()
            || self.source_profiles.len() > 128
            || self.bootstrap_peer.parse::<iroh::PublicKey>().is_err()
        {
            return Err("hosted profile rejected".into());
        }
        if self.operator_peers.len() > 16
            || self
                .operator_peers
                .iter()
                .any(|peer| peer.parse::<iroh::PublicKey>().is_err())
        {
            return Err("operator peer policy rejected".into());
        }
        self.network.admit()
    }
}

pub struct Service {
    pub db: Connection,
    pub profile: Arc<Profile>,
    _lock: Arc<std::fs::File>,
    cancellations: CancellationRegistry,
    native_effects: Arc<RwLock<()>>,
}
impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service")
            .field("service_id", &self.profile.service_id)
            .finish_non_exhaustive()
    }
}
impl Service {
    pub fn open(profile: Profile) -> Result<Self, String> {
        profile.admit()?;
        match std::fs::symlink_metadata(&profile.state_directory) {
            Ok(meta) => {
                let owner = std::fs::metadata("/proc/self")
                    .map_err(|_| "owner unavailable")?
                    .uid();
                if !meta.is_dir()
                    || meta.file_type().is_symlink()
                    || meta.uid() != owner
                    || meta.mode() & 0o777 != 0o700
                {
                    return Err("state root must be owned private directory".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&profile.state_directory)
                    .map_err(|_| "state root unavailable")?;
                std::fs::set_permissions(
                    &profile.state_directory,
                    std::fs::Permissions::from_mode(0o700),
                )
                .map_err(|_| "state permissions failed")?;
            }
            Err(_) => return Err("state root unavailable".into()),
        }
        for ancestor in profile.state_directory.ancestors() {
            let meta =
                std::fs::symlink_metadata(ancestor).map_err(|_| "state ancestor unavailable")?;
            if meta.file_type().is_symlink()
                || (meta.mode() & 0o022 != 0 && meta.mode() & 0o1000 == 0)
            {
                return Err("unsafe state ancestor".into());
            }
        }
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(profile.state_directory.join("service.lock"))
            .map_err(|_| "state lock unavailable")?;
        lock.try_lock_exclusive()
            .map_err(|_| "hosted service already active")?;
        let path = profile.state_directory.join("hosted.sqlite");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| "database unavailable")?;
        if !file
            .metadata()
            .map_err(|_| "database metadata unavailable")?
            .is_file()
        {
            return Err("database must be regular file".into());
        }
        let mut db = Connection::open(&path).map_err(|_| "database open failed")?;
        db.busy_timeout(std::time::Duration::from_millis(SQLITE_BUSY_MS))
            .map_err(|_| "database timeout failed")?;
        db.execute_batch("PRAGMA foreign_keys=ON;PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;CREATE TABLE IF NOT EXISTS hosted_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema TEXT NOT NULL,service_id TEXT NOT NULL);CREATE TABLE IF NOT EXISTS hosted_requests(peer TEXT NOT NULL,organization TEXT NOT NULL,request_id TEXT NOT NULL,fingerprint TEXT NOT NULL,response BLOB,PRIMARY KEY(peer,organization,request_id));CREATE TABLE IF NOT EXISTS hosted_audit(ordinal INTEGER PRIMARY KEY,peer TEXT NOT NULL,organization TEXT NOT NULL,action TEXT NOT NULL,request_id TEXT NOT NULL,recorded_at_ms INTEGER NOT NULL,success INTEGER NOT NULL);").map_err(|_|"database schema failed")?;
        let prior: Option<(String, String)> = db
            .query_row(
                "SELECT schema,service_id FROM hosted_meta WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| "service identity read failed")?;
        match prior {
            Some((schema, id)) if schema == SCHEMA && id == profile.service_id => {}
            Some(_) => return Err("state belongs to another service".into()),
            None => {
                db.execute(
                    "INSERT INTO hosted_meta VALUES(1,?1,?2)",
                    params![SCHEMA, profile.service_id],
                )
                .map_err(|_| "service identity save failed")?;
            }
        }
        accounts_init(&db)?;
        crate::accounts::bootstrap(
            &mut db,
            &profile.bootstrap_peer,
            &profile.bootstrap_organization,
        )?;
        crate::connections::begin_profile_configuration(&db)?;
        for source in &profile.source_profiles {
            crate::connections::admit_profile(&mut db, &source.path, &source.blake3)?;
            crate::connections::admit_profile_scope(
                &db,
                &source.blake3,
                &source.organization,
                &source.project,
            )?;
        }
        if let Some(operations) = &profile.operations {
            crate::operations::configure(&db, operations)?;
        }
        if let Some(secrets) = &profile.secrets {
            crate::secrets::configure(&mut db, secrets)?;
        }
        if let Some(execution) = &profile.execution {
            crate::execution::configure(&mut db, execution)?;
        }
        if let Some(fleet) = &profile.fleet {
            crate::fleet::configure_trusted(&mut db, fleet)?;
        }
        Ok(Self {
            db,
            profile: Arc::new(profile),
            _lock: Arc::new(lock),
            cancellations: Arc::new(Mutex::new(BTreeMap::new())),
            native_effects: Arc::new(RwLock::new(())),
        })
    }
    /// A request owns its SQLite connection; long jobs never hold the transport mutex.
    pub fn fork(&self) -> Result<Self, String> {
        let db = Connection::open(self.profile.state_directory.join("hosted.sqlite"))
            .map_err(|_| "request database unavailable")?;
        db.busy_timeout(std::time::Duration::from_millis(SQLITE_BUSY_MS))
            .map_err(|_| "request database timeout failed")?;
        db.execute_batch("PRAGMA foreign_keys=ON;PRAGMA synchronous=FULL;")
            .map_err(|_| "request database policy failed")?;
        Ok(Self {
            db,
            profile: Arc::clone(&self.profile),
            _lock: Arc::clone(&self._lock),
            cancellations: Arc::clone(&self.cancellations),
            native_effects: Arc::clone(&self.native_effects),
        })
    }
    fn tick_connection(
        &mut self,
        actor: &crate::Actor,
        input: &Value,
        now: u64,
    ) -> Result<Value, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Tick {
            project: String,
            connection: String,
            generation: i64,
        }
        let input: Tick =
            serde_json::from_value(input.clone()).map_err(|_| "invalid connection tick")?;
        let digest = crate::connections::connection_profile_identity(
            &self.db,
            actor,
            &input.project,
            &input.connection,
            input.generation,
        )?;
        let profile = self
            .profile
            .source_profiles
            .iter()
            .find(|p| {
                p.blake3 == digest
                    && p.organization == actor.organization
                    && p.project == input.project
                    && p.network_activation
            })
            .cloned()
            .ok_or("operator has not enabled this exact source connection's network effects")?;
        let held = crate::operations::is_held(&self.db, &actor.organization)?
            || !crate::connections::dispatch_allowed(
                &self.db,
                actor,
                &input.project,
                &input.connection,
                input.generation,
            )?;
        let mut approved = Vec::new();
        if !held {
            for pending in crate::connections::pending_dispatches(
                &self.db,
                actor,
                &input.project,
                &input.connection,
                input.generation,
            )? {
                let price = profile
                    .price_id
                    .as_deref()
                    .ok_or("operator source dispatch price unavailable")?;
                let class = profile
                    .class
                    .as_deref()
                    .ok_or("operator source dispatch class unavailable")?;
                let units = profile
                    .max_units
                    .ok_or("operator source dispatch exposure unavailable")?;
                let binding = json!({"project":input.project,"connection":input.connection,"generation":input.generation,"profile":digest,"run":pending.run,"plan":crate::identity(&pending.plan)?});
                crate::finance::reserve(
                    &mut self.db,
                    actor,
                    json!({"attempt_id":pending.attempt,"price_id":price,"class":class,"max_units":units,"binding":binding}),
                    now,
                )?;
                let evidence =
                    crate::identity(&("source-dispatch-reservation", &pending.attempt, &binding))?;
                crate::finance::observe_unknown_trusted(
                    &mut self.db,
                    &actor.organization,
                    &pending.attempt,
                    &evidence,
                    "Source dispatch reserved; native status is not trusted usage metering",
                    now,
                )?;
                approved.push(pending.attempt);
            }
        }
        let result = crate::connections::tick(
            &mut self.db,
            actor,
            &input.project,
            &input.connection,
            input.generation,
            &approved,
            !held,
        )?;
        Ok(
            json!({"source":result,"financial_observation":"Source reservations remain unavailable until independently trusted exact-attempt metering is supplied"}),
        )
    }
    pub fn handle(&mut self, peer: &str, request: Request) -> Response {
        let request_id = request.request_id.clone();
        match self.dispatch(peer, &request) {
            Ok(result) => Response {
                schema: SCHEMA.into(),
                request_id,
                result: Some(result),
                error: None,
            },
            Err(error) => Response {
                schema: SCHEMA.into(),
                request_id,
                result: None,
                error: Some(error),
            },
        }
    }
    fn dispatch(&mut self, peer: &str, request: &Request) -> Result<Value, String> {
        if request.action != "accounts.accept" {
            return self.dispatch_request(peer, request);
        }
        // Enrollment is not membership-authorized. Commit its invitation, membership,
        // replay receipt and audit together; rejected peers retain no global quota.
        self.db
            .execute_batch("SAVEPOINT hosted_invitation_request")
            .map_err(|_| "invitation request transaction unavailable")?;
        let result = self.dispatch_request(peer, request);
        let finish = if result.is_ok() {
            "RELEASE hosted_invitation_request"
        } else {
            "ROLLBACK TO hosted_invitation_request; RELEASE hosted_invitation_request"
        };
        self.db
            .execute_batch(finish)
            .map_err(|_| "invitation request durability unknown")?;
        result
    }
    fn dispatch_request(&mut self, peer: &str, request: &Request) -> Result<Value, String> {
        request.admit()?;
        let now = crate::now_ms()?;
        // Authorization is refreshed before cached responses, including after revocation.
        let actor = if request.action == "accounts.accept" {
            None
        } else {
            Some(crate::accounts::authorize(
                &self.db,
                peer,
                &request.organization,
            )?)
        };
        if let Some(actor) = &actor {
            if let Some(project) = request.input.get("project").and_then(Value::as_str) {
                crate::accounts::require_project(
                    &self.db,
                    actor,
                    project,
                    &["owner", "admin", "member", "viewer", "billing"],
                )?;
            }
        }
        // Issuer metadata is a current authorized read, not an effect-replay entry.
        // Public metadata GETs must not consume permanent service-wide capacity.
        if matches!(
            request.action.as_str(),
            "secrets.discovery" | "secrets.jwks"
        ) {
            if self.profile.secrets.is_none() {
                return Err("operator secret policy unavailable".into());
            }
            return if request.action == "secrets.jwks" {
                crate::secrets::jwks(&self.db)
            } else {
                crate::secrets::discovery(&self.db)
            };
        }
        let authority = if let Some(actor) = &actor {
            let revision:i64=self.db.query_row("SELECT revision FROM hosted_memberships WHERE organization=?1 AND peer=?2 AND active=1",params![actor.organization,actor.peer],|r|r.get(0)).map_err(|_|"current authority unavailable")?;
            Some((actor.role.clone(), revision))
        } else {
            None
        };
        let fingerprint = crate::identity(&(request, authority))?;
        let prior:Option<(String,Option<Vec<u8>>)>=self.db.query_row("SELECT fingerprint,response FROM hosted_requests WHERE peer=?1 AND organization=?2 AND request_id=?3",params![peer,request.organization,request.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|"request lookup failed")?;
        if let Some((old, response)) = prior {
            if old != fingerprint {
                return Err("request identity conflicts with retained input".into());
            }
            return match response {
                Some(bytes) => serde_json::from_slice::<Result<Value, String>>(&bytes)
                    .map_err(|_| "retained response malformed")?,
                None => Err(
                    "request outcome uncertain; inspect retained operation before a new request"
                        .into(),
                ),
            };
        }
        let reserved=self.db.execute("INSERT INTO hosted_requests(peer,organization,request_id,fingerprint) SELECT ?1,?2,?3,?4 WHERE (SELECT COUNT(*) FROM hosted_requests)<?5",params![peer,request.organization,request.request_id,fingerprint,MAX_REQUEST_RECORDS]).map_err(|_|"request reservation failed")?;
        if reserved != 1 {
            return Err("request retention exhausted; preserve replay protection".into());
        }
        let result = (|| {
            if let Some(actor) = &actor {
                if let Some(project) = request.input.get("project").and_then(Value::as_str) {
                    crate::accounts::require_project(
                        &self.db,
                        actor,
                        project,
                        &["owner", "admin", "member", "viewer", "billing"],
                    )?;
                }
                if matches!(
                    request.action.as_str(),
                    "execution.run" | "fleet.provision" | "finance.reserve"
                ) && crate::operations::is_held(&self.db, &actor.organization)?
                {
                    return Err("organization incident hold denies new consumption".into());
                }
                if request.action == "execution.run" && self.profile.execution.is_none() {
                    return Err("operator execution profile unavailable".into());
                }
                if matches!(
                    request.action.as_str(),
                    "fleet.provision" | "fleet.configure"
                ) && self.profile.fleet.is_none()
                {
                    return Err("operator fleet profile unavailable".into());
                }
                if module_disabled(&request.action, self.profile.secrets.is_some()) {
                    return Err("operator secret policy unavailable".into());
                }
                let (module, action) = request.action.split_once('.').ok_or("action rejected")?;
                if module == "system" && action == "info" {
                    return Ok(
                        json!({"schema":SCHEMA,"service_id":self.profile.service_id,"peer":actor.peer,"organization":actor.organization,"role":actor.role,"connection":"iroh","alpn":"kiln.customer.v1","modules":["accounts","connections","finance","pipelines","execution","fleet","secrets","operations"],"production_ready":false}),
                    );
                }
                if module == "operations" && action == "backup" {
                    crate::require(actor, &["owner"])?;
                    if !self.profile.operator_peers.contains(&actor.peer) {
                        return Err(
                            "whole-service backup requires explicit host operator peer admission"
                                .into(),
                        );
                    }
                    let _barrier = self
                        .native_effects
                        .write()
                        .map_err(|_| "source snapshot barrier unavailable")?;
                    let backup_id = request
                        .input
                        .get("backup_id")
                        .and_then(Value::as_str)
                        .ok_or("backup_id required")?;
                    let profile = self
                        .profile
                        .operations
                        .as_ref()
                        .ok_or("operator operations profile unavailable")?;
                    let manifest = crate::operations::backup(&self.db, profile, backup_id, now)?;
                    return Ok(json!({"identity":manifest.identity()?,"manifest":manifest}));
                }
                if module == "connections" && action == "tick" {
                    let barrier = Arc::clone(&self.native_effects);
                    let _guard = barrier
                        .read()
                        .map_err(|_| "source effect barrier unavailable")?;
                    return self.tick_connection(actor, &request.input, now);
                }
                if module == "execution" && action == "run" {
                    let input: crate::execution::PipelineRequest =
                        serde_json::from_value(request.input.clone())
                            .map_err(|_| "execution request rejected")?;
                    crate::accounts::require_project(
                        &self.db,
                        actor,
                        &input.project,
                        &["owner", "admin", "member"],
                    )?;
                    let key = (actor.organization.clone(), input.attempt_id.clone());
                    let cancellation = Arc::new(AtomicBool::new(false));
                    {
                        let mut active = self
                            .cancellations
                            .lock()
                            .map_err(|_| "execution registry unavailable")?;
                        if active.contains_key(&key) {
                            return Err(
                                "attempt already running; inspect or cancel the original attempt"
                                    .into(),
                            );
                        }
                        active.insert(key.clone(), Arc::clone(&cancellation));
                    }
                    let result = crate::execution::run_pipeline(
                        &mut self.db,
                        actor,
                        input,
                        now,
                        &cancellation,
                    );
                    self.cancellations
                        .lock()
                        .map_err(|_| "execution registry unavailable")?
                        .remove(&key);
                    return result;
                }
                if module == "execution" && action == "cancel" {
                    #[derive(Deserialize)]
                    #[serde(deny_unknown_fields)]
                    struct Cancel {
                        project: String,
                        attempt_id: String,
                    }
                    let input: Cancel = serde_json::from_value(request.input.clone())
                        .map_err(|_| "cancellation request rejected")?;
                    crate::accounts::require_project(
                        &self.db,
                        actor,
                        &input.project,
                        &["owner", "admin", "member"],
                    )?;
                    let exists:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM hosted_execution WHERE org=?1 AND project=?2 AND attempt=?3)",params![actor.organization,input.project,input.attempt_id],|r|r.get(0)).map_err(|_|"execution cancellation lookup failed")?;
                    if !exists {
                        return Err("exact project attempt unavailable".into());
                    }
                    let active = self
                        .cancellations
                        .lock()
                        .map_err(|_| "execution registry unavailable")?;
                    let flag=active.get(&(actor.organization.clone(),input.attempt_id)).ok_or("attempt not running in this service; recover retained execution instead")?;
                    flag.store(true, Ordering::Relaxed);
                    return Ok(
                        json!({"cancellation_requested":true,"completion":"inspect retained execution outcome"}),
                    );
                }
                if module == "execution" && action == "recover" {
                    let attempt = request
                        .input
                        .get("attempt_id")
                        .and_then(Value::as_str)
                        .ok_or("attempt_id required")?;
                    if self
                        .cancellations
                        .lock()
                        .map_err(|_| "execution registry unavailable")?
                        .contains_key(&(actor.organization.clone(), attempt.to_owned()))
                    {
                        return Err(
                            "attempt has a live executor; request cancellation instead of recovery"
                                .into(),
                        );
                    }
                }
                match module {
                    "accounts" => crate::accounts::handle(
                        &mut self.db,
                        actor,
                        action,
                        request.input.clone(),
                        now,
                    ),
                    "connections" => {
                        let _barrier = self
                            .native_effects
                            .read()
                            .map_err(|_| "source effect barrier unavailable")?;
                        crate::connections::handle(
                            &mut self.db,
                            actor,
                            action,
                            request.input.clone(),
                            now,
                        )
                    }
                    "finance" => crate::finance::handle(
                        &mut self.db,
                        actor,
                        action,
                        request.input.clone(),
                        now,
                    ),
                    "pipelines" => crate::pipelines::handle(
                        &mut self.db,
                        actor,
                        action,
                        request.input.clone(),
                        now,
                    ),
                    "execution" => crate::execution::handle(
                        &mut self.db,
                        actor,
                        action,
                        request.input.clone(),
                        now,
                    ),
                    "fleet" => crate::fleet::handle(
                        &mut self.db,
                        actor,
                        action,
                        request.input.clone(),
                        now,
                    ),
                    "secrets" => crate::secrets::handle(
                        &mut self.db,
                        actor,
                        action,
                        request.input.clone(),
                        now,
                    ),
                    "operations" => crate::operations::handle(
                        &mut self.db,
                        actor,
                        action,
                        request.input.clone(),
                        now,
                    ),
                    _ => Err("unsupported customer action".into()),
                }
            } else {
                crate::accounts::accept_invitation(
                    &mut self.db,
                    peer,
                    &request.organization,
                    request.input.clone(),
                    now,
                )
            }
        })();
        let bytes =
            serde_json::to_vec(&result).map_err(|_| "response persistence encoding failed")?;
        if bytes.len() > crate::MAX_FRAME_BYTES / 2 {
            return Err("operation response exceeds retention bound; inspect operation".into());
        }
        self.db.execute("UPDATE hosted_requests SET response=?4 WHERE peer=?1 AND organization=?2 AND request_id=?3",params![peer,request.organization,request.request_id,bytes]).map_err(|_|"response durability unknown; inspect operation")?;
        self.db.execute("INSERT INTO hosted_audit(peer,organization,action,request_id,recorded_at_ms,success) VALUES(?1,?2,?3,?4,?5,?6)",params![peer,request.organization,request.action,request.request_id,crate::sqlite_integer(now)?,result.is_ok()]).map_err(|_|"audit write failed")?;
        result
    }
}
fn accounts_init(db: &Connection) -> Result<(), String> {
    crate::accounts::init(db)?;
    crate::connections::init(db)?;
    crate::finance::init(db)?;
    crate::pipelines::init(db)?;
    crate::execution::init(db)?;
    crate::fleet::init(db)?;
    crate::secrets::init(db)?;
    crate::operations::init(db)
}

fn module_disabled(action: &str, secrets_enabled: bool) -> bool {
    !secrets_enabled
        && action.starts_with("secrets.")
        && !matches!(
            action,
            "secrets.revoke" | "secrets.revoke_job" | "secrets.list"
        )
}
