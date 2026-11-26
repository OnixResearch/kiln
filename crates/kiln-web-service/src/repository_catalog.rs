//! Durable operator-guarded repository catalog and read-only local/fleet aggregator.
//!
//! Persists imported remote/mirror registrations atomically in
//! `<state_directory>/repositories.json` with `request_id` replay/conflict
//! protection, observes coordinator-local checkouts (`git`, `jj`, `pijul`, and
//! `.cairn/changes` counts) strictly read-only, fetches proxied Fleet local
//! copies when configured, and reconciles everything via
//! [`kiln_core::repository_catalog::reconcile_catalog`].
//!
//! Radicle RIDs are recorded strictly as read-only declarations; this module
//! never invokes Radicle init, signing, push, sync, or mirror publication.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use git2::{Repository, RepositoryOpenFlags};
use kiln_core::repository_catalog::{
    CopyHealth, FleetLocalCopy, FleetSyncStatus, LocalVcs, MAX_CATALOG_REPOS, MAX_MACHINE_BYTES,
    MAX_PATH_BYTES, MAX_SLUG_BYTES, RemoteEndpoint, RemoteProvider, RemoteRole,
    RepositoryAdmission, RepositoryRecord, attach_remote_to_record, build_repository_record,
    default_non_claims, normalize_remote, reconcile_catalog, validate_local_copy,
};
use kiln_core::types::ContentId;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use nix::unistd::geteuid;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::engine::{ApiResponse, Role};
use crate::profile::{ProjectProfile, ServiceProfile};

/// Maximum number of coordinator-local checkouts inspected per census.
pub const MAX_LOCAL_SCAN: usize = 128;
const MAX_SCAN_DEPTH: usize = 2;
const MAX_SCAN_ENTRIES_PER_DIR: usize = 256;
const MAX_REMOTES_PER_COPY: usize = 16;
const MAX_CAIRN_COUNT_SCAN: u32 = 10_000;
const MAX_STORE_BYTES: usize = 4 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_REQUEST_LEDGER: usize = 512;
const MAX_REASON_BYTES: usize = 1024;
const MAX_ERROR_BYTES: usize = 240;

const CATALOG_FILENAME: &str = "repositories.json";
const STORE_SCHEMA: &str = "kiln.repository-catalog-store.v1";
const REPOSITORIES_SCHEMA: &str = "kiln.repositories.v1";
const MUTATION_SCHEMA: &str = "kiln.repository-mutation.v1";
const COMMIT_HEX_MIN: usize = 40;
const COMMIT_HEX_MAX: usize = 64;

const CATALOG_NON_CLAIMS: [&str; 3] = [
    "Catalog registration does not grant CI workflow execution or Fleet agent dispatch.",
    "Observed remotes and local copies are read-only snapshots and do not prove upstream reachability or publication.",
    "Radicle RIDs are recorded as read-only declarations; Kiln performs no Radicle initialization, signing, push, or mirror publication.",
];

/// Configured `sources.json` entry projected into the catalog observer.
#[derive(Clone, Debug)]
pub struct ConfiguredSource {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub path: PathBuf,
    pub executable: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct ConfiguredProject {
    repository_path: PathBuf,
}

impl From<&ProjectProfile> for ConfiguredProject {
    fn from(project: &ProjectProfile) -> Self {
        Self {
            repository_path: project.repository_path.clone(),
        }
    }
}

/// Per-host census summary in `GET /api/repositories`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostCensusSummary {
    pub machine: String,
    pub health: CopyHealth,
    pub copy_count: usize,
    pub dirty_count: usize,
    pub cairn_count: u32,
    pub error: Option<String>,
    pub observed_at_ms: Option<u64>,
}

/// Aggregated summary counts across reconciled repositories.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CatalogSummary {
    pub total: usize,
    pub synced: usize,
    pub diverged: usize,
    pub dirty: usize,
    pub unknown: usize,
    pub local_only: usize,
    pub total_local_copies: usize,
    pub total_cairns: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceHealthStatus {
    pub configured: bool,
    pub available: bool,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<Vec<String>>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CatalogSourcesStatus {
    pub catalog: SourceHealthStatus,
    pub local_scan: SourceHealthStatus,
    pub fleet: SourceHealthStatus,
}

/// Full response body for `GET /api/repositories` and `POST /api/repositories/refresh`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoriesResponse {
    pub schema: &'static str,
    pub generated_at_ms: u64,
    pub repositories: Vec<RepositoryRecord>,
    pub hosts: Vec<HostCensusSummary>,
    pub summary: CatalogSummary,
    pub sources: CatalogSourcesStatus,
    pub bounded: bool,
    pub truncated: bool,
    pub non_claims: Vec<String>,
}

