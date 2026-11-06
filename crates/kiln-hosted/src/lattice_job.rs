//! Fixed-profile hosted execution task launched by the native Lattice runner.
//! Lattice owns workflow scheduling and task state; this shell owns one backend
//! request, read-only observation, captured output and cancellation teardown.
use std::io::Read;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use kiln_web_service::worker::{ExecutionBinding, ServiceClient, ServiceConnection, SourceContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lattice_execution::LatticeExecutionRequest;

pub const JOB_SCHEMA: &str = "kiln.hosted.lattice-job.v1";
const MAX_LOOKUP_MS: u64 = 60_000;
const MAX_RUN_MS: u64 = 86_400_000;
const MIN_POLL_MS: u64 = 10;
const MAX_POLL_MS: u64 = 5_000;
// Cancellation is not complete until the retained backend proves teardown.
const DEFAULT_CLEANUP_MS: u64 = 10_000;
const MIN_CLEANUP_MS: u64 = 1_000;
const MAX_CLEANUP_MS: u64 = 120_000;
const CONTROL_RPC_MS: u64 = 1_000;
const CANCELLATION_CHECK_MS: u64 = 50;
// The native runner destroys a cancelled task with SIGKILL, so the fixed task
// must observe the authoritative persisted cancellation intent itself.
const WEB_CANCEL_POLL_MS: u64 = 500;
const MAX_ROUTES: usize = 16;
const CANCELLED_EXIT: i32 = 130;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobProfile {
    pub schema: String,
    pub web: ServiceConnection,
    pub network: crate::transport::NetworkProfile,
    pub remote: iroh::EndpointAddr,
    pub organization: String,
    pub project_id: String,
    pub repository_id: String,
    pub node_id: String,
    pub policy_id: String,
    pub lookup_timeout_ms: u64,
    pub poll_interval_ms: u64,
    pub maximum_run_ms: u64,
    #[serde(default = "default_cleanup_ms")]
    pub cleanup_timeout_ms: u64,
}

fn default_cleanup_ms() -> u64 {
    DEFAULT_CLEANUP_MS
}

impl JobProfile {
    pub fn admit(&self) -> Result<(), String> {
        self.web.admit()?;
        self.network.admit()?;
        if self.schema != JOB_SCHEMA
            || [
                &self.organization,
                &self.project_id,
                &self.repository_id,
                &self.node_id,
                &self.policy_id,
            ]
            .iter()
            .any(|value| !crate::identifier(value))
            || self.lookup_timeout_ms == 0
            || self.lookup_timeout_ms > MAX_LOOKUP_MS
            || !(MIN_POLL_MS..=MAX_POLL_MS).contains(&self.poll_interval_ms)
            || self.poll_interval_ms > self.lookup_timeout_ms
            || self.maximum_run_ms == 0
            || self.maximum_run_ms > MAX_RUN_MS
            || !(MIN_CLEANUP_MS..=MAX_CLEANUP_MS).contains(&self.cleanup_timeout_ms)
        {
            return Err("Lattice hosted job profile outside admitted bounds".into());
        }
        let remote =
            serde_json::to_value(&self.remote).map_err(|_| "hosted route encoding failed")?;
        let routes = remote
            .get("addrs")
            .and_then(Value::as_array)
            .ok_or("explicit hosted route required")?;
        if routes.is_empty() || routes.len() > MAX_ROUTES {
            return Err("explicit bounded hosted routes required".into());
        }
        Ok(())
    }

    pub fn load(path: &Path, identity: &str) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|_| "Lattice job profile unavailable")?;
        if !file
            .metadata()
            .map_err(|_| "Lattice job profile metadata unavailable")?
            .is_file()
        {
            return Err("regular Lattice job profile required".into());
        }
        let mut bytes = Vec::new();
        file.take(crate::MAX_FRAME_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Lattice job profile read failed")?;
        if bytes.len() > crate::MAX_FRAME_BYTES
            || blake3::hash(&bytes).to_hex().as_str() != identity
        {
            return Err("Lattice job original profile identity mismatch".into());
        }
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "Lattice job profile contract rejected")?;
        profile.admit()?;
        Ok(profile)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    binding_identity: String,
    attempt_id: String,
    sequence: u64,
    state: String,
    result: Option<Value>,
    web_approval_verified: bool,
    source_verified: bool,
    admission_registered: bool,
    cancellation_requested: bool,
}

