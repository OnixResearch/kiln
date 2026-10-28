//! Read-only native Onix COB observations, separate from CI state and Cairn reduction.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engine::api::{decoded, error, json_response, query};
use crate::engine::{ApiResponse, Role};

mod store;

const MAX_POLICY_BYTES: usize = 262_144;
const MAX_REPOSITORIES: usize = 64;
const MAX_QUERY_BYTES: usize = 256;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_PAGE_LIMIT: usize = 25;
const MAX_PAGE_LIMIT: usize = 100;
pub(super) const MAX_OBJECTS: usize = 4096;
pub(super) const MAX_REF_TIPS: usize = 128;

const NON_CLAIMS: [&str; 3] = [
    "Raw local operation history is not Cairn-reduced lifecycle state or a semantic conflict verdict.",
    "Signatures, actor authority, remote completeness, synchronization and publication are not verified by this view.",
    "Browsing neither mutates COBs nor grants CI, gate, acceptance or release authority.",
];

/// Types this read-only browser can display.
///
/// The six `xyz.onixresearch.*` types are Cairn decision objects. The three
/// Radicle types are native collaboration objects that Kiln observes without
/// reduction, translation or acceptance meaning.
pub(super) const COB_TYPES: [(&str, &str); 9] = [
    ("xyz.onixresearch.cairn-change", "Cairn changes"),
    ("xyz.onixresearch.rfd", "RFDs"),
    ("xyz.onixresearch.adr", "ADRs"),
    (
        "xyz.onixresearch.verification-obligation",
        "Verification obligations",
    ),
    ("xyz.onixresearch.release-decision", "Release decisions"),
    ("xyz.onixresearch.policy-exception", "Policy exceptions"),
    ("xyz.radicle.issue", "Radicle issues"),
    ("xyz.radicle.patch", "Radicle patches"),
    ("xyz.radworks.job", "Radicle CI jobs"),
];

pub(super) fn supported_type(type_name: &str) -> bool {
    COB_TYPES.iter().any(|(name, _)| *name == type_name)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryPolicy {
    pub id: String,
    pub label: String,
    pub repository_id: String,
    pub git_directory: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub repositories: Vec<RepositoryPolicy>,
}

impl Profile {
    // r[impl kiln.dashboard.cobs.authority]
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != "kiln.web-cobs.v1" || self.repositories.len() > MAX_REPOSITORIES {
            return Err("COB policy schema or repository bound rejected".into());
        }
        let mut ids = BTreeSet::new();
        let mut native_ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for repository in &self.repositories {
            if !crate::profile::identifier(&repository.id)
                || repository.id.len() > 128
                || repository.label.is_empty()
                || repository.label.len() > 128
                || repository.label.chars().any(char::is_control)
                || repository.repository_id.is_empty()
                || repository.repository_id.len() > 256
                || repository.repository_id.chars().any(char::is_control)
                || !crate::profile::absolute(&repository.git_directory)
                || !ids.insert(&repository.id)
                || !native_ids.insert(&repository.repository_id)
                || !paths.insert(&repository.git_directory)
            {
                return Err("COB repository policy rejected".into());
            }
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, MAX_POLICY_BYTES, false)
            .map_err(|_| "COB policy unavailable or unsafe")?;
        let profile: Self = serde_json::from_slice(&bytes).map_err(|_| "COB policy malformed")?;
        profile.admit()?;
        Ok(profile)
    }
}

#[derive(Debug)]
pub struct Browser {
    profile: Option<Profile>,
}

impl Browser {
    pub fn new(profile: Option<Profile>) -> Result<Self, String> {
        if let Some(profile) = &profile {
            profile.admit()?;
        }
        Ok(Self { profile })
    }

    // r[impl kiln.dashboard.cobs.read] r[impl kiln.dashboard.cobs.authority]
    pub fn response(&self, method: &str, target: &str, body: &[u8], role: Role) -> ApiResponse {
        let result = self.read(method, target, body, role);
        match result {
            Ok(response) if response.body.len() <= MAX_RESPONSE_BYTES => response,
            _ => ApiResponse {
                status: 503,
                content_type: "application/json",
                body: br#"{"code":"cob_read_unavailable","message":"COB read exceeded its bounds or is unavailable"}"#.to_vec(),
                download_name: None,
            },
        }
    }