/// Response body for catalog mutations (`import`, `mirrors`, `remove`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MutationResponse {
    pub schema: String,
    pub action: String,
    pub repository: RepositoryRecord,
    pub already_recorded: bool,
    pub request_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedRequestEntry {
    request_id: String,
    fingerprint: String,
    status: u16,
    response: MutationResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogStore {
    schema: String,
    records: Vec<RepositoryRecord>,
    #[serde(default)]
    requests: Vec<RecordedRequestEntry>,
}

impl Default for CatalogStore {
    fn default() -> Self {
        Self {
            schema: STORE_SCHEMA.to_owned(),
            records: Vec::new(),
            requests: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportRequest {
    provider: RemoteProvider,
    role: RemoteRole,
    url: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    default_ref: Option<String>,
    #[serde(default)]
    attach_to: Option<String>,
    request_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachMirrorRequest {
    provider: RemoteProvider,
    role: RemoteRole,
    url: String,
    #[serde(default)]
    default_ref: Option<String>,
    request_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveRequest {
    reason: String,
    request_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefreshRequest {}

#[derive(Clone, Debug, Deserialize)]
struct FleetHostPayload {
    machine: String,
    health: CopyHealth,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    copy_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
struct FleetRepositoriesPayload {
    #[serde(default)]
    schema: String,
    #[serde(default)]
    observed_at_ms: Option<u64>,
    #[serde(default)]
    hosts: Vec<FleetHostPayload>,
    #[serde(default)]
    copies: Vec<FleetLocalCopy>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Clone, Debug)]
struct LocalObservation {
    copies: Vec<FleetLocalCopy>,
    ci_paths: BTreeSet<String>,
    source_paths: BTreeSet<String>,
    truncated: bool,
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct FleetObservation {
    configured: bool,
    available: bool,
    hosts: Vec<FleetHostPayload>,
    copies: Vec<FleetLocalCopy>,
    observed_at_ms: Option<u64>,
    truncated: bool,
    error: Option<String>,
}

/// Stateful service managing the durable repository catalog and census aggregation.
pub struct CatalogService {
    state_directory: PathBuf,
    coordinator_machine: String,
    projects: Vec<ConfiguredProject>,
    sources: Vec<ConfiguredSource>,
    scan_roots: Vec<PathBuf>,
    store_lock: Mutex<()>,
    cached_snapshot: Mutex<Option<RepositoriesResponse>>,
}

impl std::fmt::Debug for CatalogService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CatalogService")
            .field("state_directory", &self.state_directory)
            .field("coordinator_machine", &self.coordinator_machine)
            .field("projects", &self.projects.len())
            .field("sources", &self.sources.len())
            .field("scan_roots", &self.scan_roots.len())
            .finish_non_exhaustive()
    }
}

impl CatalogService {
    /// Construct a new catalog service bound to the service's state directory and configured roots.
    #[must_use]
    pub fn new(
        profile: &ServiceProfile,
        sources: Vec<ConfiguredSource>,
        scan_roots: Vec<PathBuf>,
    ) -> Self {
        Self {
            state_directory: profile.state_directory.clone(),
            coordinator_machine: resolve_coordinator_machine(),
            projects: profile
                .projects
                .iter()
                .map(ConfiguredProject::from)
                .collect(),
            sources,
            scan_roots,
            store_lock: Mutex::new(()),
            cached_snapshot: Mutex::new(None),
        }
    }

    /// Dispatch `/api/repositories*` HTTP requests after caller authentication.
    pub(crate) fn response(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
        fleet: Option<&crate::fleet::Client>,
    ) -> ApiResponse {
        if role == Role::Worker {
            return error_json(
                403,
                "repositories_forbidden",
                "worker role cannot access repositories",
            );
        }
        if target.contains('#')
            || target
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        {
            return error_json(
                400,
                "repository_query_invalid",
                "request target contains invalid characters",
            );
        }
        let (path, query) = match target.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (target, None),
        };
        if query.is_some() {
            return error_json(
                400,
                "repository_query_invalid",
                "repository endpoints do not accept query parameters",
            );
        }
        let Some(rest) = path.strip_prefix("/api/repositories") else {
            return error_json(404, "not_found", "unknown repository route");
        };

        match (method, rest) {
            ("GET", "") => {
                if !body.is_empty() {
                    return error_json(
                        400,
                        "unexpected_body",
                        "GET /api/repositories takes no body",
                    );
                }
                self.handle_list(fleet)
            }
            ("POST", "") => error_json(
                405,
                "method_not_allowed",
                "POST /api/repositories is not allowed",
            ),
            ("POST", "/import") => {
                if role != Role::Operator {
                    return error_json(
                        403,
                        "operator_required",
                        "operator role required to import repositories",
                    );
                }
                self.handle_import(body, fleet)
            }
            ("POST", "/refresh") => {
                if role != Role::Operator {
                    return error_json(
                        403,
                        "operator_required",
                        "operator role required to refresh census",
                    );
                }
                self.handle_refresh(body, fleet)
            }
            ("GET", "/import" | "/refresh") => {
                error_json(405, "method_not_allowed", "mutation endpoint requires POST")
            }
            ("POST", suffix) => {
                if role != Role::Operator {
                    return error_json(
                        403,
                        "operator_required",
                        "operator role required to mutate repositories",
                    );
                }
                let Some(inner) = suffix.strip_prefix('/') else {
                    return error_json(404, "not_found", "unknown repository route");
                };
                let Some((id_hex, action)) = inner.split_once('/') else {
                    return error_json(404, "not_found", "unknown repository route");
                };
                match action {
                    "mirrors" => self.handle_attach_mirror(id_hex, body, fleet),
                    "remove" => self.handle_remove(id_hex, body),
                    _ => error_json(404, "not_found", "unknown repository action"),
                }
            }
            ("GET", suffix) if suffix.ends_with("/mirrors") || suffix.ends_with("/remove") => {
                error_json(405, "method_not_allowed", "mutation endpoint requires POST")
            }
            _ => error_json(404, "not_found", "unknown repository route"),
        }
    }

    /// Return a deterministic version/digest string for SSE change detection.
    #[must_use]
    pub fn catalog_digest(&self) -> String {
        let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let store = self.load_store().unwrap_or_default();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"kiln.repositories.digest.v1\0");
        if let Ok(bytes) = serde_json::to_vec(&store.records) {
            hasher.update(&bytes);
        }
        hasher.finalize().to_hex().to_string()
    }

    fn handle_list(&self, fleet: Option<&crate::fleet::Client>) -> ApiResponse {
        match self.build_catalog_snapshot(fleet) {
            Ok(snapshot) => {
                if let Ok(mut guard) = self.cached_snapshot.lock() {
                    *guard = Some(snapshot.clone());
                }
                json_api_response(200, &snapshot)
            }
            Err((status, code, msg)) => error_json(status, code, &msg),
        }
    }

    fn handle_refresh(&self, body: &[u8], fleet: Option<&crate::fleet::Client>) -> ApiResponse {
        if !body.is_empty() && serde_json::from_slice::<RefreshRequest>(body).is_err() {
            return error_json(
                400,
                "repository_request_invalid",
                "refresh payload must be empty or {}",
            );
        }
        match self.build_catalog_snapshot(fleet) {
            Ok(snapshot) => {
                if let Ok(mut guard) = self.cached_snapshot.lock() {
                    *guard = Some(snapshot.clone());
                }
                json_api_response(200, &snapshot)
            }
            Err((status, code, msg)) => error_json(status, code, &msg),
        }
    }

    fn handle_import(&self, body: &[u8], fleet: Option<&crate::fleet::Client>) -> ApiResponse {
        let req: ImportRequest = match serde_json::from_slice(body) {
            Ok(req) => req,
            Err(_) => {
                return error_json(
                    400,
                    "repository_request_invalid",
                    "import payload is malformed or contains unknown fields",
                );
            }
        };
        if !crate::profile::identifier(&req.request_id) {
            return error_json(
                400,
                "request_id_invalid",
                "request_id must be a bounded identifier",
            );
        }
        if let Some(ref label) = req.label
            && (label.trim().is_empty()
                || label.len() > MAX_SLUG_BYTES
                || label.chars().any(char::is_control))
        {
            return error_json(
                400,
                "invalid_repository_label",
                "repository label must be non-empty, bounded, and free of control characters",
            );
        }

        let fingerprint = request_fingerprint(&(
            "import",
            req.provider,
            req.role,
            &req.url,
            &req.label,
            &req.default_ref,
            &req.attach_to,
        ));

        let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut store = match self.load_store() {
            Ok(store) => store,
            Err(msg) => return error_json(503, "catalog_store_unavailable", &msg),
        };

        if let Some(replay) = check_request_replay(&store, &req.request_id, &fingerprint) {
            return match replay {
                Ok(mut resp) => {
                    resp.already_recorded = true;
                    json_api_response(200, &resp)
                }
                Err(()) => error_json(
                    409,
                    "request_id_conflict",
                    "request_id was already used with a different payload",
                ),
            };
        }

        let endpoint =
            match normalize_remote(req.provider, req.role, &req.url, req.default_ref.as_deref()) {
                Ok(ep) => ep,
                Err(diag) => return error_json(400, &diag.code, &diag.message),
            };

        let (status, action, mut mutated_record) = if let Some(ref attach_hex) = req.attach_to {
            let Some(target_id) = parse_content_id(attach_hex) else {
                return error_json(
                    400,
                    "repository_id_invalid",
                    "attach_to must be a 64-character lowercase BLAKE3 hex identity",
                );
            };
            match self.attach_to_existing(
                &mut store,
                &target_id,
                endpoint,
                req.label.clone(),
                fleet,
            ) {
                Ok(record) => (200_u16, "mirror_attached", record),
                Err((status, code, msg)) => return error_json(status, code, &msg),
            }
        } else {
            if let Some(conflict) = check_import_duplicate(&store.records, &endpoint) {
                return error_json(409, conflict.0, conflict.1);
            }
            if store.records.len() >= MAX_CATALOG_REPOS {
                return error_json(
                    409,
                    "catalog_limit_exceeded",
                    "repository catalog has reached MAX_CATALOG_REPOS",
                );
            }
            let record = match build_repository_record(
                endpoint,
                Vec::new(),
                req.label.clone(),
                RepositoryAdmission {
                    catalog_imported: true,
                    ci_configured: false,
                    source_configured: false,
                    fleet_enrolled: false,
                },
            ) {
                Ok(record) => record,
                Err(diag) => return error_json(400, &diag.code, &diag.message),
            };
            store.records.push(record.clone());
            (201_u16, "imported", record)
        };

        if let Ok(reconciled) = self.reconcile_single_record(&store.records, &mutated_record, fleet)
        {
            mutated_record = reconciled;
        }

        let response = MutationResponse {
            schema: MUTATION_SCHEMA.to_owned(),
            action: action.to_owned(),
            repository: mutated_record,
            already_recorded: false,
            request_id: req.request_id.clone(),
        };
        record_request(
            &mut store,
            req.request_id,
            fingerprint,
            status,
            response.clone(),
        );
        if let Err(msg) = self.save_store(&store) {
            return error_json(503, "catalog_store_write_failed", &msg);
        }
        if let Ok(mut guard) = self.cached_snapshot.lock() {
            *guard = None;
        }
        json_api_response(status, &response)
    }

    fn handle_attach_mirror(
        &self,
        id_hex: &str,
        body: &[u8],
        fleet: Option<&crate::fleet::Client>,
    ) -> ApiResponse {
        let Some(target_id) = parse_content_id(id_hex) else {
            return error_json(
                400,
                "repository_id_invalid",
                "repository id must be a 64-character lowercase BLAKE3 hex identity",
            );
        };
        let req: AttachMirrorRequest = match serde_json::from_slice(body) {
            Ok(req) => req,
            Err(_) => {
                return error_json(
                    400,
                    "repository_request_invalid",
                    "mirror payload is malformed or contains unknown fields",
                );
            }
        };
        if !crate::profile::identifier(&req.request_id) {
            return error_json(
                400,
                "request_id_invalid",
                "request_id must be a bounded identifier",
            );
        }

        let fingerprint = request_fingerprint(&(
            "mirror",
            id_hex,
            req.provider,
            req.role,
            &req.url,
            &req.default_ref,
        ));

        let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut store = match self.load_store() {
            Ok(store) => store,
            Err(msg) => return error_json(503, "catalog_store_unavailable", &msg),
        };

        if let Some(replay) = check_request_replay(&store, &req.request_id, &fingerprint) {
            return match replay {
                Ok(mut resp) => {
                    resp.already_recorded = true;
                    json_api_response(200, &resp)
                }
                Err(()) => error_json(
                    409,
                    "request_id_conflict",
                    "request_id was already used with a different payload",
                ),
            };
        }

        let endpoint =
            match normalize_remote(req.provider, req.role, &req.url, req.default_ref.as_deref()) {
                Ok(ep) => ep,
                Err(diag) => return error_json(400, &diag.code, &diag.message),
            };

        let mut updated =
            match self.attach_to_existing(&mut store, &target_id, endpoint, None, fleet) {
                Ok(record) => record,
                Err((status, code, msg)) => return error_json(status, code, &msg),
            };

        if let Ok(reconciled) = self.reconcile_single_record(&store.records, &updated, fleet) {
            updated = reconciled;
        }

        let response = MutationResponse {
            schema: MUTATION_SCHEMA.to_owned(),
            action: "mirror_attached".to_owned(),
            repository: updated,
            already_recorded: false,
            request_id: req.request_id.clone(),
        };
        record_request(
            &mut store,
            req.request_id,
            fingerprint,
            200,
            response.clone(),
        );
        if let Err(msg) = self.save_store(&store) {
            return error_json(503, "catalog_store_write_failed", &msg);
        }
        if let Ok(mut guard) = self.cached_snapshot.lock() {
            *guard = None;
        }
        json_api_response(200, &response)
    }

    fn handle_remove(&self, id_hex: &str, body: &[u8]) -> ApiResponse {
        let Some(target_id) = parse_content_id(id_hex) else {
            return error_json(
                400,
                "repository_id_invalid",
                "repository id must be a 64-character lowercase BLAKE3 hex identity",
            );
        };
        let req: RemoveRequest = match serde_json::from_slice(body) {
            Ok(req) => req,
            Err(_) => {
                return error_json(
                    400,
                    "repository_request_invalid",
                    "remove payload is malformed or contains unknown fields",
                );
            }
        };
        if !crate::profile::identifier(&req.request_id) {
            return error_json(
                400,
                "request_id_invalid",
                "request_id must be a bounded identifier",
            );
        }
        if req.reason.trim().is_empty()
            || req.reason.len() > MAX_REASON_BYTES
            || req.reason.chars().any(char::is_control)
        {
            return error_json(
                400,
                "reason_invalid",
                "removal reason must be non-empty, bounded, and free of control characters",
            );
        }

        let fingerprint = request_fingerprint(&("remove", id_hex, &req.reason));

        let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut store = match self.load_store() {
            Ok(store) => store,
            Err(msg) => return error_json(503, "catalog_store_unavailable", &msg),
        };

        if let Some(replay) = check_request_replay(&store, &req.request_id, &fingerprint) {
            return match replay {
                Ok(mut resp) => {
                    resp.already_recorded = true;
                    json_api_response(200, &resp)
                }
                Err(()) => error_json(
                    409,
                    "request_id_conflict",
                    "request_id was already used with a different payload",
                ),
            };
        }

        let Some(pos) = store.records.iter().position(|r| r.identity == target_id) else {
            return error_json(
                404,
                "repository_not_found",
                "repository is not registered in the imported catalog",
            );
        };
        // Removal ONLY updates catalog metadata in repositories.json; never delete disk checkouts.
        let mut removed = store.records.remove(pos);
        removed.admission.catalog_imported = false;

        let response = MutationResponse {
            schema: MUTATION_SCHEMA.to_owned(),
            action: "removed".to_owned(),
            repository: removed,
            already_recorded: false,
            request_id: req.request_id.clone(),
        };
        record_request(
            &mut store,
            req.request_id,
            fingerprint,
            200,
            response.clone(),
        );
        if let Err(msg) = self.save_store(&store) {
            return error_json(503, "catalog_store_write_failed", &msg);
        }
        if let Ok(mut guard) = self.cached_snapshot.lock() {
            *guard = None;
        }
        json_api_response(200, &response)
    }

    fn attach_to_existing(
        &self,
        store: &mut CatalogStore,
        target_id: &ContentId,
        endpoint: RemoteEndpoint,
        label_override: Option<String>,
        fleet: Option<&crate::fleet::Client>,
    ) -> Result<RepositoryRecord, (u16, &'static str, String)> {
        if let Some(record) = store.records.iter_mut().find(|r| &r.identity == target_id) {
            if let Err(diag) = attach_remote_to_record(record, endpoint) {
                let status = map_diagnostic_status(&diag.code);
                return Err((status, static_code(&diag.code), diag.message));
            }
            if record.label.is_none() && label_override.is_some() {
                record.label = label_override;
            }
            record.admission.catalog_imported = true;
            return Ok(record.clone());
        }

        // Allow attaching a canonical/mirror remote to a discovered local/CI/fleet repo by promoting it.
        if store.records.len() >= MAX_CATALOG_REPOS {
            return Err((
                409,
                "catalog_limit_exceeded",
                "repository catalog has reached MAX_CATALOG_REPOS".to_owned(),
            ));
        }
        let discovered = self.observe_without_store(fleet);
        let reconciled = reconcile_catalog(&[], &discovered.all_copies);
        let Some(mut promoted) = reconciled.into_iter().find(|r| &r.identity == target_id) else {
            return Err((
                404,
                "repository_not_found",
                "target repository not found in catalog or observed copies".to_owned(),
            ));
        };
        if let Err(diag) = attach_remote_to_record(&mut promoted, endpoint) {
            let status = map_diagnostic_status(&diag.code);
            return Err((status, static_code(&diag.code), diag.message));
        }
        if promoted.label.is_none() && label_override.is_some() {
            promoted.label = label_override;
        }
        promoted.admission.catalog_imported = true;
        promoted.local_copies.clear();
        store.records.push(promoted.clone());
        Ok(promoted)
    }

    fn reconcile_single_record(
        &self,
        records: &[RepositoryRecord],
        target: &RepositoryRecord,
        fleet: Option<&crate::fleet::Client>,
    ) -> Result<RepositoryRecord, ()> {
        let obs = self.observe_without_store(fleet);
        let mut reconciled = reconcile_catalog(records, &obs.all_copies);
        enrich_admissions(&mut reconciled, &obs);
        reconciled
            .into_iter()
            .find(|r| r.identity == target.identity)
            .ok_or(())
    }

    fn build_catalog_snapshot(
        &self,
        fleet: Option<&crate::fleet::Client>,
    ) -> Result<RepositoriesResponse, (u16, &'static str, String)> {
        let now_ms = crate::engine::host_time_ms().map_err(|_| {
            (
                503_u16,
                "clock_unavailable",
                "host clock unavailable".to_owned(),
            )
        })?;
        let store = {
            let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
            self.load_store()
                .map_err(|msg| (503_u16, "catalog_store_unavailable", msg))?
        };
        let obs = self.observe_without_store(fleet);
        let mut repositories = reconcile_catalog(&store.records, &obs.all_copies);
        enrich_admissions(&mut repositories, &obs);

        let hosts = build_host_census(
            &self.coordinator_machine,
            now_ms,
            &obs.local,
            &obs.fleet,
            &obs.all_copies,
        );
        let summary = summarize_repositories(&repositories);
        let has_radicle = repositories.iter().any(|r| {
            r.canonical
                .as_ref()
                .is_some_and(|c| c.provider == RemoteProvider::Radicle)
                || r.remotes
                    .iter()
                    .any(|ep| ep.provider == RemoteProvider::Radicle)
        });
        let non_claims = if has_radicle {
            default_non_claims(true)
        } else {
            CATALOG_NON_CLAIMS.iter().map(|s| (*s).to_owned()).collect()
        };

        let local_configured =
            !self.projects.is_empty() || !self.sources.is_empty() || !self.scan_roots.is_empty();
        let scan_root_strings: Vec<String> = self
            .scan_roots
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        let sources = CatalogSourcesStatus {
            catalog: SourceHealthStatus {
                configured: true,
                available: true,
                count: store.records.len(),
                roots: None,
                error: None,
            },
            local_scan: SourceHealthStatus {
                configured: local_configured,
                available: obs.local.error.is_none(),
                count: obs.local.copies.len(),
                roots: (!scan_root_strings.is_empty()).then_some(scan_root_strings),
                error: obs.local.error,
            },
            fleet: SourceHealthStatus {
                configured: obs.fleet.configured,
                available: obs.fleet.available,
                count: obs.fleet.copies.len(),
                roots: None,
                error: obs.fleet.error,
            },
        };

        Ok(RepositoriesResponse {
            schema: REPOSITORIES_SCHEMA,
            generated_at_ms: now_ms,
            repositories,
            hosts,
            summary,
            sources,
            bounded: true,
            truncated: obs.local.truncated
                || obs.fleet.truncated
                || store.records.len() >= MAX_CATALOG_REPOS,
            non_claims,
        })
    }

    fn observe_without_store(&self, fleet: Option<&crate::fleet::Client>) -> CombinedObservation {
        let now_ms = crate::engine::host_time_ms().unwrap_or(0);
        let local = observe_local_checkouts(
            &self.coordinator_machine,
            &self.projects,
            &self.sources,
            &self.scan_roots,
            now_ms,
        );
        let fleet_obs = observe_fleet_repositories(fleet);
        let (all_copies, fleet_keys) =
            merge_local_and_fleet_copies(&local.copies, &fleet_obs.copies);
        CombinedObservation {
            local,
            fleet: fleet_obs,
            all_copies,
            fleet_keys,
        }
    }

    fn load_store(&self) -> Result<CatalogStore, String> {
        let dir_fd = match crate::worker::files::directory(&self.state_directory) {
            Ok(fd) => fd,
            Err(_) if !self.state_directory.exists() => return Ok(CatalogStore::default()),
            Err(_) => return Err("state directory unavailable or unsafe".to_owned()),
        };
        let file = match openat(
            &dir_fd,
            Path::new(CATALOG_FILENAME),
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(nix::errno::Errno::ENOENT) => return Ok(CatalogStore::default()),
            Err(_) => return Err("catalog store file unavailable or unsafe".to_owned()),
        };
        let meta = file
            .metadata()
            .map_err(|_| "catalog store metadata unavailable".to_owned())?;
        if !meta.is_file()
            || meta.uid() != geteuid().as_raw()
            || meta.nlink() != 1
            || (meta.mode() & 0o7777) != 0o600
            || meta.len() > MAX_STORE_BYTES as u64
        {
            return Err("catalog store file ownership, permissions, or bound denied".to_owned());
        }
        let mut bytes = Vec::with_capacity(meta.len() as usize);
        Read::by_ref(&mut &file)
            .take(MAX_STORE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "catalog store read failed".to_owned())?;
        if bytes.len() > MAX_STORE_BYTES {
            return Err("catalog store exceeds size bound".to_owned());
        }
        let store: CatalogStore = serde_json::from_slice(&bytes)
            .map_err(|_| "catalog store JSON is malformed".to_owned())?;
        if store.schema != STORE_SCHEMA || store.records.len() > MAX_CATALOG_REPOS {
            return Err("catalog store schema or bound rejected".to_owned());
        }
        Ok(store)
    }

    fn save_store(&self, store: &CatalogStore) -> Result<(), String> {
        ensure_private_state_dir(&self.state_directory)?;
        let dir_fd = crate::worker::files::directory(&self.state_directory)
            .map_err(|_| "state directory unavailable or unsafe".to_owned())?;
        let dir_file = File::from(dir_fd);
        let dir_meta = dir_file
            .metadata()
            .map_err(|_| "state directory metadata unavailable".to_owned())?;
        if !dir_meta.is_dir()
            || dir_meta.uid() != geteuid().as_raw()
            || (dir_meta.mode() & 0o7777) != 0o700
        {
            return Err("state directory must be owned and mode 0700".to_owned());
        }
        drop(dir_file);

        let bytes = serde_json::to_vec_pretty(store)
            .map_err(|_| "catalog store serialization failed".to_owned())?;
        if bytes.len() > MAX_STORE_BYTES {
            return Err("catalog store exceeds MAX_STORE_BYTES".to_owned());
        }

        let mut nonce = [0_u8; 8];
        getrandom::fill(&mut nonce).map_err(|_| "randomness unavailable for atomic store write")?;
        let mut suffix = String::with_capacity(16);
        for byte in nonce {
            use std::fmt::Write as _;
            let _ = write!(&mut suffix, "{byte:02x}");
        }
        let tmp_name = format!("{CATALOG_FILENAME}.tmp.{suffix}");
        let tmp_path = self.state_directory.join(&tmp_name);
        let final_path = self.state_directory.join(CATALOG_FILENAME);

        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .mode(0o600)
            .open(&tmp_path)
            .map_err(|_| "could not create temporary catalog file".to_owned())?;
        if tmp_file.write_all(&bytes).is_err() || tmp_file.sync_all().is_err() {
            let _ = fs::remove_file(&tmp_path);
            return Err("failed writing temporary catalog store".to_owned());
        }
        drop(tmp_file);

        if fs::rename(&tmp_path, &final_path).is_err() {
            let _ = fs::remove_file(&tmp_path);
            return Err("atomic rename of catalog store failed".to_owned());
        }
        if let Ok(parent_fd) = crate::worker::files::directory(&self.state_directory) {
            let _ = File::from(parent_fd).sync_all();
        }
        Ok(())
    }
}

struct CombinedObservation {
    local: LocalObservation,
    fleet: FleetObservation,
    all_copies: Vec<FleetLocalCopy>,
    fleet_keys: BTreeSet<(String, String)>,
}

fn ensure_private_state_dir(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return Err("state directory is not a plain directory".to_owned());
            }
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            crate::worker::files::create_private(path)
        }
        Err(_) => Err("state directory metadata unavailable".to_owned()),
    }
}

fn resolve_coordinator_machine() -> String {
    let raw = fs::read_to_string("/proc/sys/kernel/hostname")
        .or_else(|_| fs::read_to_string("/etc/hostname"))
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "coordinator".to_owned());
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_MACHINE_BYTES)
        .collect();
    if cleaned.is_empty() {
        "coordinator".to_owned()
    } else {
        cleaned
    }
}

