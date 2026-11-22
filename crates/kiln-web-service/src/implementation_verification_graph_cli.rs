//! `kiln-web-service implementation-verification-graph`: construct a dedicated
//! canonical CI service profile from the complete frozen verification graph.
//! This emits configuration only, never execution evidence.
// r[impl kiln.implementation.verification_graph_cli]
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use kiln_implementer::verification::VerificationProfile;
use kiln_web_service::engine::Engine;
use kiln_web_service::profile::{MAX_PROFILE_BYTES, ServiceProfile};
use serde::de::DeserializeOwned;

const USAGE: &str = "usage: kiln-web-service implementation-verification-graph --template DEDICATED_EMPTY_GRAPH_SERVICE_PROFILE --verification-profile FROZEN_PROFILE --project ID --output NEW_SERVICE_PROFILE";
const MAX_VERIFICATION_PROFILE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn run(arguments: impl Iterator<Item = String>) -> i32 {
    match execute(arguments) {
        Ok(None) => {
            println!("{USAGE}");
            0
        }
        Ok(Some(summary)) => {
            println!("{summary}");
            0
        }
        Err(message) => {
            eprintln!("kiln-web-service implementation-verification-graph: {message}\n{USAGE}");
            2
        }
    }
}

fn execute(mut arguments: impl Iterator<Item = String>) -> Result<Option<String>, String> {
    let mut fields = std::collections::BTreeMap::new();
    while let Some(flag) = arguments.next() {
        if flag == "--help" {
            return Ok(None);
        }
        if !matches!(
            flag.as_str(),
            "--template" | "--verification-profile" | "--project" | "--output"
        ) {
            return Err(format!("unknown argument {flag}"));
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        if fields.insert(flag, value).is_some() {
            return Err("duplicate argument".into());
        }
    }
    let value = |key: &str| {
        fields
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| format!("missing {key}"))
    };
    let template = PathBuf::from(value("--template")?);
    let verification_path = PathBuf::from(value("--verification-profile")?);
    let output = PathBuf::from(value("--output")?);
    let project_id = value("--project")?;
    let service: ServiceProfile = read_json(&template, MAX_PROFILE_BYTES as u64)?;
    let verification: VerificationProfile =
        read_json(&verification_path, MAX_VERIFICATION_PROFILE_BYTES)?;
    let service =
        Engine::construct_implementation_verification_service(service, &verification, project_id)?;
    let bytes = serde_json::to_vec_pretty(&service).map_err(text)?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("constructed service profile exceeds native byte bound".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .map_err(text)?;
    file.write_all(&bytes).map_err(text)?;
    file.sync_all().map_err(text)?;
    Ok(Some(
        serde_json::json!({"configuration_path":output,"verification_profile":verification.identity()?,
        "checks_executed":false,"effects_executed":false,"scope":"dedicated-verification-service-configuration"})
        .to_string(),
    ))
}

fn read_json<T: DeserializeOwned>(path: &Path, bound: u64) -> Result<T, String> {
    let metadata = fs::symlink_metadata(path).map_err(text)?;
    if !metadata.is_file() || metadata.len() > bound {
        return Err("configuration is not a bounded regular file".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(path)
        .map_err(text)?
        .take(bound + 1)
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() as u64 > bound {
        return Err("configuration grew beyond native byte bound".into());
    }
    serde_json::from_slice(&bytes).map_err(text)
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