    fn read(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
    ) -> Result<ApiResponse, String> {
        if role == Role::Worker {
            return error(403, "worker_scope");
        }
        if method != "GET" {
            return error(405, "cobs_read_only");
        }
        if !body.is_empty() {
            return error(400, "unexpected_body");
        }
        let (path, parameters) = match query(target) {
            Ok(value) => value,
            Err(()) => return error(400, "query_invalid"),
        };
        let detail = path == "/api/cobs/object";
        if path != "/api/cobs" && !detail {
            return error(404, "route_missing");
        }
        let allowed: &[&str] = if detail {
            &["repository", "type", "object"]
        } else {
            &["repository", "type", "q", "limit", "cursor"]
        };
        if parameters.keys().any(|key| !allowed.contains(key)) {
            return error(400, "query_invalid");
        }
        let mut values = std::collections::BTreeMap::new();
        for (key, value) in parameters {
            match decoded(value) {
                Ok(value)
                    if value.len() <= MAX_QUERY_BYTES && !value.chars().any(char::is_control) =>
                {
                    values.insert(key, value);
                }
                _ => return error(400, "query_invalid"),
            }
        }
        let repository = values.get("repository").map_or("", String::as_str);
        let type_name = values.get("type").map_or("", String::as_str);
        if !type_name.is_empty() && !supported_type(type_name) {
            return error(400, "cob_type_invalid");
        }
        if !repository.is_empty() && !crate::profile::identifier(repository) {
            return error(400, "repository_invalid");
        }
        let Some(profile) = &self.profile else {
            return if detail || !repository.is_empty() {
                error(404, "cob_repository_missing")
            } else {
                json_response(
                    200,
                    json!({"configured":false,"complete":true,"repositories":[],"sources":[],"types":types(),"objects":[],"total":0,"next_cursor":null,"non_claims":NON_CLAIMS}),
                )
            };
        };
        if !repository.is_empty() && !profile.repositories.iter().any(|row| row.id == repository) {
            return error(404, "cob_repository_missing");
        }
        if detail {
            let object = values.get("object").map_or("", String::as_str);
            if repository.is_empty() || type_name.is_empty() || !store::is_oid(object) {
                return error(400, "cob_selection_invalid");
            }
            let policy = profile
                .repositories
                .iter()
                .find(|row| row.id == repository)
                .ok_or("admitted COB repository missing")?;
            return match store::detail(policy, type_name, object) {
                Ok(Some(value)) => {
                    json_response(200, json!({"object":value,"non_claims":NON_CLAIMS}))
                }
                Ok(None) => error(404, "cob_object_missing"),
                Err(_) => error(503, "cob_store_unavailable"),
            };
        }
        let limit = match values.get("limit") {
            None => DEFAULT_PAGE_LIMIT,
            Some(value) => match value.parse::<usize>() {
                Ok(limit) if (1..=MAX_PAGE_LIMIT).contains(&limit) => limit,
                _ => return error(400, "page_limit_invalid"),
            },
        };
        let search = values.get("q").map_or("", String::as_str);
        let mut objects = Vec::new();
        let mut repositories = Vec::new();
        let mut complete = true;
        for policy in &profile.repositories {
            if !repository.is_empty() && policy.id != repository {
                continue;
            }
            match store::inventory(policy) {
                Ok(mut rows) => {
                    repositories.push(json!({"id":policy.id,"label":policy.label,"repository_id":policy.repository_id,"status":"available","objects":rows.len()}));
                    objects.append(&mut rows);
                }
                Err(_) => {
                    complete = false;
                    repositories.push(json!({"id":policy.id,"label":policy.label,"repository_id":policy.repository_id,"status":"unavailable","objects":null}));
                }
            }
            if objects.len() > MAX_OBJECTS {
                return error(503, "cob_inventory_bound");
            }
        }
        objects.sort_by(|left, right| {
            (&left.repository, &left.type_name, &left.object_id).cmp(&(
                &right.repository,
                &right.type_name,
                &right.object_id,
            ))
        });
        let snapshot = blake3::hash(
            &serde_json::to_vec(&(
                repository,
                type_name,
                search,
                limit,
                &objects,
                &repositories,
            ))
            .map_err(|_| "COB snapshot encoding failed")?,
        )
        .to_hex()
        .to_string();
        let needle = search.to_lowercase();
        let filtered: Vec<_> = objects
            .into_iter()
            .filter(|row| {
                (type_name.is_empty() || row.type_name == type_name)
                    && (needle.is_empty()
                        || row
                            .root_summary
                            .as_ref()
                            .is_some_and(|summary| summary.to_lowercase().contains(&needle))
                        || [
                            &row.title,
                            &row.object_id,
                            &row.repository,
                            &row.repository_label,
                            &row.repository_id,
                            &row.type_name,
                            &row.author,
                        ]
                        .iter()
                        .any(|value| value.to_lowercase().contains(&needle)))
            })
            .collect();
        let offset = match values.get("cursor") {
            None => 0,
            Some(cursor) => match cursor.split_once(':') {
                Some((identity, offset)) if identity == snapshot => match offset.parse::<usize>() {
                    Ok(offset) if offset > 0 && offset < filtered.len() && offset % limit == 0 => {
                        offset
                    }
                    _ => return error(400, "cursor_invalid"),
                },
                _ => return error(409, "cob_snapshot_changed"),
            },
        };
        let next = offset.saturating_add(limit);
        let next_cursor = (next < filtered.len()).then(|| format!("{snapshot}:{next}"));
        let page = filtered
            .get(offset..next.min(filtered.len()))
            .ok_or("COB page range rejected")?;
        let available = profile.repositories.iter().map(|policy| json!({"id":policy.id,"label":policy.label,"repository_id":policy.repository_id})).collect::<Vec<_>>();
        json_response(
            200,
            json!({"configured":true,"complete":complete,"repositories":available,"sources":repositories,"types":types(),"objects":page,"total":filtered.len(),"next_cursor":next_cursor,"snapshot":snapshot,"non_claims":NON_CLAIMS}),
        )
    }
}

fn types() -> Vec<Value> {
    COB_TYPES
        .iter()
        .map(|(type_name, label)| json!({"id":type_name,"label":label}))
        .collect()
}