fn parse_content_id(raw: &str) -> Option<ContentId> {
    if raw.len() != blake3::OUT_LEN * 2
        || !raw
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    ContentId::from_blake3_hex(raw).ok()
}

fn request_fingerprint<T: Serialize>(value: &T) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    blake3::hash(&encoded).to_hex().to_string()
}

fn check_request_replay(
    store: &CatalogStore,
    request_id: &str,
    fingerprint: &str,
) -> Option<Result<MutationResponse, ()>> {
    store
        .requests
        .iter()
        .find(|entry| entry.request_id == request_id)
        .map(|entry| {
            if entry.fingerprint == fingerprint {
                Ok(entry.response.clone())
            } else {
                Err(())
            }
        })
}

fn record_request(
    store: &mut CatalogStore,
    request_id: String,
    fingerprint: String,
    status: u16,
    response: MutationResponse,
) {
    if store.requests.len() >= MAX_REQUEST_LEDGER {
        let excess = store.requests.len() + 1 - MAX_REQUEST_LEDGER;
        store.requests.drain(0..excess);
    }
    store.requests.push(RecordedRequestEntry {
        request_id,
        fingerprint,
        status,
        response,
    });
}

fn check_import_duplicate(
    records: &[RepositoryRecord],
    endpoint: &RemoteEndpoint,
) -> Option<(&'static str, &'static str)> {
    for record in records {
        let same_remote = record.remotes.iter().any(|r| {
            r.provider == endpoint.provider && r.normalized_key == endpoint.normalized_key
        }) || record.canonical.as_ref().is_some_and(|c| {
            c.provider == endpoint.provider && c.normalized_key == endpoint.normalized_key
        });
        if same_remote {
            if endpoint.role == RemoteRole::Canonical && record.canonical.is_some() {
                return Some((
                    "duplicate_canonical_remote",
                    "a canonical remote is already registered for this repository",
                ));
            }
            return Some((
                "duplicate_remote",
                "remote endpoint is already registered in the catalog",
            ));
        }
    }
    None
}

