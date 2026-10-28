use std::collections::BTreeSet;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::api::{error, json_response};
use super::{ApiResponse, Engine};
use crate::access::Principal;

const MAX_WORKERS: usize = 256;
const MAX_RETAINED_WORKERS: usize = 4096;
const MAX_HEARTBEAT_BYTES: usize = 8192;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerStatusProfile {
    pub schema: String,
    pub maximum_ttl_ms: u64,
    pub maximum_clock_skew_ms: u64,
    pub workers: Vec<WorkerAdmission>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerAdmission {
    pub worker_id: String,
    pub principal: String,
    pub project_id: String,
    pub target_id: String,
    pub maximum_slots: usize,
}
fn component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
fn hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl WorkerStatusProfile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, 1_048_576, false)?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "worker status profile JSON invalid")?;
        profile.admit()?;
        Ok(profile)
    }
    pub fn admit(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        if self.schema != "kiln.worker-status.v1"
            || self.maximum_ttl_ms < 300
            || self.maximum_ttl_ms > 120_000
            || self.maximum_clock_skew_ms > 10_000
            || self.workers.is_empty()
            || self.workers.len() > MAX_WORKERS
            || self.workers.iter().any(|w| {
                !component(&w.worker_id)
                    || w.principal.is_empty()
                    || w.principal.len() > 256
                    || w.principal.bytes().any(|b| !b.is_ascii_graphic())
                    || !component(&w.project_id)
                    || !hex(&w.target_id)
                    || w.maximum_slots == 0
                    || w.maximum_slots > 256
                    || !ids.insert(&w.worker_id)
            })
        {
            return Err("worker status profile outside admitted bounds".into());
        }
        Ok(())
    }
    pub fn preflight_for(&self, service: &crate::profile::ServiceProfile) -> Result<(), String> {
        self.admit()?;
        for worker in &self.workers {
            let project = service
                .projects
                .iter()
                .find(|p| p.id == worker.project_id)
                .ok_or("worker status project not installed")?;
            let target = crate::profile::worker_target_identity(
                &project.workflow.provider_profile_id,
                &project.workflow.socket_path,
                &project.workflow.platform,
            )?;
            if target.as_str() != worker.target_id {
                return Err("worker status target differs from installed workflow".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Heartbeat {
    schema: String,
    worker_id: String,
    project_id: String,
    target_id: String,
    session_id: String,
    session_started_at_ms: u64,
    sequence: u64,
    sent_at_ms: u64,
    ttl_ms: u64,
    observed_slots: usize,
    active_slots: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Retained {
    heartbeat: Heartbeat,
    received_at_ms: u64,
    principal: String,
}
impl Retained {
    fn online(&self, now_ms: u64) -> bool {
        now_ms >= self.received_at_ms
            && now_ms < self.received_at_ms.saturating_add(self.heartbeat.ttl_ms)
            && now_ms
                < self
                    .heartbeat
                    .sent_at_ms
                    .saturating_add(self.heartbeat.ttl_ms)
    }
}

/// Pure admission. Repeated identical observations never renew their TTL.
fn admit(
    profile: &WorkerStatusProfile,
    binding: &WorkerAdmission,
    heartbeat: &Heartbeat,
    prior: Option<&Retained>,
    now_ms: u64,
) -> Result<bool, ()> {
    if heartbeat.schema != "kiln.worker-heartbeat.v1"
        || heartbeat.worker_id != binding.worker_id
        || heartbeat.project_id != binding.project_id
        || heartbeat.target_id != binding.target_id
        || !hex(&heartbeat.session_id)
        || heartbeat.ttl_ms < 300
        || heartbeat.ttl_ms > profile.maximum_ttl_ms
        || heartbeat.observed_slots == 0
        || heartbeat.observed_slots > binding.maximum_slots
        || heartbeat.active_slots > heartbeat.observed_slots
        || heartbeat.sent_at_ms > now_ms.saturating_add(profile.maximum_clock_skew_ms)
        || now_ms >= heartbeat.sent_at_ms.saturating_add(heartbeat.ttl_ms)
        || heartbeat.session_started_at_ms > heartbeat.sent_at_ms
        || heartbeat.sequence > i64::MAX as u64
    {
        return Err(());
    }
    match prior {
        Some(p) if p.heartbeat.session_id == heartbeat.session_id => {
            if p.principal != binding.principal {
                return Err(());
            }
            if p.heartbeat == *heartbeat {
                return Ok(false);
            }
            if heartbeat.session_started_at_ms != p.heartbeat.session_started_at_ms
                || heartbeat.sequence <= p.heartbeat.sequence
                || heartbeat.sent_at_ms < p.heartbeat.sent_at_ms
                || now_ms < p.received_at_ms
            {
                return Err(());
            }
        }
        previous => {
            if let Some(p) = previous {
                if p.online(now_ms)
                    || heartbeat.session_started_at_ms <= p.heartbeat.session_started_at_ms
                    || heartbeat.session_started_at_ms <= p.heartbeat.sent_at_ms
                {
                    return Err(());
                }
            }
        }
    }
    Ok(true)
}

pub(super) struct Store {
    profile: WorkerStatusProfile,
    connection: Connection,
}
#[derive(Clone, Copy)]
pub(super) struct Observation {
    pub online: bool,
    pub observed_slots: usize,
    pub active_slots: usize,
    pub last_seen_at_ms: Option<u64>,
}
impl Store {
    fn open(path: &Path, profile: WorkerStatusProfile) -> Result<Self, String> {
        profile.admit()?;
        let _file = super::storage::owned_file(path)?;
        let connection = Connection::open(path).map_err(|_| "worker status store unavailable")?;
        connection
            .busy_timeout(std::time::Duration::from_millis(500))
            .map_err(|_| "worker status store timeout unavailable")?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|_| "worker status schema version unavailable")?;
        let tables: i64 = connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |r| r.get(0)).map_err(|_| "worker status schema inventory unavailable")?;
        if (version == 0 && tables != 0) || (version != 0 && version != 1) {
            return Err("unknown worker status database cannot be adopted".into());
        }
        connection
            .pragma_update(None, "journal_mode", "DELETE")
            .map_err(|_| "worker status durability unavailable")?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|_| "worker status durability unavailable")?;
        if version == 0 {
            connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE worker_status (worker_id TEXT PRIMARY KEY NOT NULL, observation BLOB NOT NULL); PRAGMA user_version=1; COMMIT;").map_err(|_| "worker status schema initialization unavailable")?;
        }
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM worker_status", [], |r| r.get(0))
            .map_err(|_| "worker status retention inventory unavailable")?;
        if count < 0 || count > MAX_RETAINED_WORKERS as i64 {
            return Err("worker status retained worker bound exceeded".into());
        }
        Ok(Self {
            profile,
            connection,
        })
    }
    fn prior(&self, worker_id: &str) -> Result<Option<Retained>, String> {
        let length: Option<i64> = self
            .connection
            .query_row(
                "SELECT length(observation) FROM worker_status WHERE worker_id=?1",
                [worker_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| "worker status retained bound unavailable")?;
        if length.is_some_and(|n| n < 0 || n > MAX_HEARTBEAT_BYTES as i64) {
            return Err("worker status retained bound invalid".into());
        }
        let bytes: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT observation FROM worker_status WHERE worker_id=?1",
                [worker_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| "worker status read unavailable")?;
        bytes
            .map(|b| {
                if b.len() > MAX_HEARTBEAT_BYTES {
                    return Err("worker status retained bound invalid".into());
                }
                serde_json::from_slice(&b)
                    .map_err(|_| "worker status retained observation invalid".into())
            })
            .transpose()
    }
    pub(super) fn observe_target(
        &self,
        target_id: &str,
        now_ms: u64,
    ) -> Result<Option<Observation>, String> {
        let bindings: Vec<_> = self
            .profile
            .workers
            .iter()
            .filter(|w| w.target_id == target_id)
            .collect();
        if bindings.is_empty() {
            return Ok(None);
        }
        let mut result = Observation {
            online: false,
            observed_slots: 0,
            active_slots: 0,
            last_seen_at_ms: None,
        };
        for binding in bindings {
            if let Some(mut prior) = self.prior(&binding.worker_id)? {
                if prior.heartbeat.project_id != binding.project_id
                    || prior.heartbeat.target_id != binding.target_id
                    || prior.principal != binding.principal
                {
                    continue;
                }
                result.last_seen_at_ms = result.last_seen_at_ms.max(Some(prior.received_at_ms));
                prior.heartbeat.ttl_ms = prior.heartbeat.ttl_ms.min(self.profile.maximum_ttl_ms);
                if prior.online(now_ms) {
                    result.online = true;
                    result.observed_slots +=
                        prior.heartbeat.observed_slots.min(binding.maximum_slots);
                    result.active_slots += prior.heartbeat.active_slots;
                }
            }
        }
        Ok(Some(result))
    }
}

impl Engine {
    pub fn configure_worker_status(&mut self, profile: WorkerStatusProfile) -> Result<(), String> {
        profile.preflight_for(&self.profile)?;
        self.worker_status = Some(Store::open(
            &self.profile.state_directory.join("worker-status.sqlite"),
            profile,
        )?);
        Ok(())
    }
    pub(super) fn worker_heartbeat(
        &mut self,
        principal: &Principal,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<ApiResponse, String> {
        if principal.role != super::Role::Worker {
            return error(403, "worker_heartbeat_role");
        }
        if bytes.len() > MAX_HEARTBEAT_BYTES {
            return error(400, "worker_heartbeat_bound");
        }
        let Ok(heartbeat) = serde_json::from_slice::<Heartbeat>(bytes) else {
            return error(400, "worker_heartbeat_invalid");
        };
        let Some(store) = &self.worker_status else {
            return error(409, "worker_heartbeat_not_configured");
        };
        let Some(binding) = store
            .profile
            .workers
            .iter()
            .find(|w| w.worker_id == heartbeat.worker_id)
        else {
            return error(403, "worker_heartbeat_binding");
        };
        if binding.principal != principal.id || !self.state.enrolled.contains(&binding.project_id) {
            return error(403, "worker_heartbeat_binding");
        }
        let prior = store.prior(&heartbeat.worker_id)?;
        let Ok(update) = admit(&store.profile, binding, &heartbeat, prior.as_ref(), now_ms) else {
            return error(409, "worker_heartbeat_stale_or_conflicting");
        };
        if update {
            if prior.is_none() {
                let count: i64 = store
                    .connection
                    .query_row("SELECT COUNT(*) FROM worker_status", [], |r| r.get(0))
                    .map_err(|_| "worker status retention count unavailable")?;
                if count < 0 || count >= MAX_RETAINED_WORKERS as i64 {
                    return error(409, "worker_heartbeat_retention_full");
                }
            }
            let record = Retained {
                heartbeat,
                received_at_ms: now_ms,
                principal: principal.id.clone(),
            };
            let bytes =
                serde_json::to_vec(&record).map_err(|_| "worker status encoding unavailable")?;
            store.connection.execute("INSERT INTO worker_status(worker_id,observation) VALUES(?1,?2) ON CONFLICT(worker_id) DO UPDATE SET observation=excluded.observation", params![record.heartbeat.worker_id, bytes]).map_err(|_| "worker status retention unavailable")?;
        }
        json_response(
            200,
            serde_json::json!({"accepted":true,"duplicate":!update}),
        )
    }
    pub(crate) fn worker_capacity_available(
        &self,
        project_id: &str,
        now_ms: u64,
    ) -> Result<Option<bool>, String> {
        let Some(store) = &self.worker_status else {
            return Ok(None);
        };
        let Some(binding) = store
            .profile
            .workers
            .iter()
            .find(|w| w.project_id == project_id)
        else {
            return Ok(None);
        };
        let observation = store
            .observe_target(&binding.target_id, now_ms)?
            .ok_or("configured worker observation unavailable")?;
        let project = self
            .profile
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .ok_or("worker capacity project missing")?;
        let projects: BTreeSet<_> = self
            .profile
            .projects
            .iter()
            .filter(|p| {
                p.workflow.provider_profile_id == project.workflow.provider_profile_id
                    && p.workflow.socket_path == project.workflow.socket_path
                    && p.workflow.platform == project.workflow.platform
            })
            .map(|p| p.id.as_str())
            .collect();
        // Dispatch reserves capacity before the next daemon observation. Do not
        // double-count locks already reflected by nonterminal durable runs.
        let reserved = self
            .state
            .runs
            .values()
            .filter(|run| {
                projects.contains(run.project_id.as_str())
                    && run.phase != "queued"
                    && !run.outcome().is_terminal()
            })
            .count();
        Ok(Some(
            observation.online
                && observation.observed_slots > observation.active_slots.max(reserved),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> WorkerStatusProfile {
        WorkerStatusProfile {
            schema: "kiln.worker-status.v1".into(),
            maximum_ttl_ms: 3000,
            maximum_clock_skew_ms: 10,
            workers: vec![WorkerAdmission {
                worker_id: "w".into(),
                principal: "worker".into(),
                project_id: "p".into(),
                target_id: "a".repeat(64),
                maximum_slots: 2,
            }],
        }
    }
    fn heartbeat() -> Heartbeat {
        Heartbeat {
            schema: "kiln.worker-heartbeat.v1".into(),
            worker_id: "w".into(),
            project_id: "p".into(),
            target_id: "a".repeat(64),
            session_id: "b".repeat(64),
            session_started_at_ms: 1000,
            sequence: 0,
            sent_at_ms: 1000,
            ttl_ms: 3000,
            observed_slots: 2,
            active_slots: 1,
        }
    }
    #[test]
    fn admitted_duplicate_does_not_renew_expiry_and_replay_cannot_resurrect() {
        let p = profile();
        let h = heartbeat();
        let worker = p
            .workers
            .first()
            .unwrap_or_else(|| unreachable!("{}", "one admitted worker"));
        assert_eq!(admit(&p, worker, &h, None, 1000), Ok(true));
        let retained = Retained {
            heartbeat: h.clone(),
            received_at_ms: 1000,
            principal: worker.principal.clone(),
        };
        assert_eq!(admit(&p, worker, &h, Some(&retained), 2000), Ok(false));
        assert!(!retained.online(4000));
        assert!(admit(&p, worker, &h, Some(&retained), 4000).is_err());
    }
    #[test]
    fn future_wrong_target_regressing_sequence_and_live_session_replacement_denied() {
        let p = profile();
        let h = heartbeat();
        let worker = p
            .workers
            .first()
            .unwrap_or_else(|| unreachable!("{}", "one admitted worker"));
        let retained = Retained {
            heartbeat: h.clone(),
            received_at_ms: 1000,
            principal: worker.principal.clone(),
        };
        let mut changed = h.clone();
        changed.sent_at_ms = 1011;
        assert!(admit(&p, worker, &changed, None, 1000).is_err());
        changed = h.clone();
        changed.target_id = "c".repeat(64);
        assert!(admit(&p, worker, &changed, None, 1000).is_err());
        changed = h.clone();
        changed.sequence = 0;
        changed.sent_at_ms = 1100;
        assert!(admit(&p, worker, &changed, Some(&retained), 1100).is_err());
        changed = h;
        changed.session_id = "c".repeat(64);
        changed.session_started_at_ms = 1100;
        changed.sent_at_ms = 1100;
        assert!(admit(&p, worker, &changed, Some(&retained), 1100).is_err());
        changed.session_started_at_ms = 4001;
        changed.sent_at_ms = 4001;
        assert_eq!(admit(&p, worker, &changed, Some(&retained), 4001), Ok(true));
    }
    #[test]
    fn retained_liveness_respects_current_principal_and_shorter_ttl_after_restart() {
        let root = tempfile::tempdir()
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "private temporary directory", error));
        let path = root.path().join("workers.sqlite");
        let p = profile();
        let worker = p
            .workers
            .first()
            .unwrap_or_else(|| unreachable!("{}", "one admitted worker"));
        let store = Store::open(&path, p.clone())
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "open worker store", error));
        let retained = Retained {
            heartbeat: heartbeat(),
            received_at_ms: 1000,
            principal: worker.principal.clone(),
        };
        store
            .connection
            .execute(
                "INSERT INTO worker_status VALUES(?1,?2)",
                params![
                    retained.heartbeat.worker_id,
                    serde_json::to_vec(&retained)
                        .unwrap_or_else(|error| unreachable!("retained heartbeat: {error}"))
                ],
            )
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "insert retained heartbeat", error));
        let observed = store
            .observe_target(&worker.target_id, 1100)
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "read worker observation", error))
            .unwrap_or_else(|| unreachable!("{}", "retained worker observation"));
        assert!(observed.online);
        assert_eq!((observed.observed_slots, observed.active_slots), (2, 1));
        drop(store);
        let mut shorter = p.clone();
        shorter.maximum_ttl_ms = 300;
        let store = Store::open(&path, shorter)
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "reopen worker store", error));
        let observed = store
            .observe_target(&worker.target_id, 2000)
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "read worker observation", error))
            .unwrap_or_else(|| unreachable!("{}", "retained worker observation"));
        assert!(!observed.online);
        assert_eq!((observed.observed_slots, observed.active_slots), (0, 0));
        drop(store);
        let mut rotated = p.clone();
        rotated
            .workers
            .first_mut()
            .unwrap_or_else(|| unreachable!("{}", "one admitted worker"))
            .principal = "new:worker".into();
        let store = Store::open(&path, rotated)
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "reopen worker store", error));
        let observed = store
            .observe_target(&worker.target_id, 1100)
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "read worker observation", error))
            .unwrap_or_else(|| unreachable!("{}", "retained worker observation"));
        assert!(!observed.online);
        assert_eq!((observed.observed_slots, observed.active_slots), (0, 0));
    }
    #[test]
    fn lost_observations_allow_sequence_gaps_but_not_regression_or_changed_replay() {
        let p = profile();
        let worker = p
            .workers
            .first()
            .unwrap_or_else(|| unreachable!("{}", "one admitted worker"));
        let retained = Retained {
            heartbeat: heartbeat(),
            received_at_ms: 1000,
            principal: worker.principal.clone(),
        };
        let mut resumed = heartbeat();
        resumed.sequence = 5;
        resumed.sent_at_ms = 5000;
        assert_eq!(admit(&p, worker, &resumed, Some(&retained), 5000), Ok(true));
        // A daemon may have started while the controller was unavailable. Its
        // first delivered observation is fresh even if its session is older.
        assert_eq!(admit(&p, worker, &resumed, None, 5000), Ok(true));
        let latest = Retained {
            heartbeat: resumed.clone(),
            received_at_ms: 5000,
            principal: worker.principal.clone(),
        };
        resumed.sequence = 4;
        resumed.sent_at_ms = 5100;
        assert!(admit(&p, worker, &resumed, Some(&latest), 5100).is_err());
        resumed.sequence = 5;
        assert!(admit(&p, worker, &resumed, Some(&latest), 5100).is_err());
    }
    #[test]
    fn compatible_workers_share_a_target_but_cannot_share_worker_identity() {
        let mut p = profile();
        let mut second = p
            .workers
            .first()
            .unwrap_or_else(|| unreachable!("{}", "one admitted worker"))
            .clone();
        second.worker_id = "second".into();
        p.workers.push(second);
        assert!(p.admit().is_ok());
        let first_id = p
            .workers
            .first()
            .unwrap_or_else(|| unreachable!("{}", "one admitted worker"))
            .worker_id
            .clone();
        p.workers
            .last_mut()
            .unwrap_or_else(|| unreachable!("{}", "second admitted worker"))
            .worker_id = first_id;
        assert!(p.admit().is_err());
    }
}