fn response(value: crate::Response, identity: &str) -> Result<Envelope, String> {
    if value.error.is_some() || value.result.is_none() {
        return Err("hosted Lattice task refused; inspect its retained backend request".into());
    }
    let envelope: Envelope = serde_json::from_value(value.result.ok_or("hosted result absent")?)
        .map_err(|_| "hosted Lattice observation contract rejected")?;
    if envelope.cancellation_requested && !envelope.admission_registered {
        return Err("hosted cancellation acknowledgement lacks a live admission".into());
    }
    if envelope.binding_identity != identity || envelope.attempt_id != format!("lattice-{identity}")
    {
        return Err("hosted Lattice observation identity mismatch".into());
    }
    let uncertain = envelope.state == "unknown" && envelope.result.is_none();
    if !(envelope.web_approval_verified && envelope.source_verified || uncertain) {
        return Err("hosted Lattice observation lacks verified source authority".into());
    }
    if let Some(result) = &envelope.result {
        if result.get("attempt_id").and_then(Value::as_str) != Some(envelope.attempt_id.as_str())
            || result.get("state").and_then(Value::as_str) != Some(envelope.state.as_str())
        {
            return Err("hosted Lattice receipt identity mismatch".into());
        }
    }
    Ok(envelope)
}

fn rpc(
    profile: &JobProfile,
    action: &str,
    request: &LatticeExecutionRequest,
    identity: &str,
) -> Result<crate::Request, String> {
    Ok(crate::Request {
        schema: crate::SCHEMA.into(),
        organization: profile.organization.clone(),
        request_id: format!("{action}-{identity}"),
        action: format!("execution.{action}"),
        input: serde_json::to_value(request)
            .map_err(|_| "Lattice backend request encoding failed")?,
    })
}

fn terminal(envelope: &Envelope) -> bool {
    matches!(
        envelope.state.as_str(),
        "succeeded" | "failed" | "cancelled" | "timed_out"
    )
}

/// Watches the authoritative Web run for cancellation intent and raises the
/// same flag the signal handlers use. The native runner gives a cancelled task
/// no signal, so this is the only cooperative cancellation source.
struct CancelWatch {
    stop: Arc<AtomicBool>,
    handle: std::thread::JoinHandle<()>,
}

impl CancelWatch {
    fn start(
        web: &ServiceConnection,
        run_id: &str,
        cancellation: &Arc<AtomicBool>,
        maximum_run_ms: u64,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let watch_stop = Arc::clone(&stop);
        let watch_cancellation = Arc::clone(cancellation);
        let connection = web.clone();
        let run_id = run_id.to_owned();
        let handle = std::thread::spawn(move || {
            let Ok(client) = ServiceClient::connect(&connection) else {
                let _ = std::io::Write::write_all(
                    &mut std::io::stderr().lock(),
                    b"kiln-hosted: cancellation watch unavailable\n",
                );
                return;
            };
            let deadline = Instant::now() + Duration::from_millis(maximum_run_ms);
            while !watch_stop.load(Ordering::SeqCst)
                && !watch_cancellation.load(Ordering::SeqCst)
                && Instant::now() < deadline
            {
                if client.execution_binding(&run_id).is_ok_and(|bound| {
                    bound.cancel_requested || bound.outcome == kiln_core::domain::Outcome::Cancelled
                }) {
                    let _ = std::io::Write::write_all(
                        &mut std::io::stderr().lock(),
                        b"kiln-hosted: persisted Web cancellation observed\n",
                    );
                    watch_cancellation.store(true, Ordering::SeqCst);
                    return;
                }
                std::thread::sleep(Duration::from_millis(WEB_CANCEL_POLL_MS));
            }
        });
        Self { stop, handle }
    }

    fn finish(self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.handle.join();
    }
}