fn map_diagnostic_status(code: &str) -> u16 {
    match code {
        "duplicate_canonical_remote"
        | "duplicate_remote"
        | "remote_limit_exceeded"
        | "catalog_limit_exceeded" => 409,
        _ => 400,
    }
}

fn static_code(code: &str) -> &'static str {
    match code {
        "duplicate_canonical_remote" => "duplicate_canonical_remote",
        "duplicate_remote" => "duplicate_remote",
        "remote_limit_exceeded" => "remote_limit_exceeded",
        "catalog_limit_exceeded" => "catalog_limit_exceeded",
        "embedded_credentials" => "embedded_credentials",
        "invalid_radicle_rid" => "invalid_radicle_rid",
        "invalid_radicle_rid_length" => "invalid_radicle_rid_length",
        "invalid_radicle_rid_base58" => "invalid_radicle_rid_base58",
        "ssrf_ip_literal_rejected" => "ssrf_ip_literal_rejected",
        "ssrf_loopback_rejected" => "ssrf_loopback_rejected",
        "invalid_github_url" => "invalid_github_url",
        "invalid_github_host" => "invalid_github_host",
        "invalid_gitea_url" => "invalid_gitea_url",
        "invalid_gitea_host" => "invalid_gitea_host",
        "invalid_gitea_port" => "invalid_gitea_port",
        "invalid_ssh_userinfo" => "invalid_ssh_userinfo",
        "invalid_repo_path" => "invalid_repo_path",
        "invalid_repo_segment" => "invalid_repo_segment",
        "invalid_default_ref" => "invalid_default_ref",
        "invalid_repository_label" => "invalid_repository_label",
        _ => "repository_request_invalid",
    }
}

