//! Source-bound fixed-profile job shell with an empty ambient environment.
//! An explicitly selected namespace policy confines reviewed commands; absent
//! that policy they retain the worker account's filesystem/network authority.
//! Bounded Exec owns deadlines, cancellation and the helper's process group.
//! The per-step helper observes its sole child's resources; private bounded
//! transports publish command output before completion.

mod cache;
mod client;
pub(crate) mod files;
mod heartbeat;
mod process;
mod profile;
mod reports;
mod sandbox;
mod source;
mod streaming;
mod telemetry;

pub use heartbeat::cli_main as heartbeat_cli_main;
pub use profile::{Step, WorkerProfile};
pub use source::SourceContext;

/// Fixed per-step observation helper selected before normal CLI parsing.
pub fn stream_child_main(arguments: impl Iterator<Item = std::ffi::OsString>) -> i32 {
    streaming::child_main(arguments)
}

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn load_profile(path: &Path, identity: &str) -> Result<WorkerProfile, String> {
    if !profile::hex(identity, 32) {
        return Err("worker profile identity invalid".into());
    }
    let absolute;
    let path = if path.is_absolute() {
        path
    } else {
        absolute = std::env::current_dir()
            .map_err(|_| "worker profile directory unavailable")?
            .join(path);
        &absolute
    };
    let bytes = files::regular(path, profile::MAX_PROFILE_BYTES, false)?;
    if blake3::hash(&bytes).to_hex().as_str() != identity {
        return Err("worker original profile identity mismatch".into());
    }
    let profile: WorkerProfile =
        serde_json::from_slice(&bytes).map_err(|_| "worker profile JSON invalid")?;
    profile.admit()?;
    Ok(profile)
}

pub fn run_job(
    profile: &WorkerProfile,
    context: &SourceContext,
    cancellation: &AtomicBool,
) -> Result<i32, String> {
    if cancellation.load(Ordering::SeqCst) {
        return Ok(130);
    }
    let _slot = heartbeat::acquire_slot(profile)?;
    let source = source::prepare(profile, context)?;
    let service = client::ServiceClient::new(profile)?;
    let binding = service.await_binding(profile, &source.resolution, cancellation)?;
    let mut total_artifact_bytes = 0usize;
    let mut total_report_bytes = 0usize;
    let mut log_uploads = client::LogUploads::default();
    let mut telemetry_sequences = std::collections::BTreeMap::<&str, u64>::new();
    for (step_index, step) in profile.steps.iter().enumerate() {
        if !binding.nodes.iter().any(|node| node.id == step.node_id) {
            continue;
        }
        if cancellation.load(Ordering::SeqCst) {
            return Ok(130);
        }
        let sequence = telemetry_sequences
            .get(step.node_id.as_str())
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("worker telemetry sequence exhausted")?;
        let mut cache_observations = Vec::new();
        let cache_key = if let Some(config) = &step.cache {
            let (key, observation) = cache::restore(
                &service,
                profile,
                &binding,
                &step.node_id,
                config,
                &source.checkout,
            )?;
            cache_observations.push(observation);
            Some(key)
        } else {
            None
        };
        let clock = telemetry::StepClock::start()?;
        let output = process::execute_streamed(
            profile,
            step,
            &source.checkout,
            cancellation,
            binding.log_bounds.maximum_chunk_bytes,
            |chunk| {
                service.upload_log_chunk(
                    profile,
                    &source.resolution,
                    &binding,
                    &step.node_id,
                    chunk,
                    &mut log_uploads,
                )
            },
        )?;
        let measurement = clock.finish(
            u32::try_from(step_index).map_err(|_| "worker step index exceeds bound")?,
            output.code,
            output.cpu_time_us,
            output.peak_memory_bytes,
            Vec::new(),
        );
        // Reports must survive process failure and failed log/artifact ingress.
        let reports = service.upload_reports(
            profile,
            &source.resolution,
            &binding,
            step,
            &source.checkout,
            &mut total_report_bytes,
        );
        let artifacts = (|| {
            for artifact in &step.artifacts {
                let bytes =
                    files::artifact(&source.checkout, artifact, profile.maximum_artifact_bytes)?;
                total_artifact_bytes = total_artifact_bytes
                    .checked_add(bytes.len())
                    .ok_or("worker artifact total overflow")?;
                if total_artifact_bytes > profile.maximum_total_artifact_bytes {
                    return Err("worker artifact total exceeds bound".into());
                }
                let name = artifact
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or("worker artifact name invalid")?;
                service.upload(
                    profile,
                    &source.resolution,
                    &binding,
                    &step.node_id,
                    name,
                    &bytes,
                )?;
            }
            Ok::<(), String>(())
        })();
        let mut uploaded = output.ingress.and(reports).and(artifacts);
        if output.code == 0 && uploaded.is_ok() {
            if let (Some(config), Some(key)) = (&step.cache, cache_key.as_ref()) {
                match cache::publish(
                    &service,
                    &binding,
                    &step.node_id,
                    config,
                    &source.checkout,
                    key,
                    true,
                ) {
                    Ok(observation) => cache_observations.push(observation),
                    Err(error) => uploaded = Err(error),
                }
            }
        }
        let telemetry = measurement.and_then(|mut measured| {
            measured.cache = cache_observations;
            service.upload_telemetry(
                &source.resolution,
                &binding,
                &step.node_id,
                sequence,
                measured,
            )
        });
        telemetry_sequences.insert(step.node_id.as_str(), sequence);
        let code = finish_step(output.code, uploaded.and(telemetry))?;
        if code != 0 {
            return Ok(code);
        }
        if cancellation.load(Ordering::SeqCst) {
            return Ok(130);
        }
    }
    Ok(0)
}

