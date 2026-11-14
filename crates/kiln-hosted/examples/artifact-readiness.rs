//! Real hosted-cache consumer rail; does not execute a VM or change service policy.
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use kiln_hosted::cache::readiness::{capture, phase};
use kiln_hosted::cache::{self, Context, HostedCacheConfig, Policy};
use kiln_web_service::cache::WorkerCacheProfile;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    root: PathBuf,
    workload: Workload,
    samples: usize,
    delay_phase: Option<String>,
    delay_ms: u64,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Workload {
    id: String,
    payload_bytes: usize,
    files: usize,
    symlinks: usize,
    fanout: usize,
}

fn outputs(root: &Path, workload: &Workload) -> Result<(), String> {
    std::fs::create_dir(root.join("product")).map_err(|e| e.to_string())?;
    let bytes = vec![b'x'; workload.payload_bytes / workload.files];
    for index in 0..workload.files {
        std::fs::write(root.join(format!("product/{index:04}")), &bytes)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn program(root: &Path, workload: &Workload) -> Result<String, String> {
    let count = std::fs::read_dir(root.join("product"))
        .map_err(|e| e.to_string())?
        .count();
    if count != workload.files {
        return Err("materialized regular-file count mismatch".into());
    }
    let mut hash = blake3::Hasher::new();
    for index in 0..workload.files {
        let metadata = std::fs::symlink_metadata(root.join(format!("product/{index:04}")))
            .map_err(|e| e.to_string())?;
        if !metadata.is_file() {
            return Err("materialized entry is not a regular file".into());
        }
        let body =
            std::fs::read(root.join(format!("product/{index:04}"))).map_err(|e| e.to_string())?;
        if body.len() != workload.payload_bytes / workload.files
            || body.iter().any(|byte| *byte != b'x')
        {
            return Err("restored product semantic mismatch".into());
        }
        hash.update(&body);
    }
    Ok(hash.finalize().to_hex().to_string())
}
fn run(request: Request) -> Result<(), String> {
    let workload = &request.workload;
    if request.samples == 0
        || request.samples > 100
        || workload.files == 0
        || workload.files > 512
        || workload.payload_bytes == 0
        || workload.payload_bytes > 262_144
        || workload.payload_bytes % workload.files != 0
        || workload.id.is_empty()
        || workload.symlinks != 0
        || workload.fanout != 0
    {
        return Err("invalid bounds or unsupported hosted symlink/reference cohort".into());
    }
    if request.delay_ms > 1000 || (request.delay_phase.is_none() && request.delay_ms != 0) {
        return Err("invalid delay".into());
    }
    // Validate injection before creating any files.
    capture(
        request
            .delay_phase
            .clone()
            .map(|name| (name, request.delay_ms)),
        || (),
    )?;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&request.root)
        .map_err(|e| format!("new owned root required: {e}"))?;
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    for index in 0..request.samples {
        let sample_root = request.root.join(index.to_string());
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&sample_root)
            .map_err(|e| e.to_string())?;
        let cache_root = sample_root.join("cache");
        let work_root = sample_root.join("work");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&cache_root)
            .map_err(|e| e.to_string())?;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&work_root)
            .map_err(|e| e.to_string())?;
        let maximum_entry_bytes = 1_048_576;
        let config = HostedCacheConfig {
            directory: cache_root,
            maximum_entry_bytes,
            maximum_total_bytes: 4_194_304,
            maximum_entries: 4,
            maximum_age_seconds: 3600,
            maximum_files: 1024,
            policies: vec![Policy {
                organization: "readiness".into(),
                project: "benchmark".into(),
                runner: "local".into(),
                worker: WorkerCacheProfile {
                    producer: "readiness-v1".into(),
                    toolchain: "fixed-bytes-v1".into(),
                    platform: "linux".into(),
                    inputs: vec!["input".into()],
                    paths: vec!["product".into()],
                    maximum_entry_bytes,
                    maximum_files: 1024,
                },
            }],
        };
        config.validate(&work_root)?;
        let mut cold_ok = false;
        for state in ["cold-local-miss", "warm-local-hit"] {
            if state == "warm-local-hit" && !cold_ok {
                break;
            }
            let workspace = work_root.join(state);
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&workspace)
                .map_err(|e| e.to_string())?;
            std::fs::write(
                workspace.join("input"),
                serde_json::to_vec(workload).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let context = Context {
                organization: "readiness",
                project: "benchmark",
                runner: "local",
                plan_identity: "readiness-plan-v1",
                job_identity: "fixed-bytes-v1",
                configuration_identity: "readiness-cache-v1",
                workspace: &workspace,
                now_ms: 1_000,
            };
            let delay = request
                .delay_phase
                .clone()
                .map(|name| (name, request.delay_ms));
            let mut native_restore = None;
            let mut native_publication = None;
            let mut native_key = None;
            let (result, observations) = capture(delay, || -> Result<String, String> {
                let (key, restored) = phase("restore", || cache::restore(&config, &context))?;
                native_key = Some(key.clone());
                let hit = restored.outcome == "hit";
                native_restore = Some(restored);
                if hit != (state == "warm-local-hit") {
                    return Err("cache control outcome mismatch".into());
                }
                phase("setup", || {
                    if hit {
                        Ok(())
                    } else {
                        outputs(&workspace, workload)
                    }
                })?;
                let output_identity = phase("execution", || program(&workspace, workload))?;
                // No child process exists; owned command completion is confirmed before publication.
                native_publication = Some(phase("publication", || {
                    cache::publish(&config, &context, &key)
                })?);
                Ok(output_identity)
            })?;
            cold_ok = result.is_ok();
            let retained_entry_bytes = native_key
                .as_ref()
                .and_then(|key| key.identity().ok())
                .and_then(|id| {
                    std::fs::metadata(config.directory.join("kiln-cache-v1").join(id)).ok()
                })
                .map(|metadata| metadata.len());
            let record = json!({
                "schema": "kiln.readiness.sample.v1", "provider": "hosted-local", "profile": "readiness-cache-v1",
                "workload": workload, "index": index, "cache_state": state,
                "status": if result.is_ok() { "complete" } else { "failed" },
                "output_identity": result.as_ref().ok(), "error": result.as_ref().err(),
                "observations": observations, "native_restore": native_restore,
                "native_publication": native_publication,
                "native_key": native_key,
                "retained_entry_bytes": retained_entry_bytes,
                "retained_format": "native-cache-json-base64",
                "verified_counts": if result.is_ok() { Some(json!({
                    "regular_files": workload.files, "symlinks": 0,
                    "payload_bytes": workload.payload_bytes,
                })) } else { None },
                "delay_phase": request.delay_phase, "delay_ms": request.delay_ms,
                "unavailable": ["remote-object-tier", "download", "decompression", "guest-hydration", "boot"],
                "cpu_scope": "calling-thread", "root": sample_root,
            });
            serde_json::to_writer(&mut stdout, &record).map_err(|e| e.to_string())?;
            writeln!(stdout).map_err(|e| e.to_string())?;
            stdout.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn main() {
    let result = (|| -> Result<(), String> {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(65_537)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 65_536 {
            return Err("request exceeds bound".into());
        }
        run(serde_json::from_slice(&bytes).map_err(|e| e.to_string())?)
    })();
    if let Err(error) = result {
        eprintln!("artifact readiness: {error}");
        std::process::exit(1);
    }
}