fn observe_fleet_repositories(fleet: Option<&crate::fleet::Client>) -> FleetObservation {
    let Some(client) = fleet else {
        return FleetObservation {
            configured: false,
            available: false,
            hosts: Vec::new(),
            copies: Vec::new(),
            observed_at_ms: None,
            truncated: false,
            error: None,
        };
    };
    let Ok(reply) = client.call("GET", "/v1/repositories", None) else {
        return FleetObservation {
            configured: true,
            available: false,
            hosts: Vec::new(),
            copies: Vec::new(),
            observed_at_ms: None,
            truncated: false,
            error: Some("fleet coordinator unreachable".to_owned()),
        };
    };
    if !(200..300).contains(&reply.status) {
        return FleetObservation {
            configured: true,
            available: false,
            hosts: Vec::new(),
            copies: Vec::new(),
            observed_at_ms: None,
            truncated: false,
            error: Some(format!(
                "fleet repository census returned HTTP {}",
                reply.status
            )),
        };
    }
    let Ok(payload) = serde_json::from_slice::<FleetRepositoriesPayload>(&reply.body) else {
        return FleetObservation {
            configured: true,
            available: false,
            hosts: Vec::new(),
            copies: Vec::new(),
            observed_at_ms: None,
            truncated: false,
            error: Some("fleet repository census response was malformed".to_owned()),
        };
    };
    let mut valid_copies = Vec::with_capacity(payload.copies.len());
    for copy in payload.copies {
        if validate_local_copy(&copy).is_ok() {
            valid_copies.push(copy);
        }
    }
    let _ = payload.schema;
    FleetObservation {
        configured: true,
        available: true,
        hosts: payload.hosts,
        copies: valid_copies,
        observed_at_ms: payload.observed_at_ms,
        truncated: payload.truncated,
        error: None,
    }
}

fn merge_local_and_fleet_copies(
    local: &[FleetLocalCopy],
    fleet: &[FleetLocalCopy],
) -> (Vec<FleetLocalCopy>, BTreeSet<(String, String)>) {
    let mut merged: BTreeMap<(String, String), FleetLocalCopy> = BTreeMap::new();
    let mut fleet_keys = BTreeSet::new();
    for copy in fleet {
        let key = (copy.machine.clone(), copy.path.clone());
        fleet_keys.insert(key.clone());
        merged.insert(key, copy.clone());
    }
    for copy in local {
        let key = (copy.machine.clone(), copy.path.clone());
        // Prefer available local observation over unavailable/missing if same (machine, path).
        match merged.get(&key) {
            Some(existing)
                if existing.health == CopyHealth::Available
                    && copy.health == CopyHealth::Unavailable => {}
            _ => {
                merged.insert(key, copy.clone());
            }
        }
    }
    (merged.into_values().collect(), fleet_keys)
}

fn enrich_admissions(records: &mut [RepositoryRecord], obs: &CombinedObservation) {
    for record in records {
        for copy in &record.local_copies {
            if obs.local.ci_paths.contains(&copy.path) {
                record.admission.ci_configured = true;
            }
            if obs.local.source_paths.contains(&copy.path) {
                record.admission.source_configured = true;
            }
            if obs
                .fleet_keys
                .contains(&(copy.machine.clone(), copy.path.clone()))
            {
                record.admission.fleet_enrolled = true;
            }
        }
    }
}

