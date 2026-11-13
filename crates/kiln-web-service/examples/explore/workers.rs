//! Synthetic worker heartbeat observations through the real admitted worker API.
use std::path::Path;

use kiln_web_service::engine::{Engine, Role, WorkerStatusProfile};
use serde_json::json;

use super::{Result, accepted, now_ms};
const HEARTBEAT_TTL_MS: u64 = 120_000;
const HEARTBEAT_REFRESH_MS: u64 = HEARTBEAT_TTL_MS / 2;
const EXPIRING_HEARTBEAT_MS: u64 = 300;

pub struct Heartbeats {
    target: String,
    started_at_ms: u64,
    last_sent_ms: u64,
    sequence: u64,
}
impl Heartbeats {
    pub fn refresh(&mut self, engine: &mut Engine) -> Result {
        let now = now_ms()?;
        if now.saturating_sub(self.last_sent_ms) < HEARTBEAT_REFRESH_MS {
            return Ok(());
        }
        self.sequence += 1;
        send(
            engine,
            "docs",
            &self.target,
            self.started_at_ms,
            now,
            self.sequence,
            HEARTBEAT_TTL_MS,
        )?;
        self.last_sent_ms = now;
        Ok(())
    }
}
fn send(
    engine: &mut Engine,
    project: &str,
    target: &str,
    started: u64,
    now: u64,
    sequence: u64,
    ttl: u64,
) -> Result {
    let body = serde_json::to_vec(
        &json!({"schema":"kiln.worker-heartbeat.v1","worker_id":format!("synthetic-{project}"),"project_id":project,"target_id":target,"session_id":blake3::hash(format!("synthetic-{project}").as_bytes()).to_hex().to_string(),"session_started_at_ms":started,"sequence":sequence,"sent_at_ms":now,"ttl_ms":ttl,"observed_slots":4,"active_slots":1}),
    )?;
    accepted(engine.api("POST", "/api/workers/heartbeat", &body, Role::Worker)?)?;
    Ok(())
}
// r[impl kiln.web.exploration.scenario.coverage]
pub fn configure(root: &Path, engine: &mut Engine) -> Result<Heartbeats> {
    let mut workers = Vec::new();
    let mut targets = Vec::new();
    for project in ["docs", "release"] {
        let target = kiln_web_service::profile::worker_target_identity(
            &format!("example-{project}"),
            &root.join(format!("{project}-simulator.sock")),
            "linux-x86_64",
        )?
        .as_str()
        .to_owned();
        workers.push(json!({"worker_id":format!("synthetic-{project}"),"principal":"system:worker","project_id":project,"target_id":target,"maximum_slots":4}));
        targets.push(target);
    }
    let profile: WorkerStatusProfile = serde_json::from_value(
        json!({"schema":"kiln.worker-status.v1","maximum_ttl_ms":HEARTBEAT_TTL_MS,"maximum_clock_skew_ms":1000,"workers":workers}),
    )?;
    engine.configure_worker_status(profile)?;
    let now = now_ms()?;
    send(
        engine,
        "docs",
        targets.first().ok_or("Docs target absent")?,
        now,
        now,
        1,
        HEARTBEAT_TTL_MS,
    )?;
    send(
        engine,
        "release",
        targets.get(1).ok_or("Release target absent")?,
        now,
        now,
        1,
        EXPIRING_HEARTBEAT_MS,
    )?;
    Ok(Heartbeats {
        target: targets.into_iter().next().ok_or("Docs target absent")?,
        started_at_ms: now,
        last_sent_ms: now,
        sequence: 1,
    })
}