async fn execute(
    profile: &JobProfile,
    request: &LatticeExecutionRequest,
    cancellation: &AtomicBool,
) -> Result<Envelope, String> {
    let identity = crate::identity(request)?;
    let endpoint = crate::transport::bind(&profile.network).await?;
    let outcome = async {
        if cancellation.load(Ordering::SeqCst) {
            return Err("Lattice job cancelled before backend submission".into());
        }
        let start = rpc(profile, "lattice_run", request, &identity)?;
        let get = rpc(profile, "lattice_get", request, &identity)?;
        let cancel = rpc(profile, "lattice_cancel", request, &identity)?;
        let run = crate::transport::call(&endpoint, &profile.remote, &start);
        tokio::pin!(run);
        let mut run_pending = true;
        let mut cancel_requested = false;
        let mut cancel_acknowledged = false;
        let mut cleanup_deadline = None;
        let run_deadline =
            tokio::time::Instant::now() + Duration::from_millis(profile.maximum_run_ms);
        let mut last_sequence = 0;
        let mut next_observation = tokio::time::Instant::now();
        loop {
            let wake_at = cleanup_deadline
                .unwrap_or(run_deadline)
                .min(tokio::time::Instant::now() + Duration::from_millis(CANCELLATION_CHECK_MS));
            tokio::select! {
                result = &mut run, if run_pending => {
                    run_pending = false;
                    if let Ok(result) = result {
                        let observed = response(result, &identity)?;
                        if terminal(&observed) { return Ok(observed); }
                        last_sequence = observed.sequence;
                    }
                    // An uncertain run RPC is never resubmitted: only get/cancel follow.
                }
                _ = tokio::time::sleep_until(wake_at) => {}
            }
            let now = tokio::time::Instant::now();
            if !cancel_requested && (cancellation.load(Ordering::SeqCst) || now >= run_deadline) {
                cancel_requested = true;
                let deadline = now + Duration::from_millis(profile.cleanup_timeout_ms);
                cleanup_deadline = Some(deadline);
            }
            if cancel_requested && !cancel_acknowledged {
                let deadline = cleanup_deadline.ok_or("cancellation cleanup deadline absent")?;
                let observed = response(
                    tokio::time::timeout_at(
                        deadline,
                        crate::transport::call(&endpoint, &profile.remote, &cancel),
                    )
                    .await
                    .map_err(|_| {
                        "hosted cancellation teardown remains uncertain; recover original attempt"
                    })??,
                    &identity,
                )?;
                if observed.sequence < last_sequence {
                    return Err("hosted cancellation observation regressed".into());
                }
                last_sequence = observed.sequence;
                cancel_acknowledged = observed.cancellation_requested;
                if terminal(&observed) {
                    return Ok(observed);
                }
            }
            if cleanup_deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
                return Err(
                    "hosted cancellation teardown remains uncertain; recover original attempt"
                        .into(),
                );
            }
            if (!run_pending || cancel_requested) && tokio::time::Instant::now() >= next_observation
            {
                let deadline = cleanup_deadline
                    .unwrap_or(run_deadline)
                    .min(tokio::time::Instant::now() + Duration::from_millis(CONTROL_RPC_MS));
                let result = tokio::time::timeout_at(
                    deadline,
                    crate::transport::call(&endpoint, &profile.remote, &get),
                )
                .await;
                let Ok(result) = result else { continue };
                let observed = response(result?, &identity)?;
                next_observation = tokio::time::Instant::now()
                    + Duration::from_millis(if cancel_requested {
                        profile.poll_interval_ms.min(CANCELLATION_CHECK_MS)
                    } else {
                        profile.poll_interval_ms
                    });
                if observed.sequence < last_sequence {
                    return Err("hosted observation regressed".into());
                }
                last_sequence = observed.sequence;
                if terminal(&observed) {
                    return Ok(observed);
                }
            }
        }
    }
    .await;
    endpoint.close().await;
    outcome
}

struct CapturedOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

fn captured_output(result: &Value) -> Result<CapturedOutput, String> {
    let jobs = result
        .get("jobs")
        .and_then(Value::as_array)
        .filter(|jobs| !jobs.is_empty())
        .ok_or("hosted job receipts absent")?;
    let mut output = CapturedOutput {
        stdout: Vec::new(),
        stderr: Vec::new(),
        stdout_truncated: false,
        stderr_truncated: false,
    };
    for job in jobs {
        if matches!(
            job.get("state").and_then(Value::as_str),
            Some("skipped" | "skipped_dependency")
        ) {
            continue;
        }
        let steps = job
            .get("steps")
            .and_then(Value::as_array)
            .ok_or("hosted step receipts absent")?;
        for step in steps {
            for (field, target, truncated) in [
                ("stdout", &mut output.stdout, &mut output.stdout_truncated),
                ("stderr", &mut output.stderr, &mut output.stderr_truncated),
            ] {
                if let Some(bytes) = step.get(field).and_then(Value::as_str) {
                    if target
                        .len()
                        .checked_add(bytes.len())
                        .is_none_or(|size| size > crate::MAX_FRAME_BYTES)
                    {
                        return Err("hosted captured output exceeds bound".into());
                    }
                    target.extend_from_slice(bytes.as_bytes());
                }
                let field = if field == "stdout" {
                    "stdout_truncated"
                } else {
                    "stderr_truncated"
                };
                *truncated |= step.get(field).and_then(Value::as_bool).unwrap_or(false);
            }
        }
    }
    Ok(output)
}

