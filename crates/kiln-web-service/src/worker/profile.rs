use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use kiln_core::source::GitSnapshotLimits;
use serde::{Deserialize, Serialize};

pub const MAX_PROFILE_BYTES: usize = 1_048_576;
const MAX_STEPS: usize = 128;
const MAX_DECLARATIONS: usize = 4096;
const MAX_TEXT_BYTES: usize = 65_536;
const MAX_SNAPSHOT_BYTES: usize = 1_073_741_824;
const MAX_OUTPUT_BYTES: usize = 16_777_216;
const MAX_ARTIFACT_BYTES: usize = 67_108_864;
const MAX_TIMEOUT_MS: u64 = 86_400_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerProfile {
    pub schema: String,
    pub project_id: String,
    pub source_repository: String,
    pub repository_path: PathBuf,
    pub service_origin: String,
    pub token_file: PathBuf,
    pub tls_ca_path: PathBuf,
    pub work_root: PathBuf,
    #[serde(deserialize_with = "snapshot_limits")]
    pub snapshot_limits: GitSnapshotLimits,
    pub command_timeout_ms: u64,
    pub maximum_output_bytes: usize,
    pub teardown_timeout_ms: u64,
    pub poll_interval_ms: u64,
    pub http_timeout_ms: u64,
    pub lookup_timeout_ms: u64,
    pub maximum_response_bytes: usize,
    pub maximum_artifact_bytes: usize,
    pub maximum_total_artifact_bytes: usize,
    pub maximum_artifacts: usize,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub node_id: String,
    pub program: PathBuf,
    pub argv: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub artifacts: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reports: Vec<crate::reports::ReportDeclaration>,
}

pub fn component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

pub fn relative_file(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .to_str()
            .is_some_and(|s| s.len() <= 4096 && !s.contains(['\\', '\0']))
        && path.components().all(|c| matches!(c, Component::Normal(_)))
}

pub fn hex(value: &str, bytes: usize) -> bool {
    value.len() == bytes * 2
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn environment_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    !name.is_empty()
        && name.len() <= 256
        && name
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i != 0 && b.is_ascii_digit()))
        && !["LATTICE_", "GIT_", "KILN_", "SECRETSPEC_"]
            .iter()
            .any(|s| upper.starts_with(s))
        && !["TOKEN", "SECRET", "PASSWORD", "CREDENTIAL"]
            .iter()
            .any(|s| upper.contains(s))
}

impl WorkerProfile {
    // r[impl kiln.web.remote_workers.source]
    // r[impl kiln.web.remote_workers.source.scenario.denied]
    pub fn admit(&self) -> Result<(), String> {
        let url = reqwest::Url::parse(&self.service_origin).map_err(|_| "worker origin invalid")?;
        let limits = &self.snapshot_limits;
        let absolute = [
            &self.repository_path,
            &self.token_file,
            &self.tls_ca_path,
            &self.work_root,
        ];
        if self.schema != "kiln.web-worker.v2"
            || !component(&self.project_id)
            || !hex(&self.source_repository, 32)
            || absolute.iter().any(|p| {
                !p.is_absolute()
                    || p.components()
                        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            })
            || url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || self.service_origin.ends_with('/')
            || self.token_file.starts_with(&self.repository_path)
            || self.token_file.starts_with(&self.work_root)
            || self.work_root.starts_with(&self.repository_path)
            || limits.max_entries == 0
            || limits.max_entries > MAX_DECLARATIONS * MAX_STEPS
            || limits.max_path_bytes == 0
            || limits.max_path_bytes > 4096
            || limits.max_file_bytes == 0
            || limits.max_file_bytes > limits.max_total_bytes
            || limits.max_total_bytes == 0
            || limits.max_total_bytes > MAX_SNAPSHOT_BYTES
            || [
                self.command_timeout_ms,
                self.teardown_timeout_ms,
                self.poll_interval_ms,
                self.http_timeout_ms,
                self.lookup_timeout_ms,
            ]
            .iter()
            .any(|n| *n == 0 || *n > MAX_TIMEOUT_MS)
            || self.maximum_output_bytes == 0
            || self.maximum_output_bytes > MAX_OUTPUT_BYTES
            || self.maximum_response_bytes == 0
            || self.maximum_response_bytes > MAX_PROFILE_BYTES
            || self.maximum_artifact_bytes == 0
            || self.maximum_artifact_bytes > MAX_ARTIFACT_BYTES
            || self.maximum_total_artifact_bytes < self.maximum_artifact_bytes
            || self.maximum_total_artifact_bytes > MAX_SNAPSHOT_BYTES
            || self.maximum_artifacts == 0
            || self.maximum_artifacts > MAX_DECLARATIONS
            || self.steps.is_empty()
            || self.steps.len() > MAX_STEPS
        {
            return Err("worker profile outside admitted bounds".into());
        }
        let mut artifacts = 0usize;
        let mut artifact_names = std::collections::BTreeSet::new();
        let mut reports = 0usize;
        let mut report_names = std::collections::BTreeSet::new();
        for step in &self.steps {
            reports = reports
                .checked_add(step.reports.len())
                .ok_or("report count overflow")?;
            if reports > crate::reports::MAX_REPORTS {
                return Err("worker report count exceeds bound".into());
            }
            for report in &step.reports {
                if !crate::reports::safe_relative_path(&report.path)
                    || !report_names.insert((step.node_id.clone(), report.path.clone()))
                {
                    return Err("worker report paths must be safe and unique per node".into());
                }
            }
            artifacts = artifacts
                .checked_add(step.artifacts.len())
                .ok_or("artifact count overflow")?;
            if !component(&step.node_id)
                || !step.program.is_absolute()
                || step.program.starts_with(&self.repository_path)
                || step.program.starts_with(&self.work_root)
                || step
                    .program
                    .to_str()
                    .is_none_or(|s| s.len() > MAX_TEXT_BYTES || s.contains('\0'))
                || step.argv.len() > MAX_DECLARATIONS
                || step
                    .argv
                    .iter()
                    .any(|s| s.len() > MAX_TEXT_BYTES || s.contains('\0'))
                || step.environment.len() > MAX_DECLARATIONS
                || step.environment.iter().any(|(k, v)| {
                    !environment_name(k) || v.len() > MAX_TEXT_BYTES || v.contains('\0')
                })
                || step.artifacts.iter().any(|p| !relative_file(p))
            {
                return Err("worker step declaration invalid".into());
            }
            for path in &step.artifacts {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or("worker artifact name invalid")?;
                if !component(name) || !artifact_names.insert(name) {
                    return Err("worker artifact names must be safe and unique".into());
                }
            }
        }
        if artifacts > self.maximum_artifacts {
            return Err("worker artifact count exceeds bound".into());
        }
        Ok(())
    }
}

fn snapshot_limits<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<GitSnapshotLimits, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Limits {
        max_entries: usize,
        max_path_bytes: usize,
        max_file_bytes: usize,
        max_total_bytes: usize,
    }
    let value = Limits::deserialize(deserializer)?;
    Ok(GitSnapshotLimits {
        max_entries: value.max_entries,
        max_path_bytes: value.max_path_bytes,
        max_file_bytes: value.max_file_bytes,
        max_total_bytes: value.max_total_bytes,
    })
}
