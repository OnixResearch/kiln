//! Bounded, read-only collection of Cairn's native campaign projections.
use std::{ffi::OsString, path::PathBuf};

use blake3::Hasher;
use kiln_core::types::ContentId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CairnProcessOutput, CairnRuntime, ProcessInvocation};
use bounded_exec::ExecutionLimits;

pub const MAX_CAMPAIGN_CAIRN_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const TIMEOUT_MS: u64 = 30_000;
const STDOUT_BYTES: usize = MAX_CAMPAIGN_CAIRN_OUTPUT_BYTES;
const STDERR_BYTES: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnCampaignProfile {
    pub executable: PathBuf,
    pub repository_root: PathBuf,
    pub policy_path: PathBuf,
    pub executable_identity: ContentId,
    pub policy_identity: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnCampaignSnapshot {
    pub change: String,
    pub executable_identity: ContentId,
    pub policy_identity: ContentId,
    pub graph: Value,
    pub workflow_status: Value,
    pub workflow_instructions: std::collections::BTreeMap<String, Value>,
    pub readiness: Value,
    pub implementation_ready: Value,
    pub context_identity: ContentId,
}

#[derive(Debug)]
pub struct CairnCampaignReader<R> {
    profile: CairnCampaignProfile,
    runtime: R,
    implementation_ready: Option<Value>,
}
impl<R: CairnRuntime> CairnCampaignReader<R> {
    pub fn new(profile: CairnCampaignProfile, runtime: R) -> Result<Self, String> {
        if !profile.executable.is_absolute()
            || !profile.executable.starts_with("/nix/store")
            || profile.executable.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
            || !profile.repository_root.is_absolute()
            || !profile.policy_path.is_absolute()
            || profile.executable_identity.validate().is_err()
            || profile.policy_identity.validate().is_err()
        {
            return Err("Cairn campaign requires an explicitly selected immutable executable and exact policy identity".into());
        }
        Ok(Self {
            profile,
            runtime,
            implementation_ready: None,
        })
    }

    /// Read the authoritative graph, policy-selected workflow, per-artifact
    /// instructions, and seven-stage readiness; no lifecycle mutation route is
    /// invoked. Any partial, stale-profile, or non-native response is rejected.
    pub fn collect(&mut self, change: &str) -> Result<CairnCampaignSnapshot, String> {
        if change.is_empty()
            || change.len() > 128
            || !change
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err("Cairn change selector is not a bounded native identifier".into());
        }
        if self
            .runtime
            .executable_identity(&self.profile.executable, 256 * 1024 * 1024)
            .map_err(port_text)?
            != self.profile.executable_identity
        {
            return Err("Cairn campaign executable identity changed".into());
        }
        let policy_hash = self.profile.policy_identity.as_str();
        let graph = Self::command(
            &self.profile,
            &mut self.runtime,
            &[
                "change",
                "graph",
                "--root",
                path_string(&self.profile.repository_root)?,
                "--format",
                "json",
            ],
            |v| {
                v.get("command").and_then(Value::as_str) == Some("change graph")
                    && v.get("valid").and_then(Value::as_bool) == Some(true)
            },
        )?;
        let workflow_status = Self::command(
            &self.profile,
            &mut self.runtime,
            &[
                "workflow",
                "status",
                change,
                "--root",
                path_string(&self.profile.repository_root)?,
                "--policy",
                path_string(&self.profile.policy_path)?,
            ],
            |v| {
                v.get("command").and_then(Value::as_str) == Some("workflow status")
                    && v.get("change").and_then(Value::as_str) == Some(change)
                    && v.get("policy_hash").and_then(Value::as_str) == Some(policy_hash)
                    && v.get("read_only").and_then(Value::as_bool) == Some(true)
            },
        )?;
        let artifact_ids = workflow_status
            .get("artifacts")
            .and_then(Value::as_array)
            .ok_or("Cairn workflow status omitted artifacts")?;
        if artifact_ids.is_empty() || artifact_ids.len() > 128 {
            return Err("Cairn workflow artifact count is empty or exceeds its bound".into());
        }
        let mut workflow_instructions = std::collections::BTreeMap::new();
        for artifact_value in artifact_ids {
            let artifact = artifact_value
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| {
                    !id.is_empty()
                        && id.len() <= 128
                        && id
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                })
                .ok_or("Cairn workflow artifact identity is malformed")?;
            if workflow_instructions.contains_key(artifact) {
                return Err("Cairn workflow status duplicates an artifact identity".into());
            }
            let result = Self::command(
                &self.profile,
                &mut self.runtime,
                &[
                    "workflow",
                    "instructions",
                    artifact,
                    "--change",
                    change,
                    "--root",
                    path_string(&self.profile.repository_root)?,
                    "--policy",
                    path_string(&self.profile.policy_path)?,
                ],
                |v| {
                    v.get("command").and_then(Value::as_str) == Some("workflow instructions")
                        && v.get("change").and_then(Value::as_str) == Some(change)
                        && v.get("policy_hash").and_then(Value::as_str) == Some(policy_hash)
                        && v.get("read_only").and_then(Value::as_bool) == Some(true)
                        && v.get("artifact")
                            .and_then(|a| a.get("artifact_id"))
                            .and_then(Value::as_str)
                            == Some(artifact)
                },
            )?;
            workflow_instructions.insert(artifact.to_owned(), result);
        }
        let readiness = Self::command(
            &self.profile,
            &mut self.runtime,
            &[
                "readiness",
                change,
                "--root",
                path_string(&self.profile.repository_root)?,
                "--policy",
                path_string(&self.profile.policy_path)?,
            ],
            |v| {
                v.get("schema").and_then(Value::as_str) == Some("cairn.readiness.v1")
                    && v.get("scope")
                        .and_then(|s| s.get("change"))
                        .and_then(Value::as_str)
                        == Some(change)
                    && v.get("scope")
                        .and_then(|s| s.get("policy_hash"))
                        .and_then(Value::as_str)
                        == Some(policy_hash)
                    && v.get("mutation_authorized").and_then(Value::as_bool) == Some(false)
            },
        )?;
        let implementation_ready = if let Some(snapshot) = &self.implementation_ready {
            snapshot.clone()
        } else {
            let snapshot = Self::command(
                &self.profile,
                &mut self.runtime,
                &[
                    "implementation-ready",
                    "--root",
                    path_string(&self.profile.repository_root)?,
                    "--policy",
                    path_string(&self.profile.policy_path)?,
                    "--limit",
                    "1000",
                ],
                |v| {
                    v.get("command").and_then(Value::as_str) == Some("cairn implementation-ready")
                        && v.get("schema").and_then(Value::as_str)
                            == Some("cairn.implementation-ready.v1")
                        && v.get("complete").and_then(Value::as_bool).is_some()
                        && v.get("selection").and_then(Value::as_array).is_some()
                        && v.get("changes").and_then(Value::as_array).is_some()
                        && v.get("mcp")
                            .and_then(|m| m.get("read_only"))
                            .and_then(Value::as_bool)
                            == Some(true)
                        && v.get("mcp")
                            .and_then(|m| m.get("destructive"))
                            .and_then(Value::as_bool)
                            == Some(false)
                },
            )?;
            self.implementation_ready = Some(snapshot.clone());
            snapshot
        };
        let context_identity = hash(
            &serde_json::to_vec(&(
                change,
                &self.profile.executable_identity,
                &self.profile.policy_identity,
                &graph,
                &workflow_status,
                &workflow_instructions,
                &readiness,
                &implementation_ready,
            ))
            .map_err(text)?,
        )?;
        Ok(CairnCampaignSnapshot {
            change: change.to_owned(),
            executable_identity: self.profile.executable_identity.clone(),
            policy_identity: self.profile.policy_identity.clone(),
            graph,
            workflow_status,
            workflow_instructions,
            readiness,
            implementation_ready,
            context_identity,
        })
    }

    fn command(
        profile: &CairnCampaignProfile,
        runtime: &mut R,
        args: &[&str],
        validate: impl FnOnce(&Value) -> bool,
    ) -> Result<Value, String> {
        let invocation = ProcessInvocation {
            program: profile.executable.clone(),
            args: args.iter().map(OsString::from).collect(),
            current_dir: profile.repository_root.clone(),
            environment: Vec::new(),
            limits: ExecutionLimits {
                timeout_ms: TIMEOUT_MS,
                stdin_max_bytes: 0,
                stdout_max_bytes: STDOUT_BYTES,
                stderr_max_bytes: STDERR_BYTES,
                poll_interval_ms: 25,
                teardown_timeout_ms: 1_000,
            },
        };
        let output = runtime.run(&invocation).map_err(port_text)?;
        let CairnProcessOutput::Exited {
            process_status: Some(0),
            stdout,
            ..
        } = output
        else {
            return Err("Cairn campaign read failed, timed out, or has an unknown outcome".into());
        };
        if stdout.len() > MAX_CAMPAIGN_CAIRN_OUTPUT_BYTES {
            return Err("Cairn campaign result exceeds bound".into());
        }
        let value: Value = serde_json::from_slice(&stdout).map_err(text)?;
        if !validate(&value) {
            return Err(
                "Cairn campaign response is not a valid native read-only observation".into(),
            );
        }
        Ok(value)
    }
}

fn path_string(path: &std::path::Path) -> Result<&str, String> {
    path.to_str()
        .filter(|s| !s.is_empty() && !s.contains('\0'))
        .ok_or_else(|| "Cairn campaign path is not valid UTF-8".into())
}
fn id(value: &str) -> Result<ContentId, String> {
    ContentId::from_blake3_hex(value).map_err(|error| format!("{}: {}", error.code, error.message))
}
fn hash(bytes: &[u8]) -> Result<ContentId, String> {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    id(hasher.finalize().to_hex().as_str())
}
fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn port_text(error: kiln_provider_api::PortFailure) -> String {
    format!("Cairn campaign runtime: {error:?}")
}