fn build_host_census(
    coordinator_machine: &str,
    now_ms: u64,
    local: &LocalObservation,
    fleet: &FleetObservation,
    all_copies: &[FleetLocalCopy],
) -> Vec<HostCensusSummary> {
    let mut per_machine: BTreeMap<String, HostCensusSummary> = BTreeMap::new();

    if !local.copies.is_empty() || local.error.is_some() {
        per_machine.insert(
            coordinator_machine.to_owned(),
            HostCensusSummary {
                machine: coordinator_machine.to_owned(),
                health: if local.error.is_some() && local.copies.is_empty() {
                    CopyHealth::Unavailable
                } else {
                    CopyHealth::Available
                },
                copy_count: 0,
                dirty_count: 0,
                cairn_count: 0,
                error: local.error.clone(),
                observed_at_ms: Some(now_ms),
            },
        );
    }

    for host in &fleet.hosts {
        let entry = per_machine
            .entry(host.machine.clone())
            .or_insert_with(|| HostCensusSummary {
                machine: host.machine.clone(),
                health: host.health,
                copy_count: host.copy_count,
                dirty_count: 0,
                cairn_count: 0,
                error: host.error.clone(),
                observed_at_ms: fleet.observed_at_ms,
            });
        if host.health == CopyHealth::Unavailable {
            entry.health = CopyHealth::Unavailable;
            if entry.error.is_none() {
                entry.error = host.error.clone();
            }
        }
    }

    for copy in all_copies {
        let entry = per_machine
            .entry(copy.machine.clone())
            .or_insert_with(|| HostCensusSummary {
                machine: copy.machine.clone(),
                health: copy.health,
                copy_count: 0,
                dirty_count: 0,
                cairn_count: 0,
                error: copy.error.clone(),
                observed_at_ms: copy.observed_at_ms,
            });
        entry.copy_count += 1;
        if copy.dirty == Some(true) {
            entry.dirty_count += 1;
        }
        entry.cairn_count = entry
            .cairn_count
            .saturating_add(copy.cairn_count.unwrap_or(0));
        if copy.health == CopyHealth::Unavailable && entry.copy_count == 1 {
            entry.health = CopyHealth::Unavailable;
            if entry.error.is_none() {
                entry.error = copy.error.clone();
            }
        }
    }

    per_machine.into_values().collect()
}

fn summarize_repositories(repositories: &[RepositoryRecord]) -> CatalogSummary {
    let mut summary = CatalogSummary {
        total: repositories.len(),
        ..CatalogSummary::default()
    };
    for repo in repositories {
        match repo.sync_status {
            FleetSyncStatus::Synced => summary.synced += 1,
            FleetSyncStatus::Diverged => summary.diverged += 1,
            FleetSyncStatus::Dirty => summary.dirty += 1,
            FleetSyncStatus::Unknown => summary.unknown += 1,
            FleetSyncStatus::LocalOnly => summary.local_only += 1,
        }
        summary.total_local_copies += repo.local_copies.len();
        summary.total_cairns = summary.total_cairns.saturating_add(repo.total_cairns);
    }
    summary
}

#[derive(Clone, Debug)]
struct LocalScanTarget {
    path: PathBuf,
    hint_kind: Option<LocalVcs>,
}

fn observe_local_checkouts(
    machine: &str,
    projects: &[ConfiguredProject],
    sources: &[ConfiguredSource],
    scan_roots: &[PathBuf],
    now_ms: u64,
) -> LocalObservation {
    let mut seen_paths: BTreeSet<PathBuf> = BTreeSet::new();
    let mut targets: Vec<LocalScanTarget> = Vec::new();
    let mut ci_paths = BTreeSet::new();
    let mut source_paths = BTreeSet::new();
    let mut truncated = false;
    let mut root_errors = Vec::new();

    for project in projects {
        let key = normalized_path_string(&project.repository_path);
        ci_paths.insert(key);
        if seen_paths.insert(project.repository_path.clone()) {
            targets.push(LocalScanTarget {
                path: project.repository_path.clone(),
                hint_kind: None,
            });
        }
    }

    for source in sources {
        let key = normalized_path_string(&source.path);
        source_paths.insert(key);
        let hint = match source.kind.as_str() {
            "git" => Some(LocalVcs::Git),
            "jj" => Some(LocalVcs::Jj),
            "pijul" => Some(LocalVcs::Pijul),
            _ => None,
        };
        if seen_paths.insert(source.path.clone()) {
            targets.push(LocalScanTarget {
                path: source.path.clone(),
                hint_kind: hint,
            });
        }
    }

    for root in scan_roots {
        if targets.len() >= MAX_LOCAL_SCAN {
            truncated = true;
            break;
        }
        match discover_scan_root(
            root,
            MAX_LOCAL_SCAN - targets.len(),
            &mut seen_paths,
            &mut targets,
        ) {
            Ok(root_truncated) => {
                if root_truncated {
                    truncated = true;
                }
            }
            Err(err) => {
                root_errors.push(err);
            }
        }
    }

    if targets.len() > MAX_LOCAL_SCAN {
        targets.truncate(MAX_LOCAL_SCAN);
        truncated = true;
    }

    let mut copies = Vec::with_capacity(targets.len());
    for target in &targets {
        let copy = inspect_checkout(machine, &target.path, target.hint_kind, now_ms);
        if validate_local_copy(&copy).is_ok() {
            copies.push(copy);
        }
    }

    let error = if root_errors.is_empty() {
        None
    } else {
        Some(truncate_text(&root_errors.join("; "), MAX_ERROR_BYTES))
    };

    LocalObservation {
        copies,
        ci_paths,
        source_paths,
        truncated,
        error,
    }
}

fn discover_scan_root(
    root: &Path,
    budget: usize,
    seen: &mut BTreeSet<PathBuf>,
    out: &mut Vec<LocalScanTarget>,
) -> Result<bool, String> {
    let meta = fs::symlink_metadata(root)
        .map_err(|_| format!("scan root unreadable: {}", root.display()))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(format!(
            "scan root is not a plain directory: {}",
            root.display()
        ));
    }
    let mut truncated = false;
    let mut discovered = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if discovered >= budget {
            truncated = true;
            break;
        }
        if depth > 0 && detect_vcs_kind(&dir).is_some() {
            if seen.insert(dir.clone()) {
                out.push(LocalScanTarget {
                    path: dir,
                    hint_kind: None,
                });
                discovered += 1;
            }
            continue;
        }
        if depth >= MAX_SCAN_DEPTH {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children = Vec::new();
        for (idx, entry) in entries.enumerate() {
            if idx >= MAX_SCAN_ENTRIES_PER_DIR {
                truncated = true;
                break;
            }
            let Ok(entry) = entry else {
                continue;
            };
            let name = entry.file_name();
            let Some(name_str) = name.to_str() else {
                continue;
            };
            if name_str.starts_with('.') || matches!(name_str, "target" | "node_modules") {
                continue;
            }
            let child_path = entry.path();
            let Ok(child_meta) = fs::symlink_metadata(&child_path) else {
                continue;
            };
            if !child_meta.file_type().is_symlink() && child_meta.is_dir() {
                children.push(child_path);
            }
        }
        children.sort();
        for child in children.into_iter().rev() {
            stack.push((child, depth + 1));
        }
    }
    Ok(truncated)
}

fn normalized_path_string(path: &Path) -> String {
    truncate_text(&path.display().to_string(), MAX_PATH_BYTES)
}

fn detect_vcs_kind(path: &Path) -> Option<LocalVcs> {
    if is_plain_dir(&path.join(".jj")) {
        return Some(LocalVcs::Jj);
    }
    if is_plain_dir(&path.join(".pijul")) {
        return Some(LocalVcs::Pijul);
    }
    if is_plain_dir(&path.join(".git")) || is_plain_file(&path.join(".git")) {
        return Some(LocalVcs::Git);
    }
    if is_plain_file(&path.join("HEAD")) && is_plain_dir(&path.join("objects")) {
        return Some(LocalVcs::Git);
    }
    None
}

fn is_plain_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| !m.file_type().is_symlink() && m.is_dir())
}

fn is_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| !m.file_type().is_symlink() && m.is_file())
}

