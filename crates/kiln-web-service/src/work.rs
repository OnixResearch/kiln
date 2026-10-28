//! Authenticated application work, persisted as service-signed native COBs.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::engine::api::{error, json_response};
use crate::engine::{ApiResponse, Role};

pub mod model;
mod store;

const MAX_PROFILE_BYTES: usize = 262_144;
const MAX_COMMAND_BYTES: usize = 262_144;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_PRINCIPALS: usize = 128;
const NON_CLAIMS: [&str; 2] = [
    "Native signatures attest the service-recorded authenticated application principal, not that user's Radicle key.",
    "Local durable work operations grant no CI, provider, remote publication or release authority.",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub git_directory: PathBuf,
    /// Raw 32-byte Ed25519 seed, admitted through the existing private-file policy.
    pub signer_seed_file: PathBuf,
    pub write_principals: Vec<String>,
}

impl Profile {
    pub fn admit(&self) -> Result<(), String> {
        let mut principals = BTreeSet::new();
        if self.schema != "kiln.web-work.v1"
            || !crate::profile::absolute(&self.git_directory)
            || !crate::profile::absolute(&self.signer_seed_file)
            || self.write_principals.is_empty()
            || self.write_principals.len() > MAX_PRINCIPALS
            || self
                .write_principals
                .iter()
                .any(|principal| !valid_principal(principal) || !principals.insert(principal))
        {
            return Err("work profile schema, paths or principals rejected".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, MAX_PROFILE_BYTES, false)
            .map_err(|_| "work profile unavailable or unsafe")?;
        let profile: Self = serde_json::from_slice(&bytes).map_err(|_| "work profile malformed")?;
        profile.admit()?;
        Ok(profile)
    }
}

fn valid_principal(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
        && !value.contains('*')
}

/// No repository handle or cached projection outlives a request. Every read
/// revalidates native signatures and reconstructs state from durable history.
#[derive(Debug)]
pub struct Workspace {
    profile: Option<Profile>,
    gate: Mutex<()>,
}

impl Workspace {
    pub fn new(profile: Option<Profile>) -> Result<Self, String> {
        if let Some(profile) = &profile {
            profile.admit()?;
            store::signer(profile)?;
        }
        Ok(Self {
            profile,
            gate: Mutex::new(()),
        })
    }

    pub fn response(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
        principal: &str,
        now_ms: i64,
    ) -> ApiResponse {
        match self.respond(method, target, body, role, principal, now_ms) {
            Ok(response) if response.body.len() <= MAX_RESPONSE_BYTES => response,
            _ => unavailable(),
        }
    }

    fn respond(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
        principal: &str,
        now_ms: i64,
    ) -> Result<ApiResponse, String> {
        if role == Role::Worker {
            return error(403, "worker_scope");
        }
        if !valid_principal(principal) {
            return error(403, "work_principal_denied");
        }
        let detail = target.strip_prefix("/api/work/objects/");
        let listing = target == "/api/work";
        let command_route = target == "/api/work/commands";
        if !listing && !command_route && detail.is_none() {
            return error(404, "route_missing");
        }
        if (command_route && method != "POST") || (!command_route && method != "GET") {
            return error(405, "work_method_denied");
        }
        if detail.is_some_and(|id| !store::is_oid(id)) {
            return error(404, "work_object_missing");
        }
        if !command_route && !body.is_empty() {
            return error(400, "unexpected_body");
        }
        let can_write = self.profile.as_ref().is_some_and(|profile| {
            profile
                .write_principals
                .iter()
                .any(|allowed| allowed == principal)
        });
        if command_route && !can_write {
            return error(403, "work_write_denied");
        }
        let Some(profile) = &self.profile else {
            return if listing {
                json_response(
                    200,
                    json!({"schema":"kiln.work.v1","configured":false,
                    "principal":principal,"can_write":false,"objects":[],"bounded":true,"non_claims":NON_CLAIMS}),
                )
            } else {
                error(404, "work_object_missing")
            };
        };
        let command = if command_route {
            if body.len() > MAX_COMMAND_BYTES || now_ms < 0 {
                return error(400, "work_command_invalid");
            }
            match serde_json::from_slice::<model::Command>(body) {
                Ok(value) => Some(value),
                Err(_) => return error(400, "work_command_invalid"),
            }
        } else {
            None
        };
        let _gate = self.gate.lock().map_err(|_| "work lock unavailable")?;
        match store::execute(profile, command, principal, now_ms) {
            Ok(result) => {
                if let Some((object, replayed)) = result.written {
                    return json_response(200, json!({"object":object,"replayed":replayed}));
                }
                if let Some(id) = detail {
                    return match result
                        .records
                        .get(id)
                        .filter(|row| model::visible(row, principal))
                    {
                        Some(record) => json_response(200, record),
                        None => error(404, "work_object_missing"),
                    };
                }
                let objects: Vec<_> = result
                    .records
                    .values()
                    .filter(|row| model::visible(row, principal))
                    .collect();
                json_response(
                    200,
                    json!({"schema":"kiln.work.v1","configured":true,
                    "principal":principal,"can_write":can_write,"objects":objects,"bounded":true,"non_claims":NON_CLAIMS}),
                )
            }
            Err(store::Failure::Missing) => error(404, "work_object_missing"),
            Err(store::Failure::Invalid) => error(400, "work_command_invalid"),
            Err(store::Failure::Conflict) => error(409, "work_head_conflict"),
            Err(store::Failure::Replay) => error(409, "work_request_conflict"),
            Err(store::Failure::Unavailable) => error(503, "work_store_unavailable"),
        }
    }
}

fn unavailable() -> ApiResponse {
    ApiResponse {
        status: 503,
        content_type: "application/json",
        body: br#"{"code":"work_store_unavailable","message":"Work storage is unavailable or exceeded its admitted bounds"}"#.to_vec(),
        download_name: None,
    }
}
