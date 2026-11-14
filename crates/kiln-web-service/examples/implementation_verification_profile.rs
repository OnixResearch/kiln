//! Construct a dedicated qualification CI profile from the complete frozen
//! verification graph. This emits configuration only, never execution evidence.
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use kiln_implementer::verification::VerificationProfile;
use kiln_web_service::engine::Engine;
use kiln_web_service::profile::{MAX_PROFILE_BYTES, ServiceProfile};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const USAGE: &str = "implementation_verification_profile --template DEDICATED_EMPTY_GRAPH_SERVICE_PROFILE --verification-profile FROZEN_PROFILE --project ID --output NEW_SERVICE_PROFILE";
const MAX_VERIFICATION_PROFILE_BYTES: u64 = 16 * 1024 * 1024;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}\n{USAGE}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let mut fields = std::collections::BTreeMap::new();
    while let Some(flag) = arguments.next() {
        if flag == "--help" {
            println!("{USAGE}");
            return Ok(());
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
    let mut service: ServiceProfile = read_json(&template, MAX_PROFILE_BYTES as u64)?;
    let verification: VerificationProfile =
        read_json(&verification_path, MAX_VERIFICATION_PROFILE_BYTES)?;
    if verification.schema != "kiln.verification-profile.v1"
        || verification.checks.is_empty()
        || verification.toolchain != verification.derived_toolchain_identity()?
        || verification.environment_identity != verification.derived_environment_identity()?
    {
        return Err("frozen verification profile/schema/cohort is inconsistent".into());
    }
    let project = service
        .projects
        .iter_mut()
        .find(|project| project.id == project_id)
        .ok_or("selected dedicated project is absent from template")?;
    // Never replace/relabel an existing CI graph with a passing subset. The
    // operator must explicitly supply a new dedicated construction template.
    if !project.workflow.nodes.is_empty() || !project.workflow.edges.is_empty() {
        return Err("refusing to replace an existing CI graph; selected dedicated template nodes/edges must be empty".into());
    }
    let mut nodes =
        Vec::with_capacity(verification.checks.len() + usize::from(verification.cairn.is_some()));
    let mut edges = Vec::new();
    for check in &verification.checks {
        if check.id == "cairn-authoritative-completion" {
            return Err("frozen check collides with reserved native marker".into());
        }
        nodes.push(json!({
            "id":check.id, "kind":check.kind, "effect_class":"test", "adapter_kind":"local",
            "operation_identity":Engine::implementation_verification_operation(&verification, &check.id)?.as_str(),
        }));
        edges.extend(
            check
                .prerequisites
                .iter()
                .map(|prerequisite| json!({"source":prerequisite, "target":check.id})),
        );
    }
    if verification.cairn.is_some() {
        let id = "cairn-authoritative-completion";
        nodes.push(json!({"id":id,"kind":"cairn-agent-completion-v2","effect_class":"read","adapter_kind":"local",
            "operation_identity":Engine::implementation_verification_operation(&verification,id)?.as_str()}));
        edges.extend(
            verification
                .checks
                .iter()
                .map(|check| json!({"source":check.id,"target":id})),
        );
    }
    let mut workflow = serde_json::to_value(&project.workflow).map_err(text)?;
    *workflow.get_mut("nodes").ok_or("workflow nodes missing")? = Value::Array(nodes);
    *workflow.get_mut("edges").ok_or("workflow edges missing")? = Value::Array(edges);
    project.workflow = serde_json::from_value(workflow).map_err(text)?;
    project
        .workflow
        .nodes
        .sort_by(|left, right| left.id.cmp(&right.id));
    project.workflow.edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
    });
    Engine::validate_implementation_verification_project(&verification, project)?;
    service.admit()?;
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
    println!(
        "{}",
        json!({"configuration_path":output,"verification_profile":verification.identity()?,
        "checks_executed":false,"effects_executed":false,"scope":"dedicated-verification-service-configuration"})
    );
    Ok(())
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