fn inspect_checkout(
    machine: &str,
    path: &Path,
    hint_kind: Option<LocalVcs>,
    now_ms: u64,
) -> FleetLocalCopy {
    let path_str = normalized_path_string(path);
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(_) => {
            return unavailable_copy(
                machine,
                &path_str,
                hint_kind,
                now_ms,
                "checkout path missing or unreadable",
            );
        }
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return unavailable_copy(
            machine,
            &path_str,
            hint_kind,
            now_ms,
            "checkout path is not a plain directory",
        );
    }

    let vcs = detect_vcs_kind(path).or(hint_kind);
    let cairn_count = count_cairn_changes(path);

    match vcs {
        Some(LocalVcs::Pijul) => FleetLocalCopy {
            machine: machine.to_owned(),
            path: path_str,
            vcs: Some(LocalVcs::Pijul),
            head_commit: None,
            branch_or_bookmark: read_pijul_channel(path),
            dirty: None,
            ahead: None,
            behind: None,
            remotes: read_pijul_remotes(path),
            cairn_count: Some(cairn_count),
            observed_at_ms: Some(now_ms),
            health: CopyHealth::Available,
            error: None,
        },
        Some(kind @ (LocalVcs::Git | LocalVcs::Jj)) => {
            let (git_open_path, is_worktree) = resolve_git_open_path(path, kind);
            let Some(git_open_path) = git_open_path else {
                return FleetLocalCopy {
                    machine: machine.to_owned(),
                    path: path_str,
                    vcs: Some(kind),
                    head_commit: None,
                    branch_or_bookmark: None,
                    dirty: None,
                    ahead: None,
                    behind: None,
                    remotes: Vec::new(),
                    cairn_count: Some(cairn_count),
                    observed_at_ms: Some(now_ms),
                    health: CopyHealth::Available,
                    error: Some("no git store found for checkout".to_owned()),
                };
            };
            let Ok(repo) = Repository::open_ext(
                &git_open_path,
                RepositoryOpenFlags::NO_SEARCH | RepositoryOpenFlags::NO_DOTGIT,
                std::iter::empty::<&OsStr>(),
            ) else {
                return unavailable_copy(
                    machine,
                    &path_str,
                    Some(kind),
                    now_ms,
                    "git object store could not be opened",
                );
            };
            let (head_commit, branch_or_bookmark, ahead, behind) =
                inspect_git_head_and_divergence(&repo);
            let dirty = if is_worktree && !repo.is_bare() {
                let mut opts = git2::StatusOptions::new();
                opts.include_untracked(true).recurse_untracked_dirs(false);
                repo.statuses(Some(&mut opts)).ok().map(|s| !s.is_empty())
            } else {
                None
            };
            let remotes = read_git_remotes(&repo);
            FleetLocalCopy {
                machine: machine.to_owned(),
                path: path_str,
                vcs: Some(kind),
                head_commit,
                branch_or_bookmark,
                dirty,
                ahead,
                behind,
                remotes,
                cairn_count: Some(cairn_count),
                observed_at_ms: Some(now_ms),
                health: CopyHealth::Available,
                error: None,
            }
        }
        None => unavailable_copy(
            machine,
            &path_str,
            None,
            now_ms,
            "unrecognized repository format",
        ),
    }
}

fn resolve_git_open_path(path: &Path, kind: LocalVcs) -> (Option<PathBuf>, bool) {
    let dot_git = path.join(".git");
    if is_plain_dir(&dot_git) {
        return (Some(dot_git), true);
    }
    if kind == LocalVcs::Jj {
        let jj_git = path.join(".jj").join("repo").join("store").join("git");
        if is_plain_file(&jj_git.join("HEAD")) {
            return (Some(jj_git), false);
        }
    }
    if is_plain_file(&path.join("HEAD")) && is_plain_dir(&path.join("objects")) {
        return (Some(path.to_path_buf()), false);
    }
    (None, false)
}

fn inspect_git_head_and_divergence(
    repo: &Repository,
) -> (Option<String>, Option<String>, Option<u32>, Option<u32>) {
    let Ok(head_ref) = repo.head() else {
        return (None, None, None, None);
    };
    let head_commit = head_ref
        .peel_to_commit()
        .ok()
        .map(|c| c.id().to_string())
        .filter(|hex| (COMMIT_HEX_MIN..=COMMIT_HEX_MAX).contains(&hex.len()));
    let branch_or_bookmark = if head_ref.is_branch() {
        head_ref.shorthand().ok().map(str::to_owned)
    } else {
        None
    };

    let (ahead, behind) = if let (Some(local_oid), Some(branch_name)) =
        (head_ref.target(), branch_or_bookmark.as_deref())
    {
        repo.find_branch(branch_name, git2::BranchType::Local)
            .ok()
            .and_then(|b| b.upstream().ok())
            .and_then(|u| u.get().target())
            .and_then(|up_oid| repo.graph_ahead_behind(local_oid, up_oid).ok())
            .map_or((None, None), |(a, b)| {
                (u32::try_from(a).ok(), u32::try_from(b).ok())
            })
    } else {
        (None, None)
    };

    (head_commit, branch_or_bookmark, ahead, behind)
}

fn read_git_remotes(repo: &Repository) -> Vec<String> {
    let Ok(names) = repo.remotes() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in names.iter().flatten().flatten() {
        if out.len() >= MAX_REMOTES_PER_COPY {
            break;
        }
        let Ok(remote) = repo.find_remote(name) else {
            continue;
        };
        let Ok(url) = remote.url() else {
            continue;
        };
        if has_embedded_credentials(url) || url.chars().any(char::is_control) {
            continue;
        }
        let owned = url.to_owned();
        if !out.contains(&owned) {
            out.push(owned);
        }
    }
    out
}

fn has_embedded_credentials(url: &str) -> bool {
    if let Some((_, rest)) = url.split_once("://") {
        let authority = rest.split('/').next().unwrap_or(rest);
        if authority.contains('@') && !url.starts_with("ssh://git@") {
            return true;
        }
    }
    false
}

fn read_pijul_channel(path: &Path) -> Option<String> {
    let config_path = path.join(".pijul").join("config");
    let text = fs::read_to_string(config_path).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("current_channel")
            && let Some((_, val)) = rest.split_once('=')
        {
            let channel = val.trim().trim_matches('"').trim_matches('\'');
            if !channel.is_empty() && !channel.chars().any(char::is_control) {
                return Some(channel.to_owned());
            }
        }
    }
    None
}

fn read_pijul_remotes(path: &Path) -> Vec<String> {
    let config_path = path.join(".pijul").join("config");
    let Ok(text) = fs::read_to_string(config_path) else {
        return Vec::new();
    };
    let mut remotes = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("default_remote")
            && let Some((_, val)) = rest.split_once('=')
        {
            let remote = val.trim().trim_matches('"').trim_matches('\'');
            if !remote.is_empty() && !has_embedded_credentials(remote) {
                remotes.push(remote.to_owned());
            }
        }
    }
    remotes
}