fn finish_step(code: i32, ingress: Result<(), String>) -> Result<i32, String> {
    // A real child failure, timeout or cancellation remains authoritative even
    // when its captured output cannot be admitted. Success requires ingress.
    if code != 0 {
        if ingress.is_err() {
            eprintln!("kiln worker: failed step output could not be admitted");
        }
        return Ok(code);
    }
    ingress?;
    Ok(0)
}

/// Parse arguments after the parent's `job` selector.
pub fn cli_main(mut arguments: impl Iterator<Item = String>) -> i32 {
    let (path, identity) = match (
        arguments.next().as_deref(),
        arguments.next(),
        arguments.next().as_deref(),
        arguments.next(),
        arguments.next(),
    ) {
        (Some("--profile"), Some(path), Some("--identity"), Some(identity), None) => {
            (path, identity)
        }
        _ => {
            eprintln!(
                "usage: kiln-web-service job --profile <JSON path> --identity <original-profile BLAKE3>"
            );
            return 2;
        }
    };
    let cancellation = Arc::new(AtomicBool::new(false));
    let mut registrations = Vec::new();
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        match signal_hook::flag::register(signal, cancellation.clone()) {
            Ok(id) => registrations.push(id),
            Err(_) => {
                for id in registrations {
                    signal_hook::low_level::unregister(id);
                }
                eprintln!("kiln worker: signal registration failed");
                return 1;
            }
        }
    }
    let result = (|| {
        let profile = load_profile(Path::new(&path), &identity)?;
        let context = SourceContext::from_environment()?;
        run_job(&profile, &context, &cancellation)
    })();
    for id in registrations {
        signal_hook::low_level::unregister(id);
    }
    match result {
        Ok(code) => code,
        Err(error) => {
            // All diagnostics are fixed worker-owned strings; never format a
            // provider/HTTP/child error, credential, request body or argv.
            eprintln!("kiln worker: {error}");
            if cancellation.load(Ordering::SeqCst) {
                130
            } else {
                1
            }
        }
    }
}

#[cfg(test)]
#[path = "worker/tests.rs"]
mod tests;