// r[impl kiln.web_hosted.adapter]
fn run_job(profile: &JobProfile, cancellation: &Arc<AtomicBool>) -> Result<i32, String> {
    let context = SourceContext::from_environment()?;
    if context.provider != "git" || context.repository != profile.repository_id {
        return Err("native Lattice source context differs from fixed job profile".into());
    }
    let web = ServiceClient::connect(&profile.web)?;
    let lookup_deadline = Instant::now() + Duration::from_millis(profile.lookup_timeout_ms);
    let bound: ExecutionBinding = loop {
        if cancellation.load(Ordering::SeqCst) {
            return Ok(CANCELLED_EXIT);
        }
        let remaining = lookup_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("native Lattice binding lookup deadline expired".into());
        }
        let found = web.lookup_execution_binding(
            &profile.project_id,
            &context,
            &profile.node_id,
            remaining,
        );
        if cancellation.load(Ordering::SeqCst) {
            return Ok(CANCELLED_EXIT);
        }
        if Instant::now() >= lookup_deadline {
            return Err("native Lattice binding lookup deadline expired".into());
        }
        if let Some(bound) = found? {
            break bound;
        }
        let remaining = lookup_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("native Lattice task could not find its exact active Web binding".into());
        }
        std::thread::sleep(remaining.min(Duration::from_millis(profile.poll_interval_ms)));
    };
    if cancellation.load(Ordering::SeqCst) {
        return Ok(CANCELLED_EXIT);
    }
    let request = LatticeExecutionRequest {
        policy_id: profile.policy_id.clone(),
        plan: bound.plan.clone(),
        binding: bound.binding.clone(),
        lattice_execution: bound
            .execution
            .clone()
            .ok_or("native Lattice execution absent")?,
        node_id: profile.node_id.clone(),
    };
    let watch = CancelWatch::start(
        &profile.web,
        &request.binding.run_id,
        cancellation,
        profile.maximum_run_ms,
    );
    if cancellation.load(Ordering::SeqCst) {
        watch.finish();
        return Ok(CANCELLED_EXIT);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "hosted task runtime unavailable")?;
    let observed = runtime.block_on(execute(profile, &request, cancellation.as_ref()));
    watch.finish();
    let observed = observed?;
    let result = observed
        .result
        .as_ref()
        .ok_or("hosted terminal receipt absent")?;
    if result.get("cleaned").and_then(Value::as_bool) != Some(true) {
        return Err("hosted task teardown is not proven".into());
    }
    if observed.state != "cancelled" {
        let output = captured_output(result)?;
        web.upload_execution_logs(
            &bound,
            &profile.node_id,
            &output.stdout,
            &output.stderr,
            output.stdout_truncated,
            output.stderr_truncated,
        )?;
        std::io::Write::write_all(&mut std::io::stdout().lock(), &output.stdout)
            .map_err(|_| "hosted task stdout unavailable")?;
        std::io::Write::write_all(&mut std::io::stderr().lock(), &output.stderr)
            .map_err(|_| "hosted task stderr unavailable")?;
    }
    Ok(match observed.state.as_str() {
        "succeeded" => 0,
        "cancelled" => CANCELLED_EXIT,
        _ => 1,
    })
}