fn count_cairn_changes(repo_path: &Path) -> u32 {
    let changes_dir = repo_path.join(".cairn").join("changes");
    let Ok(entries) = fs::read_dir(&changes_dir) else {
        return 0;
    };
    let mut count = 0_u32;
    for entry in entries {
        if count >= MAX_CAIRN_COUNT_SCAN {
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.starts_with('.') || name_str == "archive" {
            continue;
        }
        if fs::symlink_metadata(entry.path())
            .is_ok_and(|m| !m.file_type().is_symlink() && m.is_dir())
        {
            count = count.saturating_add(1);
        }
    }
    count
}

fn unavailable_copy(
    machine: &str,
    path: &str,
    vcs: Option<LocalVcs>,
    now_ms: u64,
    error: &str,
) -> FleetLocalCopy {
    FleetLocalCopy {
        machine: machine.to_owned(),
        path: path.to_owned(),
        vcs,
        head_commit: None,
        branch_or_bookmark: None,
        dirty: None,
        ahead: None,
        behind: None,
        remotes: Vec::new(),
        cairn_count: None,
        observed_at_ms: Some(now_ms),
        health: CopyHealth::Unavailable,
        error: Some(truncate_text(error, MAX_ERROR_BYTES)),
    }
}

fn truncate_text(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_owned();
    }
    let mut end = 0;
    for (idx, ch) in input.char_indices() {
        let next = idx + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    input.get(..end).unwrap_or_default().to_owned()
}

fn json_api_response<T: Serialize>(status: u16, value: &T) -> ApiResponse {
    match serde_json::to_vec(value) {
        Ok(body) if body.len() <= MAX_RESPONSE_BYTES => ApiResponse {
            status,
            content_type: "application/json",
            body,
            download_name: None,
            headers: vec![("Cache-Control", "no-store".to_owned())],
        },
        _ => error_json(
            503,
            "repository_response_too_large",
            "repository response exceeded bounded size",
        ),
    }
}

fn error_json(status: u16, code: &str, message: &str) -> ApiResponse {
    let body = json!({ "code": code, "message": message })
        .to_string()
        .into_bytes();
    ApiResponse {
        status,
        content_type: "application/json",
        body,
        download_name: None,
        headers: vec![("Cache-Control", "no-store".to_owned())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::os::unix::fs::PermissionsExt;

    fn test_profile(
        dir: &Path,
        repo_path: &Path,
    ) -> Result<ServiceProfile, Box<dyn std::error::Error>> {
        let op_tok = dir.join("op.token");
        let wk_tok = dir.join("wk.token");
        fs::write(&op_tok, "a".repeat(64))?;
        fs::write(&wk_tok, "b".repeat(64))?;
        fs::set_permissions(&op_tok, fs::Permissions::from_mode(0o400))?;
        fs::set_permissions(&wk_tok, fs::Permissions::from_mode(0o400))?;
        let cert = dir.join("cert.pem");
        let key = dir.join("key.pem");
        fs::write(&cert, "dummy")?;
        fs::write(&key, "dummy")?;
        Ok(serde_json::from_value(json!({
            "schema": "kiln.web-service.v1",
            "service_id": "repo-unit-test",
            "listen_address": "127.0.0.1:9443",
            "origin": "https://localhost:9443",
            "tls": {"certificate_path": cert, "private_key_path": key, "min_version": "TLS1.3"},
            "operator_token_file": op_tok,
            "worker_token_file": wk_tok,
            "state_directory": dir.join("state"),
            "projects": [{
                "id": "fixture",
                "label": "Fixture",
                "repository_path": repo_path,
                "reference": "refs/heads/main",
                "source_limits": {"max_entries": 32, "max_path_bytes": 1024, "max_file_bytes": 4096, "max_total_bytes": 16384},
                "workflow": {
                    "socket_path": dir.join("lattice.sock"),
                    "allowed_user_ids": [geteuid().as_raw()],
                    "transport_limits": {"maximum_frame_bytes": 1048576, "maximum_log_bytes": 65536, "maximum_connections": 4, "read_deadline_milliseconds": 1000, "write_deadline_milliseconds": 1000},
                    "adapter_limits": {"maximum_request_bytes": 1048576, "maximum_response_bytes": 1048576},
                    "policy_id": "reviewed-policy",
                    "provider_profile_id": "reviewed-lattice",
                    "platform": "linux-x86_64",
                    "nodes": [{"id": "job", "kind": "test", "effect_class": "test", "adapter_kind": "local", "operation_identity": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],
                    "edges": [],
                    "controls": {"cancel": true, "pause": false, "resume": false},
                    "bounds": {"max_nodes": 8, "max_edges": 16, "max_log_references": 32, "max_log_read_bytes": 4096, "max_total_log_bytes": 65536}
                }
            }],
            "bounds": {
                "max_request_bytes": 16384, "max_response_bytes": 1048576, "max_header_bytes": 4096,
                "max_connections": 8, "timeout_seconds": 2, "poll_interval_ms": 10, "max_projects": 2,
                "max_runs": 4, "max_artifact_bytes": 1024, "max_artifacts": 2, "max_run_artifact_bytes": 2048,
                "max_total_artifact_bytes": 4096, "max_log_read_bytes": 4096, "max_requests": 32,
                "max_events_per_run": 8, "max_state_bytes": 1048576
            }
        }))?)
    }

    #[test]
    fn import_mirror_replay_and_remove_preserves_disk_checkout()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let repo_dir = dir.path().join("kiln");
        let repo = Repository::init(&repo_dir)?;
        fs::write(repo_dir.join("README.md"), b"hello")?;
        let mut idx = repo.index()?;
        idx.add_path(Path::new("README.md"))?;
        let tree_id = idx.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let sig = git2::Signature::now("t", "t@example.invalid")?;
        repo.commit(Some("refs/heads/main"), &sig, &sig, "init", &tree, &[])?;
        repo.remote("origin", "https://github.com/OnixResearch/kiln.git")?;
        fs::create_dir_all(repo_dir.join(".cairn/changes/my-change"))?;

        let profile = test_profile(dir.path(), &repo_dir)?;
        let service = CatalogService::new(&profile, Vec::new(), Vec::new());

        let import_body = json!({
            "provider": "radicle",
            "role": "canonical",
            "url": "rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5",
            "label": "Kiln",
            "request_id": "req-import-1"
        })
        .to_string();
        let resp = service.response(
            "POST",
            "/api/repositories/import",
            import_body.as_bytes(),
            Role::Operator,
            None,
        );
        assert_eq!(resp.status, 201);
        let parsed: Value = serde_json::from_slice(&resp.body)?;
        assert_eq!(
            parsed.get("action").and_then(Value::as_str),
            Some("imported")
        );
        let repo_id = parsed
            .get("repository")
            .and_then(|r| r.get("identity"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        assert_eq!(repo_id.len(), 64);

        // Replay identical request_id -> idempotent 200 with already_recorded: true
        let replay = service.response(
            "POST",
            "/api/repositories/import",
            import_body.as_bytes(),
            Role::Operator,
            None,
        );
        assert_eq!(replay.status, 200);
        let replay_val: Value = serde_json::from_slice(&replay.body)?;
        assert_eq!(
            replay_val.get("already_recorded").and_then(Value::as_bool),
            Some(true)
        );

        // Conflicting reuse of request_id -> 409
        let conflict_body = json!({
            "provider": "github",
            "role": "mirror",
            "url": "OnixResearch/kiln",
            "request_id": "req-import-1"
        })
        .to_string();
        let conflict = service.response(
            "POST",
            "/api/repositories/import",
            conflict_body.as_bytes(),
            Role::Operator,
            None,
        );
        assert_eq!(conflict.status, 409);

        // Attach GitHub mirror
        let mirror_body = json!({
            "provider": "github",
            "role": "mirror",
            "url": "https://github.com/OnixResearch/kiln",
            "request_id": "req-mirror-1"
        })
        .to_string();
        let attach = service.response(
            "POST",
            &format!("/api/repositories/{repo_id}/mirrors"),
            mirror_body.as_bytes(),
            Role::Operator,
            None,
        );
        assert_eq!(attach.status, 200);
        let attach_val: Value = serde_json::from_slice(&attach.body)?;
        assert_eq!(
            attach_val
                .get("repository")
                .and_then(|r| r.get("remotes"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            attach_val
                .get("repository")
                .and_then(|r| r.get("admission"))
                .and_then(|a| a.get("ci_configured"))
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            attach_val
                .get("repository")
                .and_then(|r| r.get("total_cairns"))
                .and_then(Value::as_u64),
            Some(1)
        );

        // Remove registration -> disk checkout remains intact!
        let remove_body = json!({
            "reason": "no longer tracked",
            "request_id": "req-remove-1"
        })
        .to_string();
        let removed = service.response(
            "POST",
            &format!("/api/repositories/{repo_id}/remove"),
            remove_body.as_bytes(),
            Role::Operator,
            None,
        );
        assert_eq!(removed.status, 200);
        assert!(repo_dir.join("README.md").exists());
        Ok(())
    }
}
