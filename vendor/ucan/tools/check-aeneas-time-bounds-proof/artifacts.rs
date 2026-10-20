use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

use crate::error::CheckerError;
use crate::model;
use crate::model::Observation;

const SOURCE_PATH: &str = "crates/ucan-core/src/token.rs";
const CORE_MANIFEST_PATH: &str = "crates/ucan-core/Cargo.toml";
const CORE_LIB_PATH: &str = "crates/ucan-core/src/lib.rs";
const PROVIDER_SOURCE_PATH: &str = "proofs/aeneas-time-bounds/provider-identity.ncl";
const MANIFEST_SOURCE_PATH: &str = "proofs/aeneas-time-bounds/manifest.ncl";
const RECEIPT_SOURCE_PATH: &str = "proofs/aeneas-time-bounds/blocked-receipt.ncl";
const PROVIDER_JSON_PATH: &str = "proofs/aeneas-time-bounds/generated/provider-identity.json";
const MANIFEST_JSON_PATH: &str = "proofs/aeneas-time-bounds/generated/manifest.json";
const RECEIPT_JSON_PATH: &str = "proofs/aeneas-time-bounds/generated/blocked-receipt.json";
const DIAGNOSTIC_PATH: &str = "proofs/aeneas-time-bounds/evidence/exact-item-selection.log";
const SUPPLEMENTARY_PATH: &str = "proofs/aeneas-time-bounds/evidence/broad-public-probe.log";
const DRIFT_SOURCE_PATH: &str = "proofs/aeneas-time-bounds/fixtures/invalid/source-drift.rs";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckStats {
    pub selected_items: usize,
    pub negative_cases: usize,
    pub generated_artifacts: usize,
}

pub fn check(root: &Path, nickel: &Path) -> Result<CheckStats, CheckerError> {
    let generated_artifacts = [
        (PROVIDER_SOURCE_PATH, PROVIDER_JSON_PATH),
        (MANIFEST_SOURCE_PATH, MANIFEST_JSON_PATH),
        (RECEIPT_SOURCE_PATH, RECEIPT_JSON_PATH),
    ];
    for (source, generated) in generated_artifacts {
        check_generated(root, nickel, source, generated)?;
    }
    check_runtime_boundary(root)?;
    let manifest_bytes = read_required(root, MANIFEST_JSON_PATH)?;
    let receipt_bytes = read_required(root, RECEIPT_JSON_PATH)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| CheckerError::new("manifest-json", error.to_string()))?;
    let receipt: Value =
        serde_json::from_slice(&receipt_bytes).map_err(|error| CheckerError::new("receipt-json", error.to_string()))?;
    let observation = observe(root)?;
    let issues = model::validate(&manifest, &receipt, &observation);
    if !issues.is_empty() {
        let message = issues
            .iter()
            .map(|issue| format!("{}:{}:{}", issue.code, issue.path, issue.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(CheckerError::new("evidence", message));
    }
    let selected_items = manifest.pointer("/target/items").and_then(Value::as_array).map_or(0, Vec::len);
    let negative_cases = manifest.pointer("/negative_cases").and_then(Value::as_array).map_or(0, Vec::len);
    Ok(CheckStats {
        selected_items,
        negative_cases,
        generated_artifacts: generated_artifacts.len(),
    })
}

fn observe(root: &Path) -> Result<Observation, CheckerError> {
    let source = read_required(root, SOURCE_PATH)?;
    let source_text =
        String::from_utf8(source.clone()).map_err(|error| CheckerError::new("source-utf8", error.to_string()))?;
    Ok(Observation {
        source_text,
        source_blake3: model::digest(&source),
        provider_source_blake3: model::digest(&read_required(root, PROVIDER_SOURCE_PATH)?),
        manifest_source_blake3: model::digest(&read_required(root, MANIFEST_SOURCE_PATH)?),
        diagnostic_blake3: model::digest(&read_required(root, DIAGNOSTIC_PATH)?),
        supplementary_blake3: model::digest(&read_required(root, SUPPLEMENTARY_PATH)?),
        drift_source_blake3: model::digest(&read_required(root, DRIFT_SOURCE_PATH)?),
    })
}

fn check_generated(root: &Path, nickel: &Path, source: &str, generated: &str) -> Result<(), CheckerError> {
    let source_path = confined_path(root, source)?;
    let output = Command::new(nickel)
        .current_dir(root)
        .args(["export", "--format", "json"])
        .arg(&source_path)
        .output()
        .map_err(|error| CheckerError::new("nickel-exec", error.to_string()))?;
    if !output.status.success() {
        return Err(CheckerError::new(
            "nickel-export",
            format!("{source}: {}", String::from_utf8_lossy(&output.stderr)),
        ));
    }
    let expected = read_required(root, generated)?;
    if output.stdout != expected {
        return Err(CheckerError::new(
            "generated-freshness",
            format!("{generated} does not match Nickel export of {source}"),
        ));
    }
    Ok(())
}

fn check_runtime_boundary(root: &Path) -> Result<(), CheckerError> {
    let core_manifest = String::from_utf8(read_required(root, CORE_MANIFEST_PATH)?)
        .map_err(|error| CheckerError::new("core-manifest-utf8", error.to_string()))?;
    let core_lib = String::from_utf8(read_required(root, CORE_LIB_PATH)?)
        .map_err(|error| CheckerError::new("core-lib-utf8", error.to_string()))?;
    if !core_lib.lines().any(|line| line.trim() == "#![no_std]") {
        return Err(CheckerError::new("runtime-boundary", "ucan-core no longer declares #![no_std]"));
    }
    for forbidden in ["aeneas", "lean", "nickel"] {
        if core_manifest.to_ascii_lowercase().contains(forbidden) {
            return Err(CheckerError::new(
                "runtime-boundary",
                format!("ucan-core manifest contains proof-tool dependency marker {forbidden:?}"),
            ));
        }
    }
    Ok(())
}

fn read_required(root: &Path, relative: &str) -> Result<Vec<u8>, CheckerError> {
    let path = confined_path(root, relative)?;
    fs::read(&path).map_err(|error| CheckerError::new("artifact-read", format!("{}: {error}", path.display())))
}

fn confined_path(root: &Path, relative: &str) -> Result<PathBuf, CheckerError> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path.components().any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(CheckerError::new("artifact-path", format!("unconfined path {relative:?}")));
    }
    Ok(root.join(relative_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_confined_paths() {
        let root = Path::new("/repo");
        assert_eq!(
            confined_path(root, MANIFEST_SOURCE_PATH).expect("path should be confined"),
            root.join(MANIFEST_SOURCE_PATH)
        );
    }

    #[test]
    fn rejects_parent_and_absolute_paths() {
        assert!(confined_path(Path::new("/repo"), "../outside").is_err());
        assert!(confined_path(Path::new("/repo"), "/outside").is_err());
    }
}