pub fn cli_main(path: &Path, identity: &str) -> Result<i32, String> {
    let profile = JobProfile::load(path, identity)?;
    let cancellation = Arc::new(AtomicBool::new(false));
    let mut registrations = Vec::new();
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        match signal_hook::flag::register(signal, Arc::clone(&cancellation)) {
            Ok(id) => registrations.push(id),
            Err(_) => {
                for id in registrations {
                    signal_hook::low_level::unregister(id);
                }
                return Err("hosted Lattice task signal handler unavailable".into());
            }
        }
    }
    let result = run_job(&profile, &cancellation);
    for id in registrations {
        signal_hook::low_level::unregister(id);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn failed_job_output_survives_skipped_dependencies_without_inventing_output(
    ) -> Result<(), String> {
        let output = captured_output(&json!({"jobs":[
            {"state":"failed","steps":[{"stdout":"exact source output\n","stderr":"failure\n","stderr_truncated":true}]},
            {"state":"skipped_dependency"}
        ]}))?;
        assert_eq!(output.stdout, b"exact source output\n");
        assert_eq!(output.stderr, b"failure\n");
        assert!(!output.stdout_truncated);
        assert!(output.stderr_truncated);
        assert!(captured_output(&json!({"jobs":[]})).is_err());
        assert!(captured_output(&json!({"jobs":[{"state":"succeeded"}]})).is_err());
        Ok(())
    }

    #[test]
    fn backend_receipts_require_exact_attempt_state_and_both_verification_facts(
    ) -> Result<(), String> {
        let identity = "a".repeat(64);
        let make = |approval,
                    source,
                    receipt_attempt: &str,
                    receipt_state: &str|
         -> Result<crate::Response, String> {
            serde_json::from_value(json!({"schema":crate::SCHEMA,"request_id":"test","error":null,"result":{
                "binding_identity":identity,"attempt_id":format!("lattice-{identity}"),"sequence":3,"state":"succeeded",
                "web_approval_verified":approval,"source_verified":source,
                "admission_registered":false,"cancellation_requested":false,
                "result":{"attempt_id":receipt_attempt,"state":receipt_state}
            }})).map_err(|_| "fixture response rejected".into())
        };
        let attempt = format!("lattice-{identity}");
        assert!(response(make(true, true, &attempt, "succeeded")?, &identity).is_ok());
        for (approval, source, attempt, state) in [
            (false, true, attempt.as_str(), "succeeded"),
            (true, false, attempt.as_str(), "succeeded"),
            (true, true, "different-execution", "succeeded"),
            (true, true, attempt.as_str(), "running"),
        ] {
            assert!(response(make(approval, source, attempt, state)?, &identity).is_err());
        }
        Ok(())
    }

    #[test]
    fn cleanup_window_is_bounded_and_defaults_without_widening_identity() -> Result<(), String> {
        let profile = |cleanup: Option<u64>| -> Result<JobProfile, String> {
            let mut value = json!({
                "schema":JOB_SCHEMA,
                "web":{"service_origin":"https://web.example.test","tls_ca_path":"/srv/web-ca.pem","token_file":"/srv/worker.token","http_timeout_ms":1000,"maximum_response_bytes":1048576},
                "network":{"key_file":"/srv/task.key","bind_address":"127.0.0.1:0","relay_urls":[]},
                "remote":{"id":"a".repeat(64),"addrs":[{"Ip":"127.0.0.1:4000"}]},
                "organization":"example","project_id":"example","repository_id":"b".repeat(64),
                "node_id":"build","policy_id":"lattice-example",
                "lookup_timeout_ms":1000,"poll_interval_ms":100,"maximum_run_ms":1000
            })
            .as_object()
            .cloned()
            .ok_or("profile fixture not an object")?;
            if let Some(cleanup) = cleanup {
                value.insert("cleanup_timeout_ms".into(), json!(cleanup));
            }
            serde_json::from_value(Value::Object(value))
                .map_err(|_| "profile fixture rejected".into())
        };
        let defaulted = profile(None)?;
        defaulted.admit()?;
        assert_eq!(defaulted.cleanup_timeout_ms, DEFAULT_CLEANUP_MS);
        for cleanup in [MIN_CLEANUP_MS, MAX_CLEANUP_MS] {
            profile(Some(cleanup))?.admit()?;
        }
        for cleanup in [0, MIN_CLEANUP_MS - 1, MAX_CLEANUP_MS + 1] {
            assert!(profile(Some(cleanup))?.admit().is_err());
        }
        Ok(())
    }

    #[test]
    fn pending_cancellation_requires_live_acknowledgement_and_is_not_terminal() -> Result<(), String>
    {
        let identity = "a".repeat(64);
        let make = |registered, requested, state| -> Result<crate::Response, String> {
            serde_json::from_value(json!({"schema":crate::SCHEMA,"request_id":"pending","error":null,"result":{
                "binding_identity":identity,"attempt_id":format!("lattice-{identity}"),"sequence":0,"state":state,
                "result":null,"web_approval_verified":false,"source_verified":false,
                "admission_registered":registered,"cancellation_requested":requested
            }})).map_err(|_| "pending fixture rejected".into())
        };
        let absent = response(make(false, false, "unknown")?, &identity)?;
        assert!(!terminal(&absent));
        assert!(!absent.cancellation_requested);
        assert!(response(make(false, true, "unknown")?, &identity).is_err());
        let acknowledged = response(make(true, true, "unknown")?, &identity)?;
        assert!(acknowledged.cancellation_requested);
        assert!(!terminal(&acknowledged));
        assert!(response(make(true, true, "cancelled")?, &identity).is_err());
        Ok(())
    }
}
