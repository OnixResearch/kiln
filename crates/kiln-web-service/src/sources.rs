//! Read-only local source observation for Git, Jujutsu and Pijul repositories.
//!
//! Git and Jujutsu are read through the repository's Git object store. Pijul is
//! read through a separately executed `pijul` program, never a linked library,
//! and only for its current channel state. Nothing here mutates a repository,
//! resolves a remote, or grants CI, gate or release meaning.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use bounded_exec::{CommandSpec, EnvironmentMode, Input, RunRequest, TerminationScope};
use bounded_exec_core::{ExecutionLimits, OutcomePolicy};
use git2::{ObjectType, Oid, Repository, RepositoryOpenFlags};
use kiln_core::implementation::authority::AuthoritySessionId;
use kiln_core::projection::authority::{
    EffectiveSourceScope, OriginScope, PathScope, SnapshotRule, SourceAuthority,
    SourceAuthorityRequest, SourceCurrentness, SourceViewOperation, check_request,
};
use kiln_core::projection::disclosure::{
    AudienceView, HandleKey, OpaqueHandle, OriginLookup, ScopedEnumeration, ScopedSelection,
    ViewCeiling, permits_unfiltered_history, restrict,
};
use kiln_core::projection::{
    AdmittedDefinition, AdmittedPartialView, AdmittedRepository, EntryOrigin, ProjectionFacts,
    resolve,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engine::api::{decoded, error, json_response, query};
use crate::engine::{ApiResponse, Role};
use crate::worker::source_auth::RequestProfile;

pub const MAX_SOURCES: usize = 64;
const MAX_POLICY_BYTES: usize = 262_144;
const MAX_QUERY_BYTES: usize = 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_LOG_LIMIT: usize = 30;
const MAX_LOG_LIMIT: usize = 100;
const MAX_TREE_ENTRIES: usize = 512;
const MAX_FILE_BYTES: usize = 262_144;
const MAX_PATH_BYTES: usize = 1024;
const MAX_SUMMARY_CHARS: usize = 256;
const MAX_AUTHOR_CHARS: usize = 256;
const MAX_COMMIT_BYTES: usize = 65_536;
const MAX_TREE_BYTES: usize = 65_536;
const MAX_DIFF_BYTES: usize = 262_144;
const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 50;
const MAX_SEARCH_QUERY_BYTES: usize = 256;
const MAX_SEARCH_MATCHES: usize = 200;
const MAX_SNIPPETS_PER_FILE: usize = 3;
const MAX_SNIPPET_CHARS: usize = 200;
const SEARCH_CURSOR_DOMAIN: &str = "kiln.source-view-search-cursor.v1";
const PIJUL_TIMEOUT_MS: u64 = 15_000;
const PIJUL_POLL_MS: u64 = 25;
const PIJUL_TEARDOWN_MS: u64 = 2_000;
const PIJUL_OUTPUT_BYTES: usize = 1_048_576;
const PIJUL_ERROR_BYTES: usize = 8_192;

const KINDS: [&str; 3] = ["git", "jj", "pijul"];

const NON_CLAIMS: [&str; 3] = [
    "Source views are bounded local reads of one repository; they are not a checkout, build, verification or provenance claim.",
    "Git and Jujutsu views resolve an exact commit in the repository's Git object store; Pijul views cover its current channel state and working copy only.",
    "Browsing never fetches, mutates, signs, publishes or grants CI, gate, acceptance or release authority.",
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePolicy {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub path: PathBuf,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub executable: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub sources: Vec<SourcePolicy>,
}

impl Profile {
    // r[impl kiln.dashboard.source.authority]
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != "kiln.web-sources.v1" || self.sources.len() > MAX_SOURCES {
            return Err("source policy schema or repository bound rejected".into());
        }
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for source in &self.sources {
            if !crate::profile::identifier(&source.id)
                || source.id.len() > 128
                || source.label.is_empty()
                || source.label.len() > 128
                || source.label.chars().any(char::is_control)
                || !KINDS.contains(&source.kind.as_str())
                || !crate::profile::absolute(&source.path)
                || !ids.insert(&source.id)
                || !paths.insert(&source.path)
            {
                return Err("source repository policy rejected".into());
            }
            match source.kind.as_str() {
                "pijul" => {
                    let channel = source.channel.as_deref().unwrap_or_default();
                    if !crate::profile::identifier(channel) || channel.len() > 128 {
                        return Err("Pijul source requires a bounded channel name".into());
                    }
                    if source
                        .executable
                        .as_deref()
                        .is_some_and(|path| !crate::profile::absolute(path))
                    {
                        return Err("Pijul executable must be an absolute path".into());
                    }
                }
                _ if source.channel.is_some() || source.executable.is_some() => {
                    return Err("Git and Jujutsu sources take no channel or executable".into());
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, MAX_POLICY_BYTES, false)
            .map_err(|_| "source policy unavailable or unsafe")?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "source policy malformed")?;
        profile.admit()?;
        Ok(profile)
    }
}

/// Whether a [`Browser`] runs in local unrestricted mode or enforces
/// capability-restricted projection views (`SourceAuthority` + `AudienceView`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserMode {
    LocalUnrestricted,
    RestrictedProjection,
}

/// Pure projection facts and currentness for one source repository under
/// restricted projection mode.
#[derive(Debug)]
pub struct RestrictedSourceRepo<'a> {
    pub policy_id: &'a str,
    pub admitted: &'a AdmittedRepository,
    pub definition: &'a AdmittedDefinition,
    pub facts: &'a ProjectionFacts,
    pub current: &'a SourceCurrentness,
}

/// Caller-supplied `SourceAuthority`, credential/signer, repository projections,
/// handle key, and wall clock for restricted source browsing.
pub struct RestrictedSourceAccess<'a> {
    pub authority: &'a mut dyn SourceAuthority,
    pub profile: RequestProfile<'a>,
    pub repos: &'a [RestrictedSourceRepo<'a>],
    pub handle_key: &'a HandleKey,
    pub now_unix_s: u64,
}

impl std::fmt::Debug for RestrictedSourceAccess<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RestrictedSourceAccess")
            .field("repos", &self.repos.len())
            .field("now_unix_s", &self.now_unix_s)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct Browser {
    profile: Option<Profile>,
    mode: BrowserMode,
}

impl Browser {
    pub fn new(profile: Option<Profile>) -> Result<Self, String> {
        if let Some(profile) = &profile {
            profile.admit()?;
        }
        Ok(Self {
            profile,
            mode: BrowserMode::LocalUnrestricted,
        })
    }

    /// Construct a restricted-projection browser. Un-scoped [`Self::response`]
    /// calls fail closed with `PublicUnavailable` (`{"error":"unavailable"}`);
    /// callers must use [`Self::restricted_response`] with a live
    /// [`RestrictedSourceAccess`].
    pub fn restricted(profile: Option<Profile>) -> Result<Self, String> {
        if let Some(profile) = &profile {
            profile.admit()?;
        }
        Ok(Self {
            profile,
            mode: BrowserMode::RestrictedProjection,
        })
    }

    #[must_use]
    pub const fn mode(&self) -> BrowserMode {
        self.mode
    }

    // r[impl kiln.dashboard.source.read] r[impl kiln.dashboard.source.authority]
    pub fn response(&self, method: &str, target: &str, body: &[u8], role: Role) -> ApiResponse {
        if self.mode == BrowserMode::RestrictedProjection {
            if role == Role::Worker {
                return error(403, "worker_scope").unwrap_or_else(|_| public_unavailable());
            }
            if method != "GET" {
                return error(405, "sources_read_only").unwrap_or_else(|_| public_unavailable());
            }
            return public_unavailable();
        }
        match self.read(method, target, body, role) {
            Ok(response) if response.body.len() <= MAX_RESPONSE_BYTES => response,
            _ => ApiResponse {
                status: 503,
                content_type: "application/json",
                body: br#"{"code":"source_read_unavailable","message":"Source read exceeded its bounds or is unavailable"}"#.to_vec(),
                download_name: None,
                headers: Vec::new(),
            },
        }
    }

    // r[impl kiln.source.projection.disclosure.restricted]
    // r[impl kiln.source.projection.disclosure.handles]
    // r[impl kiln.source.projection.disclosure.unavailable]
    /// Serve `/api/sources*` strictly through `SourceAuthority` (`Discover`,
    /// `ReadContent`, `ReadHistory`), `ViewCeiling`, `ScopedSelection`, and
    /// `AudienceView::restrict`/`resolve`. Missing, forbidden, expired, revoked,
    /// or stale requests all return uniform [`PublicUnavailable`]
    /// (`{"error":"unavailable"}`) with status `404` so hidden repositories and
    /// paths never leak via a 403/404 oracle.
    pub fn restricted_response(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> ApiResponse {
        match self.read_restricted(method, target, body, role, access) {
            Ok(response) if response.body.len() <= MAX_RESPONSE_BYTES => response,
            Ok(_) => public_unavailable(),
            Err(response) => response,
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
            return error(405, "sources_read_only");
        }
        if !body.is_empty() {
            return error(400, "unexpected_body");
        }
        let (path, parameters) = match query(target) {
            Ok(value) => value,
            Err(()) => return error(400, "query_invalid"),
        };
        let mut values = BTreeMap::new();
        for (key, value) in parameters {
            match decoded(value) {
                Ok(value)
                    if value.len() <= MAX_QUERY_BYTES && !value.chars().any(char::is_control) =>
                {
                    values.insert(key.to_owned(), value);
                }
                _ => return error(400, "query_invalid"),
            }
        }
        if path == "/api/sources" {
            if !values.is_empty() {
                return error(400, "query_invalid");
            }
            return self.inventory();
        }
        if path == "/api/sources/lookup" {
            if values.keys().any(|key| key != "revision") {
                return error(400, "query_invalid");
            }
            let revision = values.get("revision").map_or("", String::as_str);
            if !is_revision(revision) {
                return error(400, "source_revision_invalid");
            }
            return self.lookup(revision);
        }
        let Some(rest) = path.strip_prefix("/api/sources/") else {
            return error(404, "route_missing");
        };
        let Some((id, leaf)) = rest.split_once('/') else {
            return error(404, "route_missing");
        };
        let allowed: &[&str] = match leaf {
            "log" => &["limit", "cursor"],
            "tree" => &["revision", "path"],
            "file" => &["revision", "path", "highlight"],
            "diff" => &["revision", "highlight"],
            "search" => &["q", "revision", "cursor", "limit"],
            "download" => &["revision", "path"],
            _ => return error(404, "route_missing"),
        };
        if values.keys().any(|key| !allowed.contains(&key.as_str())) {
            return error(400, "query_invalid");
        }
        // r[impl kiln.highlight.api]
        let highlight = match values.get("highlight").map(String::as_str) {
            None => false,
            Some("1") => true,
            Some(_) => return error(400, "query_invalid"),
        };
        let Some(profile) = &self.profile else {
            return error(404, "source_missing");
        };
        let Some(policy) = profile.sources.iter().find(|row| row.id == id) else {
            return error(404, "source_missing");
        };
        match leaf {
            "log" => self.log(policy, &values),
            "tree" => self.tree(policy, &values),
            "diff" => self.diff(policy, &values, highlight),
            "search" => self.search(policy, &values),
            "download" => self.download(policy, &values),
            _ => self.file(policy, &values, highlight),
        }
    }

    /// Report which admitted sources actually contain one exact commit.
    ///
    /// This is how a recorded patch commit is linked to a readable diff; a
    /// source that lacks the object is reported by omission, never guessed.
    fn lookup(&self, revision: &str) -> Result<ApiResponse, String> {
        let Some(profile) = &self.profile else {
            return json_response(
                200,
                json!({"revision":revision,"matches":[],"non_claims":NON_CLAIMS}),
            );
        };
        let oid = Oid::from_str(revision).map_err(|_| "revision is not an object id")?;
        let mut matches = Vec::new();
        for policy in &profile.sources {
            if policy.kind == "pijul" {
                continue;
            }
            let Ok(repository) = open_git(policy) else {
                continue;
            };
            if repository.find_commit(oid).is_ok() {
                matches.push(json!({"id":policy.id,"label":policy.label,"kind":policy.kind}));
            }
        }
        json_response(
            200,
            json!({"revision":revision,"matches":matches,"non_claims":NON_CLAIMS}),
        )
    }

    fn inventory(&self) -> Result<ApiResponse, String> {
        let Some(profile) = &self.profile else {
            return json_response(
                200,
                json!({"configured":false,"sources":[],"non_claims":NON_CLAIMS}),
            );
        };
        let sources = profile
            .sources
            .iter()
            .map(|policy| {
                let observed = observe(policy);
                json!({
                    "id": policy.id,
                    "label": policy.label,
                    "kind": policy.kind,
                    "channel": policy.channel,
                    "status": if observed.head.is_some() { "available" } else { "unavailable" },
                    "head": observed.head,
                    "revision_scope": observed.scope,
                    "reason": observed.reason,
                })
            })
            .collect::<Vec<_>>();
        json_response(
            200,
            json!({"configured":true,"sources":sources,"non_claims":NON_CLAIMS}),
        )
    }

    fn log(
        &self,
        policy: &SourcePolicy,
        values: &BTreeMap<String, String>,
    ) -> Result<ApiResponse, String> {
        let limit = match values.get("limit") {
            None => DEFAULT_LOG_LIMIT,
            Some(value) => match value.parse::<usize>() {
                Ok(limit) if (1..=MAX_LOG_LIMIT).contains(&limit) => limit,
                _ => return error(400, "source_limit_invalid"),
            },
        };
        let offset = match values.get("cursor") {
            None => 0,
            Some(value) => match value.parse::<usize>() {
                Ok(offset) if offset <= 100_000 => offset,
                _ => return error(400, "source_cursor_invalid"),
            },
        };
        match policy.kind.as_str() {
            "pijul" => pijul_log(policy, limit, offset),
            _ => git_log(policy, limit, offset),
        }
    }

    fn tree(
        &self,
        policy: &SourcePolicy,
        values: &BTreeMap<String, String>,
    ) -> Result<ApiResponse, String> {
        let revision = values.get("revision").map_or("", String::as_str);
        let path = values.get("path").map_or("", String::as_str);
        let components = match components(path) {
            Ok(components) => components,
            Err(()) => return error(400, "source_path_invalid"),
        };
        match policy.kind.as_str() {
            "pijul" => pijul_tree(policy, revision, &components),
            _ => git_tree(policy, revision, &components),
        }
    }

    fn diff(
        &self,
        policy: &SourcePolicy,
        values: &BTreeMap<String, String>,
        highlight: bool,
    ) -> Result<ApiResponse, String> {
        let revision = values.get("revision").map_or("", String::as_str);
        match policy.kind.as_str() {
            "pijul" => error(400, "source_diff_unsupported"),
            _ => git_diff(policy, revision, highlight),
        }
    }

    fn file(
        &self,
        policy: &SourcePolicy,
        values: &BTreeMap<String, String>,
        highlight: bool,
    ) -> Result<ApiResponse, String> {
        let revision = values.get("revision").map_or("", String::as_str);
        let path = values.get("path").map_or("", String::as_str);
        let components = match components(path) {
            Ok(components) => components,
            Err(()) => return error(400, "source_path_invalid"),
        };
        if components.is_empty() {
            return error(400, "source_path_invalid");
        }
        match policy.kind.as_str() {
            "pijul" => pijul_file(policy, revision, &components, highlight),
            _ => git_file(policy, revision, &components, highlight),
        }
    }

    fn download(
        &self,
        policy: &SourcePolicy,
        values: &BTreeMap<String, String>,
    ) -> Result<ApiResponse, String> {
        let revision = values.get("revision").map_or("", String::as_str);
        let path = values.get("path").map_or("", String::as_str);
        let components = match components(path) {
            Ok(components) => components,
            Err(()) => return error(400, "source_path_invalid"),
        };
        if components.is_empty() {
            return error(400, "source_path_invalid");
        }
        let bytes = match policy.kind.as_str() {
            "pijul" => pijul_raw_file(policy, revision, &components)?,
            _ => git_raw_file(policy, revision, &components)?,
        };
        Ok(raw_download_response(path, bytes))
    }

    fn search(
        &self,
        policy: &SourcePolicy,
        values: &BTreeMap<String, String>,
    ) -> Result<ApiResponse, String> {
        let query_text = values.get("q").map_or("", String::as_str);
        if query_text.trim().is_empty()
            || query_text.len() > MAX_SEARCH_QUERY_BYTES
            || query_text.chars().any(char::is_control)
        {
            return error(400, "source_search_invalid");
        }
        let limit = match values.get("limit") {
            None => DEFAULT_SEARCH_LIMIT,
            Some(value) => match value.parse::<usize>() {
                Ok(limit) if (1..=MAX_SEARCH_LIMIT).contains(&limit) => limit,
                _ => return error(400, "source_limit_invalid"),
            },
        };
        let offset = match values.get("cursor") {
            None => 0,
            Some(value) => match value.parse::<usize>() {
                Ok(offset) if offset <= MAX_SEARCH_MATCHES => offset,
                _ => return error(400, "source_cursor_invalid"),
            },
        };
        let revision = values.get("revision").map_or("", String::as_str);
        match policy.kind.as_str() {
            "pijul" => pijul_search(policy, revision, query_text, limit, offset),
            _ => git_search(policy, revision, query_text, limit, offset),
        }
    }

    fn read_restricted(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> Result<ApiResponse, ApiResponse> {
        let err_resp = |status: u16, code: &str| -> ApiResponse {
            error(status, code).unwrap_or_else(|_| public_unavailable())
        };
        if role == Role::Worker {
            return Err(err_resp(403, "worker_scope"));
        }
        if method != "GET" {
            return Err(err_resp(405, "sources_read_only"));
        }
        if !body.is_empty() {
            return Err(err_resp(400, "unexpected_body"));
        }
        let (path, parameters) = query(target).map_err(|()| err_resp(400, "query_invalid"))?;
        let mut values = BTreeMap::new();
        for (key, value) in parameters {
            match decoded(value) {
                Ok(value)
                    if value.len() <= MAX_QUERY_BYTES && !value.chars().any(char::is_control) =>
                {
                    values.insert(key.to_owned(), value);
                }
                _ => return Err(err_resp(400, "query_invalid")),
            }
        }
        if path == "/api/sources" {
            if !values.is_empty() {
                return Err(err_resp(400, "query_invalid"));
            }
            return Ok(self.restricted_inventory(access));
        }
        if path == "/api/sources/lookup" {
            if values.keys().any(|key| key != "revision") {
                return Err(err_resp(400, "query_invalid"));
            }
            let revision = values.get("revision").map_or("", String::as_str);
            if !is_revision(revision) {
                return Err(err_resp(400, "source_revision_invalid"));
            }
            return Ok(self.restricted_lookup(revision, access));
        }
        let Some(rest) = path.strip_prefix("/api/sources/") else {
            return Err(public_unavailable());
        };
        let Some((id, leaf)) = rest.split_once('/') else {
            return Err(public_unavailable());
        };
        let allowed: &[&str] = match leaf {
            "log" => &["limit", "cursor"],
            "tree" => &["revision", "path"],
            "file" => &["revision", "path", "handle", "highlight"],
            "diff" => &["revision", "highlight"],
            "search" => &["q", "revision", "cursor", "limit"],
            "download" => &["revision", "path", "handle"],
            _ => return Err(public_unavailable()),
        };
        if values.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err(err_resp(400, "query_invalid"));
        }
        if (leaf == "file" || leaf == "download")
            && values.contains_key("handle")
            && values.contains_key("path")
        {
            return Err(err_resp(400, "query_invalid"));
        }
        let highlight = match values.get("highlight").map(String::as_str) {
            None => false,
            Some("1") => true,
            Some(_) => return Err(err_resp(400, "query_invalid")),
        };
        let Some(profile) = &self.profile else {
            return Err(public_unavailable());
        };
        let Some(policy) = profile.sources.iter().find(|row| row.id == id) else {
            return Err(public_unavailable());
        };
        let Some(repo) = access.repos.iter().find(|r| r.policy_id == id) else {
            return Err(public_unavailable());
        };
        match leaf {
            "log" => self.restricted_log(policy, repo, &values, access),
            "tree" => self.restricted_tree(policy, repo, &values, access),
            "diff" => self.restricted_diff(policy, repo, &values, highlight, access),
            "search" => self.restricted_search(policy, repo, &values, access),
            "download" => self.restricted_download(policy, repo, &values, access),
            _ => self.restricted_file(policy, repo, &values, highlight, access),
        }
    }

    fn restricted_inventory(&self, access: &mut RestrictedSourceAccess<'_>) -> ApiResponse {
        let Some(profile) = &self.profile else {
            return json_response(
                200,
                json!({"configured":false,"sources":[],"non_claims":NON_CLAIMS}),
            )
            .unwrap_or_else(|_| public_unavailable());
        };
        let mut sources = Vec::new();
        for policy in &profile.sources {
            let Some(repo) = access.repos.iter().find(|r| r.policy_id == policy.id) else {
                continue;
            };
            let Some((_, view, _)) = admit_best_discovery_view(repo, b"inventory", access) else {
                continue;
            };
            let exact = repo.facts.exact_revision.native_id.as_str();
            let (available, head_value, reason) = if policy.kind == "pijul" {
                match pijul_head(policy) {
                    Ok(head) if head == exact => (true, Some(exact), None),
                    Ok(_) => (false, None, Some("pinned revision unavailable")),
                    Err(reason) => (false, None, Some(reason)),
                }
            } else {
                match open_git(policy).and_then(|repository| commit(&repository, exact).map(|_| ()))
                {
                    Ok(()) => (true, Some(exact), None),
                    Err(reason) => (false, None, Some(reason)),
                }
            };
            sources.push(json!({
                "id": policy.id,
                "label": policy.label,
                "kind": policy.kind,
                "channel": policy.channel,
                "status": if available { "available" } else { "unavailable" },
                "head": head_value,
                "revision_scope": if policy.kind == "pijul" { "current_channel_state" } else { "revision" },
                "reason": reason,
                "view": view.presentation(),
            }));
        }
        json_response(
            200,
            json!({"configured":true,"sources":sources,"non_claims":NON_CLAIMS}),
        )
        .unwrap_or_else(|_| public_unavailable())
    }

    fn restricted_lookup(
        &self,
        revision: &str,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> ApiResponse {
        let Some(profile) = &self.profile else {
            return json_response(
                200,
                json!({"revision":revision,"matches":[],"non_claims":NON_CLAIMS}),
            )
            .unwrap_or_else(|_| public_unavailable());
        };
        let Ok(oid) = Oid::from_str(revision) else {
            return public_unavailable();
        };
        let mut matches = Vec::new();
        for policy in &profile.sources {
            if policy.kind == "pijul" {
                continue;
            }
            let Some(repo) = access.repos.iter().find(|r| r.policy_id == policy.id) else {
                continue;
            };
            let ceiling = ViewCeiling::from_definition(repo.definition);
            if !ceiling
                .operations()
                .contains(&SourceViewOperation::ReadHistory)
            {
                continue;
            }
            let Some(scope) = authorize_source_op(
                repo,
                SourceViewOperation::ReadHistory,
                PathScope::root(),
                revision.as_bytes(),
                access,
            ) else {
                continue;
            };
            if !scope.permits_scope(SourceViewOperation::ReadHistory, &PathScope::root()) {
                continue;
            }
            let Ok(repository) = open_git(policy) else {
                continue;
            };
            let Ok(exact_oid) = Oid::from_str(repo.facts.exact_revision.native_id.as_str()) else {
                continue;
            };
            if oid == exact_oid
                || repository
                    .graph_descendant_of(exact_oid, oid)
                    .unwrap_or(false)
            {
                matches.push(json!({"id":policy.id,"label":policy.label,"kind":policy.kind}));
            }
        }
        json_response(
            200,
            json!({"revision":revision,"matches":matches,"non_claims":NON_CLAIMS}),
        )
        .unwrap_or_else(|_| public_unavailable())
    }

    fn restricted_log(
        &self,
        policy: &SourcePolicy,
        repo: &RestrictedSourceRepo<'_>,
        values: &BTreeMap<String, String>,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> Result<ApiResponse, ApiResponse> {
        let ceiling = ViewCeiling::from_definition(repo.definition);
        if !ceiling
            .operations()
            .contains(&SourceViewOperation::ReadHistory)
        {
            return Err(public_unavailable());
        }
        let Some(scope) = authorize_source_op(
            repo,
            SourceViewOperation::ReadHistory,
            PathScope::root(),
            b"log",
            access,
        ) else {
            return Err(public_unavailable());
        };
        if !scope.permits_scope(SourceViewOperation::ReadHistory, &PathScope::root()) {
            return Err(public_unavailable());
        }
        let limit = match values.get("limit") {
            None => DEFAULT_LOG_LIMIT,
            Some(value) => match value.parse::<usize>() {
                Ok(limit) if (1..=MAX_LOG_LIMIT).contains(&limit) => limit,
                _ => return Err(public_unavailable()),
            },
        };
        let offset = match values.get("cursor") {
            None => 0,
            Some(value) => match value.parse::<usize>() {
                Ok(offset) if offset <= 100_000 => offset,
                _ => return Err(public_unavailable()),
            },
        };
        let exact = repo.facts.exact_revision.native_id.as_str();
        let resp = if policy.kind == "pijul" {
            if pijul_head(policy).ok().as_deref() != Some(exact) {
                return Err(public_unavailable());
            }
            pijul_log(policy, limit, offset)
        } else {
            git_log_at(policy, Some(exact), limit, offset)
        }
        .map_err(|_| public_unavailable())?;
        if resp.status != 200 {
            return Err(public_unavailable());
        }
        Ok(resp)
    }

    fn restricted_tree(
        &self,
        policy: &SourcePolicy,
        repo: &RestrictedSourceRepo<'_>,
        values: &BTreeMap<String, String>,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> Result<ApiResponse, ApiResponse> {
        let revision = values.get("revision").map_or("", String::as_str);
        let path = values.get("path").map_or("", String::as_str);
        let Ok(components) = components(path) else {
            return Err(public_unavailable());
        };
        if PathScope::from_path(path, true).is_none() {
            return Err(public_unavailable());
        }
        let pinned_rev = pin_restricted_revision(policy, repo, revision)?;
        let Some((scope, view, partial)) = admit_best_discovery_view(repo, path.as_bytes(), access)
        else {
            return Err(public_unavailable());
        };
        let mut visible_origins: BTreeSet<&str> = BTreeSet::new();
        let mut readable_origins: BTreeSet<&str> = BTreeSet::new();
        for entry in view
            .entries()
            .iter()
            .filter(|entry| entry.display_path.is_some())
        {
            if let Ok(manifest_entry) = view.resolve(
                &partial,
                &scope,
                access.now_unix_s,
                &entry.handle,
                SourceViewOperation::Discover,
            ) && let EntryOrigin::Native { path, .. } = &manifest_entry.origin
            {
                visible_origins.insert(path.as_str());
                if entry.readable {
                    readable_origins.insert(path.as_str());
                }
            }
        }
        if !path.is_empty() {
            let prefix = format!("{path}/");
            let has_visible_descendant = visible_origins.iter().any(|p| p.starts_with(&prefix));
            if !has_visible_descendant {
                return Err(public_unavailable());
            }
        }
        let response = match policy.kind.as_str() {
            "pijul" => pijul_tree_filtered(policy, pinned_rev, &components, Some(&visible_origins)),
            _ => git_tree_filtered(policy, pinned_rev, &components, Some(&visible_origins)),
        }
        .map_err(|_| public_unavailable())?;
        if response.status != 200 {
            return Err(public_unavailable());
        }
        let mut body: Value =
            serde_json::from_slice(&response.body).map_err(|_| public_unavailable())?;
        if let Some(entries) = body.get_mut("entries").and_then(Value::as_array_mut) {
            entries.retain(|entry| {
                let Some(entry_path) = entry.get("path").and_then(Value::as_str) else {
                    return false;
                };
                let entry_kind = entry.get("kind").and_then(Value::as_str);
                if entry_kind == Some("directory") {
                    let prefix = format!("{entry_path}/");
                    visible_origins.iter().any(|p| p.starts_with(&prefix))
                } else {
                    visible_origins.contains(entry_path)
                }
            });
            for entry in entries.iter_mut() {
                let is_dir = entry.get("kind").and_then(Value::as_str) == Some("directory");
                let is_readable = entry
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|p| readable_origins.contains(p));
                if let Some(obj) = entry.as_object_mut()
                    && (is_dir || !is_readable)
                {
                    obj.insert("oid".into(), Value::Null);
                    if !is_dir {
                        obj.insert("bytes".into(), Value::Null);
                    }
                }
            }
        }
        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "view".into(),
                serde_json::to_value(view.presentation()).map_err(|_| public_unavailable())?,
            );
        }
        json_response(200, body).map_err(|_| public_unavailable())
    }

    fn restricted_diff(
        &self,
        policy: &SourcePolicy,
        repo: &RestrictedSourceRepo<'_>,
        values: &BTreeMap<String, String>,
        highlight: bool,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> Result<ApiResponse, ApiResponse> {
        if policy.kind == "pijul" {
            return Err(public_unavailable());
        }
        let ceiling = ViewCeiling::from_definition(repo.definition);
        if !ceiling
            .operations()
            .contains(&SourceViewOperation::ReadHistory)
            || !permits_unfiltered_history(repo.definition)
        {
            return Err(public_unavailable());
        }
        let revision = values.get("revision").map_or("", String::as_str);
        let pinned_rev = pin_restricted_revision(policy, repo, revision)?;
        let Some(scope) = authorize_source_op(
            repo,
            SourceViewOperation::ReadHistory,
            PathScope::root(),
            pinned_rev.as_bytes(),
            access,
        ) else {
            return Err(public_unavailable());
        };
        // Only full-ceiling + root ReadHistory scopes may read whole-commit diffs
        // without leaking hidden paths.
        if !scope.permits_scope(SourceViewOperation::ReadHistory, &PathScope::root()) {
            return Err(public_unavailable());
        }
        let resp = git_diff(policy, pinned_rev, highlight).map_err(|_| public_unavailable())?;
        if resp.status != 200 {
            return Err(public_unavailable());
        }
        Ok(resp)
    }

    fn resolve_restricted_file_origin(
        &self,
        repo: &RestrictedSourceRepo<'_>,
        values: &BTreeMap<String, String>,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> Result<(String, String), ApiResponse> {
        let ceiling = ViewCeiling::from_definition(repo.definition);
        if let Some(handle_hex) = values.get("handle") {
            let Some(handle) = OpaqueHandle::from_hex(handle_hex) else {
                return Err(public_unavailable());
            };
            // Build the holder's AudienceView over their discover/read paths so we
            // can resolve the OpaqueHandle strictly via `AudienceView::resolve`.
            let Some((disc_scope, view, trusted)) =
                admit_best_discovery_view(repo, handle_hex.as_bytes(), access)
            else {
                return Err(public_unavailable());
            };
            let Ok(entry) = view.resolve(
                &trusted,
                &disc_scope,
                access.now_unix_s,
                &handle,
                SourceViewOperation::ReadContent,
            ) else {
                return Err(public_unavailable());
            };
            let display_name = view
                .entries()
                .iter()
                .find(|v| v.handle == handle)
                .and_then(|v| v.display_path.as_deref())
                .unwrap_or(entry.destination.as_str())
                .to_owned();
            let EntryOrigin::Native {
                path: target_origin,
                ..
            } = &entry.origin
            else {
                return Err(public_unavailable());
            };
            if !ceiling.allows(SourceViewOperation::ReadContent, target_origin) {
                return Err(public_unavailable());
            }
            let Some(exact_scope) = PathScope::from_path(target_origin, false) else {
                return Err(public_unavailable());
            };
            let Some(read_scope) = authorize_source_op(
                repo,
                SourceViewOperation::ReadContent,
                exact_scope,
                target_origin.as_bytes(),
                access,
            ) else {
                return Err(public_unavailable());
            };
            let selection = ScopedSelection::for_holder(&read_scope, &ceiling);
            let Ok(filtered) = selection.filter_facts(repo.facts) else {
                return Err(public_unavailable());
            };
            let Ok(partial) = resolve(repo.definition, repo.admitted, &filtered) else {
                return Err(public_unavailable());
            };
            let Ok(enumeration) = ScopedEnumeration::new(&selection, partial.clone()) else {
                return Err(public_unavailable());
            };
            let Ok(read_view) = restrict(
                &enumeration,
                &read_scope,
                &ceiling,
                access.handle_key,
                access.now_unix_s,
            ) else {
                return Err(public_unavailable());
            };
            let mut matched_origin = None;
            for candidate in read_view.entries().iter().filter(|e| e.readable) {
                if let Ok(resolved) = read_view.resolve(
                    &partial,
                    &read_scope,
                    access.now_unix_s,
                    &candidate.handle,
                    SourceViewOperation::ReadContent,
                ) && let EntryOrigin::Native { path, .. } = &resolved.origin
                    && path == target_origin
                {
                    matched_origin = Some(path.clone());
                    break;
                }
            }
            let Some(resolved_path) = matched_origin else {
                return Err(public_unavailable());
            };
            Ok((resolved_path, display_name))
        } else {
            let path = values.get("path").map_or("", String::as_str);
            if components(path).is_err() || path.is_empty() {
                return Err(public_unavailable());
            }
            let Some(exact_scope) = PathScope::from_path(path, false) else {
                return Err(public_unavailable());
            };
            if !ceiling.allows(SourceViewOperation::Discover, path)
                || !ceiling.allows(SourceViewOperation::ReadContent, path)
            {
                return Err(public_unavailable());
            }
            let Some(disc_scope) = authorize_source_op(
                repo,
                SourceViewOperation::Discover,
                exact_scope.clone(),
                path.as_bytes(),
                access,
            ) else {
                return Err(public_unavailable());
            };
            let Some(read_scope) = authorize_source_op(
                repo,
                SourceViewOperation::ReadContent,
                exact_scope,
                path.as_bytes(),
                access,
            ) else {
                return Err(public_unavailable());
            };
            let Ok(joined) = EffectiveSourceScope::join(&[disc_scope, read_scope]) else {
                return Err(public_unavailable());
            };
            let selection = ScopedSelection::for_holder(&joined, &ceiling);
            let Ok(filtered) = selection.filter_facts(repo.facts) else {
                return Err(public_unavailable());
            };
            let Ok(partial) = resolve(repo.definition, repo.admitted, &filtered) else {
                return Err(public_unavailable());
            };
            let Ok(enumeration) = ScopedEnumeration::new(&selection, partial.clone()) else {
                return Err(public_unavailable());
            };
            let Ok(view) = restrict(
                &enumeration,
                &joined,
                &ceiling,
                access.handle_key,
                access.now_unix_s,
            ) else {
                return Err(public_unavailable());
            };
            let Ok(OriginLookup::Entries(entries)) =
                view.lookup_origin(repo.definition, &joined, access.now_unix_s, path)
            else {
                return Err(public_unavailable());
            };
            let Some(entry) = entries.into_iter().find(|e| e.readable) else {
                return Err(public_unavailable());
            };
            let display_name = entry.display_path.clone().unwrap_or_else(|| path.to_owned());
            let Ok(manifest_entry) = view.resolve(
                &partial,
                &joined,
                access.now_unix_s,
                &entry.handle,
                SourceViewOperation::ReadContent,
            ) else {
                return Err(public_unavailable());
            };
            match &manifest_entry.origin {
                EntryOrigin::Native { path, .. } => Ok((path.clone(), display_name)),
                EntryOrigin::Generated { .. } => Err(public_unavailable()),
            }
        }
    }

    fn restricted_file(
        &self,
        policy: &SourcePolicy,
        repo: &RestrictedSourceRepo<'_>,
        values: &BTreeMap<String, String>,
        highlight: bool,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> Result<ApiResponse, ApiResponse> {
        let revision = values.get("revision").map_or("", String::as_str);
        let pinned_rev = pin_restricted_revision(policy, repo, revision)?;
        let (resolved_path, _) = self.resolve_restricted_file_origin(repo, values, access)?;
        let comps = components(&resolved_path).map_err(|()| public_unavailable())?;
        let resp = match policy.kind.as_str() {
            "pijul" => pijul_file(policy, pinned_rev, &comps, highlight),
            _ => git_file(policy, pinned_rev, &comps, highlight),
        }
        .map_err(|_| public_unavailable())?;
        if resp.status != 200 {
            return Err(public_unavailable());
        }
        Ok(resp)
    }

    fn restricted_download(
        &self,
        policy: &SourcePolicy,
        repo: &RestrictedSourceRepo<'_>,
        values: &BTreeMap<String, String>,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> Result<ApiResponse, ApiResponse> {
        let revision = values.get("revision").map_or("", String::as_str);
        let pinned_rev = pin_restricted_revision(policy, repo, revision)?;
        let (resolved_path, display_name) =
            self.resolve_restricted_file_origin(repo, values, access)?;
        let comps = components(&resolved_path).map_err(|()| public_unavailable())?;
        let bytes = match policy.kind.as_str() {
            "pijul" => pijul_raw_file(policy, pinned_rev, &comps),
            _ => git_raw_file(policy, pinned_rev, &comps),
        }
        .map_err(|_| public_unavailable())?;
        Ok(raw_download_response(&display_name, bytes))
    }

    fn restricted_search(
        &self,
        policy: &SourcePolicy,
        repo: &RestrictedSourceRepo<'_>,
        values: &BTreeMap<String, String>,
        access: &mut RestrictedSourceAccess<'_>,
    ) -> Result<ApiResponse, ApiResponse> {
        let query_text = values.get("q").map_or("", String::as_str);
        if query_text.trim().is_empty()
            || query_text.len() > MAX_SEARCH_QUERY_BYTES
            || query_text.chars().any(char::is_control)
        {
            return Err(public_unavailable());
        }
        let limit = match values.get("limit") {
            None => DEFAULT_SEARCH_LIMIT,
            Some(value) => match value.parse::<usize>() {
                Ok(limit) if (1..=MAX_SEARCH_LIMIT).contains(&limit) => limit,
                _ => return Err(public_unavailable()),
            },
        };
        let revision = values.get("revision").map_or("", String::as_str);
        let pinned_rev = pin_restricted_revision(policy, repo, revision)?;
        let Some((scope, view, partial)) =
            admit_best_discovery_view(repo, query_text.as_bytes(), access)
        else {
            return Err(public_unavailable());
        };
        if view.check_current(&scope, access.now_unix_s).is_err() {
            return Err(public_unavailable());
        }
        let offset = match values.get("cursor") {
            None => 0,
            Some(token) => decode_search_cursor(token, &view, query_text)
                .ok_or_else(public_unavailable)?,
        };
        let ceiling = ViewCeiling::from_definition(repo.definition);
        let needle = query_text.to_lowercase();
        let repository = if policy.kind == "pijul" {
            None
        } else {
            Some(open_git(policy).map_err(|_| public_unavailable())?)
        };
        let tree = match &repository {
            Some(git_repo) => {
                let commit_obj =
                    commit(git_repo, pinned_rev).map_err(|_| public_unavailable())?;
                Some(tree(git_repo, &commit_obj).map_err(|_| public_unavailable())?)
            }
            None => None,
        };

        let mut all_matches = Vec::new();
        for visible in view.entries() {
            let Some(display_path) = visible.display_path.as_deref() else {
                continue;
            };
            let Ok(manifest_entry) = view.resolve(
                &partial,
                &scope,
                access.now_unix_s,
                &visible.handle,
                SourceViewOperation::Discover,
            ) else {
                continue;
            };
            let EntryOrigin::Native { path: origin, .. } = &manifest_entry.origin else {
                continue;
            };
            let path_match = display_path.to_lowercase().contains(&needle);
            let mut snippets = Vec::new();
            let can_read = visible.readable
                && ceiling.allows(SourceViewOperation::ReadContent, origin)
                && scope.permits(SourceViewOperation::ReadContent, origin)
                && view
                    .resolve(
                        &partial,
                        &scope,
                        access.now_unix_s,
                        &visible.handle,
                        SourceViewOperation::ReadContent,
                    )
                    .is_ok();
            if can_read {
                let bytes_opt = match (&repository, &tree) {
                    (Some(git_repo), Some(git_tree)) => {
                        read_git_blob_bytes(git_repo, git_tree, origin).ok()
                    }
                    _ => components(origin)
                        .ok()
                        .and_then(|c| pijul_raw_file(policy, pinned_rev, &c).ok()),
                };
                if let Some(bytes) = bytes_opt {
                    snippets = search_snippets(&bytes, &needle);
                }
            }
            if path_match || !snippets.is_empty() {
                all_matches.push(json!({
                    "handle": visible.handle,
                    "path": display_path,
                    "readable": can_read,
                    "path_match": path_match,
                    "snippets": snippets,
                }));
                if all_matches.len() >= MAX_SEARCH_MATCHES {
                    break;
                }
            }
        }
        if offset > all_matches.len() {
            return Err(public_unavailable());
        }
        let end = offset.saturating_add(limit).min(all_matches.len());
        let page = all_matches[offset..end].to_vec();
        let next_cursor = (end < all_matches.len()).then(|| encode_search_cursor(&view, query_text, end));
        let exact_rev = if policy.kind == "pijul" {
            Value::Null
        } else {
            Value::String(pinned_rev.to_owned())
        };
        json_response(
            200,
            json!({
                "source": policy.id,
                "kind": policy.kind,
                "label": policy.label,
                "revision": exact_rev,
                "query": query_text,
                "matches": page,
                "total_matches": all_matches.len(),
                "truncated": all_matches.len() >= MAX_SEARCH_MATCHES,
                "next_cursor": next_cursor,
                "view": view.presentation(),
                "non_claims": NON_CLAIMS,
            }),
        )
        .map_err(|_| public_unavailable())
    }
}

fn public_unavailable() -> ApiResponse {
    ApiResponse {
        status: 404,
        content_type: "application/json",
        body: br#"{"error":"unavailable"}"#.to_vec(),
        download_name: None,
        headers: Vec::new(),
    }
}

fn source_request(
    repo: &RestrictedSourceRepo<'_>,
    operation: SourceViewOperation,
    path: PathScope,
    payload: &[u8],
) -> SourceAuthorityRequest {
    SourceAuthorityRequest {
        operation,
        repository: repo.admitted.identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: repo.facts.exact_revision.clone(),
        },
        view_binding: repo.definition.identity().clone(),
        scope: OriginScope::Components { path },
        request_digest: *blake3::hash(payload).as_bytes(),
        attempt: None,
    }
}

fn admit_source_op(
    repo: &RestrictedSourceRepo<'_>,
    operation: SourceViewOperation,
    path: PathScope,
    payload: &[u8],
    access: &mut RestrictedSourceAccess<'_>,
) -> Option<(
    AuthoritySessionId,
    SourceAuthorityRequest,
    EffectiveSourceScope,
)> {
    let req = source_request(repo, operation, path, payload);
    let owned_profile;
    let profile = match &access.profile {
        RequestProfile::Static(p) => *p,
        RequestProfile::Signer(sign) => {
            owned_profile = sign(&req).ok()?;
            &owned_profile
        }
    };
    let decision = access
        .authority
        .admit(&req, profile, repo.current, access.now_unix_s)
        .ok()?;
    let session = decision.session?;
    let scope = access.authority.effective_scope(session)?.clone();
    check_request(&scope, &req, repo.current, access.now_unix_s).ok()?;
    Some((session, req, scope))
}

fn authorize_source_op(
    repo: &RestrictedSourceRepo<'_>,
    operation: SourceViewOperation,
    path: PathScope,
    payload: &[u8],
    access: &mut RestrictedSourceAccess<'_>,
) -> Option<EffectiveSourceScope> {
    let (session, req, scope) = admit_source_op(repo, operation, path, payload, access)?;
    let revalid = access
        .authority
        .revalidate(session, &req, repo.current, access.now_unix_s)
        .is_ok();
    access.authority.close(session);
    revalid.then_some(scope)
}

fn pin_restricted_revision<'a>(
    policy: &SourcePolicy,
    repo: &'a RestrictedSourceRepo<'_>,
    requested: &str,
) -> Result<&'a str, ApiResponse> {
    let exact = repo.facts.exact_revision.native_id.as_str();
    if !requested.is_empty() && requested != exact {
        return Err(public_unavailable());
    }
    if policy.kind == "pijul" {
        if !requested.is_empty() || pijul_head(policy).ok().as_deref() != Some(exact) {
            return Err(public_unavailable());
        }
        return Ok("");
    }
    Ok(exact)
}

fn can_add_grant_path(grant_paths: &BTreeSet<PathScope>, path: &PathScope) -> bool {
    grant_paths.contains(path)
        || grant_paths.len() < kiln_core::projection::authority::MAX_SOURCE_GRANTS
}

fn push_source_scope(
    scopes: &mut Vec<EffectiveSourceScope>,
    grant_paths: &mut BTreeSet<PathScope>,
    scope: EffectiveSourceScope,
) -> bool {
    if scope.grants().is_empty() {
        return false;
    }
    let new_count = scope
        .grants()
        .iter()
        .filter(|grant| !grant_paths.contains(&grant.path))
        .count();
    if grant_paths.len().saturating_add(new_count)
        > kiln_core::projection::authority::MAX_SOURCE_GRANTS
    {
        return false;
    }
    for grant in scope.grants() {
        grant_paths.insert(grant.path.clone());
    }
    scopes.push(scope);
    true
}

fn collect_discover_scopes(
    repo: &RestrictedSourceRepo<'_>,
    payload: &[u8],
    access: &mut RestrictedSourceAccess<'_>,
    scopes: &mut Vec<EffectiveSourceScope>,
    grant_paths: &mut BTreeSet<PathScope>,
) {
    if let Some(root_scope) = authorize_source_op(
        repo,
        SourceViewOperation::Discover,
        PathScope::root(),
        payload,
        access,
    ) && push_source_scope(scopes, grant_paths, root_scope)
    {
        return;
    }
    let ceiling = ViewCeiling::from_definition(repo.definition);
    let mut denied_subtrees = BTreeSet::from([PathScope::root()]);
    for cpath in ceiling.paths() {
        if !can_add_grant_path(grant_paths, cpath) {
            continue;
        }
        if denied_subtrees.contains(cpath) {
            continue;
        }
        if let Some(scope) = authorize_source_op(
            repo,
            SourceViewOperation::Discover,
            cpath.clone(),
            payload,
            access,
        ) {
            let _ = push_source_scope(scopes, grant_paths, scope);
        } else if cpath.subtree {
            denied_subtrees.insert(cpath.clone());
        }
    }
    for entry in &repo.facts.entries {
        if grant_paths.len() >= kiln_core::projection::authority::MAX_SOURCE_GRANTS {
            break;
        }
        if !repo.definition.selects(&entry.path)
            || !ceiling.allows(SourceViewOperation::Discover, &entry.path)
            || scopes
                .iter()
                .any(|s| s.permits(SourceViewOperation::Discover, &entry.path))
        {
            continue;
        }
        let mut admitted = false;
        let parts: Vec<&str> = entry.path.split('/').collect();
        for end in 1..parts.len() {
            let prefix = parts
                .get(..end)
                .map(|slice| slice.join("/"))
                .unwrap_or_default();
            if let Some(dir_scope) = PathScope::from_path(&prefix, true)
                && ceiling.contains(&dir_scope)
                && !denied_subtrees.contains(&dir_scope)
                && can_add_grant_path(grant_paths, &dir_scope)
            {
                if let Some(scope) = authorize_source_op(
                    repo,
                    SourceViewOperation::Discover,
                    dir_scope.clone(),
                    payload,
                    access,
                ) {
                    if push_source_scope(scopes, grant_paths, scope) {
                        admitted = true;
                        break;
                    }
                } else {
                    denied_subtrees.insert(dir_scope);
                }
            }
        }
        if !admitted
            && let Some(entry_scope) = PathScope::from_path(&entry.path, false)
            && can_add_grant_path(grant_paths, &entry_scope)
            && let Some(scope) = authorize_source_op(
                repo,
                SourceViewOperation::Discover,
                entry_scope,
                payload,
                access,
            )
        {
            let _ = push_source_scope(scopes, grant_paths, scope);
        }
    }
}

fn collect_read_scopes(
    repo: &RestrictedSourceRepo<'_>,
    entries: &[kiln_core::projection::SourceEntry],
    payload: &[u8],
    access: &mut RestrictedSourceAccess<'_>,
    scopes: &mut Vec<EffectiveSourceScope>,
    grant_paths: &mut BTreeSet<PathScope>,
) {
    let ceiling = ViewCeiling::from_definition(repo.definition);
    if !ceiling
        .operations()
        .contains(&SourceViewOperation::ReadContent)
        || entries.is_empty()
    {
        return;
    }
    if can_add_grant_path(grant_paths, &PathScope::root())
        && let Some(root_scope) = authorize_source_op(
            repo,
            SourceViewOperation::ReadContent,
            PathScope::root(),
            payload,
            access,
        )
        && push_source_scope(scopes, grant_paths, root_scope)
    {
        return;
    }
    let mut denied_subtrees = BTreeSet::from([PathScope::root()]);
    for cpath in ceiling.paths() {
        if !can_add_grant_path(grant_paths, cpath) || denied_subtrees.contains(cpath) {
            continue;
        }
        if let Some(scope) = authorize_source_op(
            repo,
            SourceViewOperation::ReadContent,
            cpath.clone(),
            payload,
            access,
        ) {
            let _ = push_source_scope(scopes, grant_paths, scope);
        } else if cpath.subtree {
            denied_subtrees.insert(cpath.clone());
        }
    }
    for entry in entries {
        if !repo.definition.selects(&entry.path)
            || !ceiling.allows(SourceViewOperation::ReadContent, &entry.path)
            || scopes
                .iter()
                .any(|s| s.permits(SourceViewOperation::ReadContent, &entry.path))
        {
            continue;
        }
        let mut admitted = false;
        let parts: Vec<&str> = entry.path.split('/').collect();
        for end in 1..parts.len() {
            let prefix = parts
                .get(..end)
                .map(|slice| slice.join("/"))
                .unwrap_or_default();
            if let Some(dir_scope) = PathScope::from_path(&prefix, true)
                && ceiling.contains(&dir_scope)
                && !denied_subtrees.contains(&dir_scope)
                && can_add_grant_path(grant_paths, &dir_scope)
            {
                if let Some(scope) = authorize_source_op(
                    repo,
                    SourceViewOperation::ReadContent,
                    dir_scope.clone(),
                    payload,
                    access,
                ) {
                    if push_source_scope(scopes, grant_paths, scope) {
                        admitted = true;
                        break;
                    }
                } else {
                    denied_subtrees.insert(dir_scope);
                }
            }
        }
        if !admitted
            && let Some(entry_scope) = PathScope::from_path(&entry.path, false)
            && can_add_grant_path(grant_paths, &entry_scope)
            && let Some(scope) = authorize_source_op(
                repo,
                SourceViewOperation::ReadContent,
                entry_scope,
                payload,
                access,
            )
        {
            let _ = push_source_scope(scopes, grant_paths, scope);
        }
    }
}

fn admit_best_discovery_view(
    repo: &RestrictedSourceRepo<'_>,
    payload: &[u8],
    access: &mut RestrictedSourceAccess<'_>,
) -> Option<(EffectiveSourceScope, AudienceView, AdmittedPartialView)> {
    let ceiling = ViewCeiling::from_definition(repo.definition);
    let mut scopes = Vec::new();
    let mut grant_paths = BTreeSet::new();
    collect_discover_scopes(repo, payload, access, &mut scopes, &mut grant_paths);
    if scopes.is_empty() {
        return None;
    }
    let disc_scope = EffectiveSourceScope::join(&scopes).ok()?;
    let disc_selection = ScopedSelection::for_holder(&disc_scope, &ceiling);
    let disc_filtered = disc_selection.filter_facts(repo.facts).ok()?;
    collect_read_scopes(
        repo,
        &disc_filtered.entries,
        payload,
        access,
        &mut scopes,
        &mut grant_paths,
    );
    let joined = EffectiveSourceScope::join(&scopes).ok()?;
    let selection = ScopedSelection::for_holder(&joined, &ceiling);
    let filtered = selection.filter_facts(repo.facts).ok()?;
    let partial = resolve(repo.definition, repo.admitted, &filtered).ok()?;
    let enumeration = ScopedEnumeration::new(&selection, partial.clone()).ok()?;
    let view = restrict(
        &enumeration,
        &joined,
        &ceiling,
        access.handle_key,
        access.now_unix_s,
    )
    .ok()?;
    Some((joined, view, partial))
}

struct Observation {
    head: Option<String>,
    scope: &'static str,
    reason: Option<&'static str>,
}

fn observe(policy: &SourcePolicy) -> Observation {
    match policy.kind.as_str() {
        "pijul" => match pijul_head(policy) {
            Ok(head) => Observation {
                head: Some(head),
                scope: "current_channel_state",
                reason: None,
            },
            Err(reason) => Observation {
                head: None,
                scope: "current_channel_state",
                reason: Some(reason),
            },
        },
        _ => match open_git(policy).and_then(|repository| head(&repository)) {
            Ok(head) => Observation {
                head: Some(head.to_string()),
                scope: "revision",
                reason: None,
            },
            Err(reason) => Observation {
                head: None,
                scope: "revision",
                reason: Some(reason),
            },
        },
    }
}

fn is_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn reject_symlink(path: &Path) -> Result<PathBuf, &'static str> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "source path is unavailable")?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("source path is not a plain directory");
    }
    Ok(path.to_owned())
}

fn git_directory(policy: &SourcePolicy) -> Result<PathBuf, &'static str> {
    let root = reject_symlink(&policy.path)?;
    let worktree = root.join(".git");
    let worktree_is_directory = std::fs::symlink_metadata(&worktree)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink());
    if worktree_is_directory {
        return Ok(worktree);
    }
    if policy.kind == "git" {
        if std::fs::symlink_metadata(root.join("HEAD")).is_ok() {
            return Ok(root);
        }
        return Err("source path is not a Git repository or worktree");
    }
    let backend = root.join(".jj").join("repo").join("store").join("git");
    if std::fs::symlink_metadata(backend.join("HEAD")).is_ok() {
        return Ok(backend);
    }
    Err("source path has no Git-visible Jujutsu store or head")
}

fn open_git(policy: &SourcePolicy) -> Result<Repository, &'static str> {
    let directory = git_directory(policy)?;
    Repository::open_ext(
        &directory,
        RepositoryOpenFlags::NO_SEARCH | RepositoryOpenFlags::NO_DOTGIT,
        std::iter::empty::<&std::ffi::OsStr>(),
    )
    .map_err(|_| "source repository could not be opened")
}

fn head(repository: &Repository) -> Result<Oid, &'static str> {
    repository
        .head()
        .and_then(|reference| reference.peel_to_commit())
        .map(|commit| commit.id())
        .map_err(|_| "source repository has no resolvable head commit")
}

fn commit<'repo>(
    repository: &'repo Repository,
    revision: &str,
) -> Result<git2::Commit<'repo>, &'static str> {
    if revision.is_empty() {
        let oid = head(repository)?;
        return bounded_commit(repository, oid);
    }
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("revision is not an exact 40-character commit id");
    }
    let oid = Oid::from_str(revision).map_err(|_| "revision is not an exact commit id")?;
    bounded_commit(repository, oid)
}

fn bounded_commit(repository: &Repository, oid: Oid) -> Result<git2::Commit<'_>, &'static str> {
    let (size, kind) = repository
        .odb()
        .and_then(|odb| odb.read_header(oid))
        .map_err(|_| "revision is not an available object")?;
    if kind != ObjectType::Commit || size > MAX_COMMIT_BYTES {
        return Err("revision is not a bounded commit object");
    }
    repository
        .find_commit(oid)
        .map_err(|_| "revision is not an available commit")
}

fn tree<'repo>(
    repository: &'repo Repository,
    commit: &git2::Commit<'repo>,
) -> Result<git2::Tree<'repo>, &'static str> {
    let (size, kind) = repository
        .odb()
        .and_then(|odb| odb.read_header(commit.tree_id()))
        .map_err(|_| "revision tree is unavailable")?;
    if kind != ObjectType::Tree || size > MAX_TREE_BYTES {
        return Err("revision tree exceeds its bound");
    }
    repository
        .find_tree(commit.tree_id())
        .map_err(|_| "revision tree is unavailable")
}

fn components(path: &str) -> Result<Vec<&str>, ()> {
    if path.len() > MAX_PATH_BYTES || path.chars().any(char::is_control) {
        return Err(());
    }
    let mut result = Vec::new();
    if path.is_empty() {
        return Ok(result);
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains('\\') {
            return Err(());
        }
        result.push(part);
    }
    Ok(result)
}

fn brief(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn git_log(policy: &SourcePolicy, limit: usize, offset: usize) -> Result<ApiResponse, String> {
    git_log_at(policy, None, limit, offset)
}

fn git_log_at(
    policy: &SourcePolicy,
    revision: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<ApiResponse, String> {
    let repository = match open_git(policy) {
        Ok(repository) => repository,
        Err(_) => return error(503, "source_unavailable"),
    };
    let head = match revision {
        Some(rev) => match commit(&repository, rev) {
            Ok(commit) => commit.id(),
            Err(_) => return error(503, "source_unavailable"),
        },
        None => match head(&repository) {
            Ok(head) => head,
            Err(_) => return error(503, "source_unavailable"),
        },
    };
    let mut walk = match repository.revwalk() {
        Ok(walk) => walk,
        Err(_) => return error(503, "source_unavailable"),
    };
    if walk.push(head).is_err() {
        return error(503, "source_unavailable");
    }
    let mut entries = Vec::new();
    let mut total = 0usize;
    for (index, oid) in walk.enumerate() {
        let oid = match oid {
            Ok(oid) => oid,
            Err(_) => return error(503, "source_unavailable"),
        };
        total = index.saturating_add(1);
        if index < offset {
            continue;
        }
        if entries.len() > limit {
            break;
        }
        let commit = match bounded_commit(&repository, oid) {
            Ok(commit) => commit,
            Err(_) => return error(503, "source_unavailable"),
        };
        let author = commit.author();
        entries.push(json!({
            "id": oid.to_string(),
            "summary": brief(commit.summary().unwrap_or(None).unwrap_or(""), MAX_SUMMARY_CHARS),
            "author": brief(
                &format!(
                    "{} <{}>",
                    author.name().unwrap_or(""),
                    author.email().unwrap_or("")
                ),
                MAX_AUTHOR_CHARS,
            ),
            "recorded_at_ms": commit.time().seconds().saturating_mul(1000),
            "parents": commit.parent_ids().map(|id| id.to_string()).collect::<Vec<_>>(),
        }));
    }
    let has_more = entries.len() > limit;
    entries.truncate(limit);
    let next = offset.saturating_add(entries.len());
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "head": head.to_string(),
            "revision_scope": "revision",
            "entries": entries,
            "next_cursor": if has_more { Some(next.to_string()) } else { None },
            "observed_revisions": total,
            "non_claims": NON_CLAIMS,
        }),
    )
}

fn git_tree(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
) -> Result<ApiResponse, String> {
    git_tree_filtered(policy, revision, components, None)
}

fn git_tree_filtered(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
    visible_origins: Option<&BTreeSet<&str>>,
) -> Result<ApiResponse, String> {
    let repository = match open_git(policy) {
        Ok(repository) => repository,
        Err(_) => return error(503, "source_unavailable"),
    };
    let commit = match commit(&repository, revision) {
        Ok(commit) => commit,
        Err(_) if !revision.is_empty() => return error(400, "source_revision_invalid"),
        Err(_) => return error(503, "source_unavailable"),
    };
    let mut tree = match tree(&repository, &commit) {
        Ok(tree) => tree,
        Err(_) => return error(503, "source_unavailable"),
    };
    for part in components {
        let next = match tree.get_name(part) {
            Some(entry) if entry.kind() == Some(ObjectType::Tree) => entry.id(),
            Some(_) => return error(404, "source_path_missing"),
            None => return error(404, "source_path_missing"),
        };
        tree = match repository.find_tree(next) {
            Ok(tree) => tree,
            Err(_) => return error(503, "source_unavailable"),
        };
    }
    if visible_origins.is_none() && tree.len() > MAX_TREE_ENTRIES {
        return error(503, "source_tree_bound");
    }
    let mut entries = Vec::new();
    for entry in tree.iter() {
        let name = match entry.name() {
            Ok(name) => name,
            Err(_) => return error(503, "source_unavailable"),
        };
        let entry_path = if components.is_empty() {
            name.to_owned()
        } else {
            format!("{}/{}", components.join("/"), name)
        };
        let kind = match entry.kind() {
            Some(ObjectType::Blob) if entry.filemode() == 0o120000 => "symlink",
            Some(ObjectType::Blob) => "file",
            Some(ObjectType::Tree) => "directory",
            _ => "submodule",
        };
        if let Some(allowed) = visible_origins {
            let visible = if kind == "directory" {
                let prefix = format!("{entry_path}/");
                allowed.iter().any(|p| p.starts_with(&prefix))
            } else {
                allowed.contains(entry_path.as_str())
            };
            if !visible {
                continue;
            }
        }
        if entries.len() >= MAX_TREE_ENTRIES {
            return error(503, "source_tree_bound");
        }
        let bytes = if kind == "file" {
            repository
                .find_blob(entry.id())
                .map(|blob| blob.size())
                .unwrap_or(0)
        } else {
            0
        };
        entries.push(json!({
            "name": name,
            "path": entry_path,
            "kind": kind,
            "mode": format!("{:o}", entry.filemode()),
            "oid": entry.id().to_string(),
            "bytes": bytes,
        }));
    }
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "revision": commit.id().to_string(),
            "revision_scope": "revision",
            "path": components.join("/"),
            "entries": entries,
            "non_claims": NON_CLAIMS,
        }),
    )
}

fn git_file(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
    highlight: bool,
) -> Result<ApiResponse, String> {
    let repository = match open_git(policy) {
        Ok(repository) => repository,
        Err(_) => return error(503, "source_unavailable"),
    };
    let commit = match commit(&repository, revision) {
        Ok(commit) => commit,
        Err(_) if !revision.is_empty() => return error(400, "source_revision_invalid"),
        Err(_) => return error(503, "source_unavailable"),
    };
    let tree = match tree(&repository, &commit) {
        Ok(tree) => tree,
        Err(_) => return error(503, "source_unavailable"),
    };
    let entry = match tree.get_path(Path::new(&components.join("/"))) {
        Ok(entry) => entry,
        Err(_) => return error(404, "source_path_missing"),
    };
    if entry.kind() != Some(ObjectType::Blob) || entry.filemode() == 0o120000 {
        return error(404, "source_path_missing");
    }
    let (size, kind) = match repository.odb().and_then(|odb| odb.read_header(entry.id())) {
        Ok(header) => header,
        Err(_) => return error(503, "source_unavailable"),
    };
    if kind != ObjectType::Blob {
        return error(404, "source_path_missing");
    }
    if size > MAX_FILE_BYTES {
        return error(413, "source_file_too_large");
    }
    let blob = match repository.find_blob(entry.id()) {
        Ok(blob) => blob,
        Err(_) => return error(503, "source_unavailable"),
    };
    let content = blob.content();
    let binary = content.contains(&0) || std::str::from_utf8(content).is_err();
    let mut response = json!({
        "source": policy.id,
        "kind": policy.kind,
        "label": policy.label,
        "revision": commit.id().to_string(),
        "revision_scope": "revision",
        "path": components.join("/"),
        "oid": entry.id().to_string(),
        "bytes": content.len(),
        "binary": binary,
    });
    if !binary
        && let Ok(text) = std::str::from_utf8(content)
        && let Some(object) = response.as_object_mut()
    {
        object.insert("text".into(), Value::String(text.to_owned()));
    }
    if let Some(object) = response.as_object_mut() {
        object.insert("non_claims".into(), json!(NON_CLAIMS));
    }
    let path = components.join("/");
    respond(
        response,
        highlight.then(|| {
            kiln_highlight::highlight(
                kiln_highlight::language_for_path(&path),
                content,
                &kiln_highlight::Bounds::DEFAULT,
            )
        }),
    )
}

struct PijulRun {
    stdout: Vec<u8>,
}

fn git_diff(policy: &SourcePolicy, revision: &str, highlight: bool) -> Result<ApiResponse, String> {
    let repository = match open_git(policy) {
        Ok(repository) => repository,
        Err(_) => return error(503, "source_unavailable"),
    };
    let commit = match commit(&repository, revision) {
        Ok(commit) => commit,
        Err(_) if !revision.is_empty() => return error(400, "source_revision_invalid"),
        Err(_) => return error(503, "source_unavailable"),
    };
    let head = match tree(&repository, &commit) {
        Ok(tree) => tree,
        Err(_) => return error(503, "source_unavailable"),
    };
    let parent = commit.parent(0).ok().map(|parent| parent.id().to_string());
    let parent_tree = match commit.parent(0) {
        Ok(parent) => match tree(&repository, &parent) {
            Ok(tree) => Some(tree),
            Err(_) => return error(503, "source_unavailable"),
        },
        Err(_) => None,
    };
    let mut options = git2::DiffOptions::new();
    options.context_lines(3).interhunk_lines(0);
    let diff =
        match repository.diff_tree_to_tree(parent_tree.as_ref(), Some(&head), Some(&mut options)) {
            Ok(diff) => diff,
            Err(_) => return error(503, "source_unavailable"),
        };
    let stats = match diff.stats() {
        Ok(stats) => stats,
        Err(_) => return error(503, "source_unavailable"),
    };
    let mut buffer: Vec<u8> = Vec::new();
    let mut truncated = false;
    let printed = diff.print(git2::DiffFormat::Patch, |_, _, line| {
        let line_bytes = match line.origin() {
            '+' | '-' | ' ' => {
                let mut bytes = Vec::with_capacity(line.content().len() + 1);
                bytes.push(line.origin() as u8);
                bytes.extend_from_slice(line.content());
                bytes
            }
            _ => line.content().to_vec(),
        };
        if buffer.len().saturating_add(line_bytes.len()) > MAX_DIFF_BYTES {
            truncated = true;
            return false;
        }
        buffer.extend_from_slice(&line_bytes);
        true
    });
    // Stopping at the byte cap surfaces as a print error; that partial result is
    // the reported truncation, not a read failure.
    if printed.is_err() && !truncated {
        return error(503, "source_unavailable");
    }
    let text = String::from_utf8_lossy(&buffer).into_owned();
    let highlighted =
        highlight.then(|| kiln_highlight::highlight_diff(&text, &kiln_highlight::Bounds::DEFAULT));
    respond(
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "revision": commit.id().to_string(),
            "parent": parent,
            "revision_scope": "revision",
            "files_changed": stats.files_changed(),
            "insertions": stats.insertions(),
            "deletions": stats.deletions(),
            "truncated": truncated,
            "diff": text,
            "non_claims": NON_CLAIMS,
        }),
        highlighted,
    )
}

/// Encode a read, adding a requested `highlight` object after the bounded
/// read so the other fields are never altered. A highlight that would push
/// the response past its byte bound becomes an explicit `too_large` skip.
fn respond(
    mut response: Value,
    highlight: Option<kiln_highlight::Highlight>,
) -> Result<ApiResponse, String> {
    let Some(highlight) = highlight else {
        return json_response(200, response);
    };
    let language = highlight.language();
    let encoded = serde_json::to_value(highlight).map_err(|error| error.to_string())?;
    if let Some(object) = response.as_object_mut() {
        object.insert("highlight".into(), encoded);
    }
    let answer = json_response(200, &response)?;
    if answer.body.len() <= MAX_RESPONSE_BYTES {
        return Ok(answer);
    }
    let skipped = kiln_highlight::Highlight::skipped(language, kiln_highlight::Reason::TooLarge);
    let encoded = serde_json::to_value(skipped).map_err(|error| error.to_string())?;
    if let Some(object) = response.as_object_mut() {
        object.insert("highlight".into(), encoded);
    }
    json_response(200, response)
}

fn pijul_run(policy: &SourcePolicy, args: Vec<String>) -> Result<PijulRun, &'static str> {
    let executable = policy
        .executable
        .as_deref()
        .ok_or("no pijul executable is configured")?;
    reject_symlink(&policy.path)?;
    if !policy.path.join(".pijul").is_dir() {
        return Err("source path is not a Pijul repository");
    }
    let limits = ExecutionLimits {
        timeout_ms: PIJUL_TIMEOUT_MS,
        stdin_max_bytes: 1,
        stdout_max_bytes: PIJUL_OUTPUT_BYTES,
        stderr_max_bytes: PIJUL_ERROR_BYTES,
        poll_interval_ms: PIJUL_POLL_MS,
        teardown_timeout_ms: PIJUL_TEARDOWN_MS,
    };
    let outcome = OutcomePolicy::new(vec![0], true, true).map_err(|_| "pijul policy invalid")?;
    let request = RunRequest {
        command: CommandSpec {
            program: executable.to_owned(),
            args: args.into_iter().map(Into::into).collect(),
            current_dir: PathBuf::from("/"),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits,
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: outcome,
    };
    let output = bounded_exec::run(request).map_err(|_| "pijul execution failed")?;
    if output.exit_code != Some(0) || output.stdout.truncated {
        return Err("pijul read did not complete");
    }
    Ok(PijulRun {
        stdout: output.stdout.bytes,
    })
}

fn pijul_base_args(policy: &SourcePolicy, command: &str) -> Vec<String> {
    vec![
        command.to_owned(),
        "--repository".to_owned(),
        policy.path.display().to_string(),
        "--no-prompt".to_owned(),
    ]
}

fn pijul_head(policy: &SourcePolicy) -> Result<String, &'static str> {
    let mut args = pijul_base_args(policy, "log");
    for flag in ["--hash-only", "--limit", "1"] {
        args.push(flag.to_owned());
    }
    let run = pijul_run(policy, args)?;
    let text = std::str::from_utf8(&run.stdout).map_err(|_| "pijul output is not UTF-8")?;
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .ok_or("pijul channel has no recorded change")
}

fn pijul_log(policy: &SourcePolicy, limit: usize, offset: usize) -> Result<ApiResponse, String> {
    let mut args = pijul_base_args(policy, "log");
    for flag in [
        "--output-format",
        "json",
        "--limit",
        &limit.to_string(),
        "--offset",
        &offset.to_string(),
    ] {
        args.push(flag.to_owned());
    }
    let run = match pijul_run(policy, args) {
        Ok(run) => run,
        Err(_) => return error(503, "source_unavailable"),
    };
    let rows: Vec<Value> = match serde_json::from_slice(&run.stdout) {
        Ok(rows) => rows,
        Err(_) => return error(503, "source_unavailable"),
    };
    let mut entries = Vec::new();
    for row in rows.iter().take(limit) {
        let id = row.get("hash").and_then(Value::as_str).unwrap_or_default();
        let authors = row
            .get("authors")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        entries.push(json!({
            "id": brief(id, MAX_SUMMARY_CHARS),
            "summary": brief(
                row.get("message").and_then(Value::as_str).unwrap_or_default(),
                MAX_SUMMARY_CHARS,
            ),
            "author": brief(&authors, MAX_AUTHOR_CHARS),
            "recorded_at_ms": row
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(parse_rfc3339_ms),
        }));
    }
    let has_more = rows.len() > entries.len();
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "channel": policy.channel,
            "revision_scope": "current_channel_state",
            "entries": entries,
            "next_cursor": if has_more {
                Some((offset.saturating_add(entries.len())).to_string())
            } else {
                None
            },
            "non_claims": NON_CLAIMS,
        }),
    )
}

fn pijul_tree(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
) -> Result<ApiResponse, String> {
    pijul_tree_filtered(policy, revision, components, None)
}

fn pijul_tree_filtered(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
    visible_origins: Option<&BTreeSet<&str>>,
) -> Result<ApiResponse, String> {
    if !revision.is_empty() {
        return error(400, "source_revision_unsupported");
    }
    let run = match pijul_run(policy, pijul_base_args(policy, "list")) {
        Ok(run) => run,
        Err(_) => return error(503, "source_unavailable"),
    };
    let text = match std::str::from_utf8(&run.stdout) {
        Ok(text) => text,
        Err(_) => return error(503, "source_unavailable"),
    };
    let prefix = components.join("/");
    let mut seen = BTreeSet::new();
    let mut entries = vec![];
    for line in text.lines() {
        let path = line.trim();
        if path.is_empty() || path.len() > MAX_PATH_BYTES || path.contains('\\') {
            continue;
        }
        if let Some(allowed) = visible_origins
            && !allowed.contains(path)
        {
            continue;
        }
        let parts: Vec<&str> = path.split('/').collect();
        if components.len() >= parts.len() {
            continue;
        }
        if !parts.iter().take(components.len()).eq(components.iter()) {
            continue;
        }
        let Some(name) = parts.get(components.len()) else {
            continue;
        };
        let directory = parts.len() > components.len().saturating_add(1);
        let key = (name.to_string(), directory);
        if !seen.insert(key.clone()) {
            continue;
        }
        if entries.len() >= MAX_TREE_ENTRIES {
            return error(503, "source_tree_bound");
        }
        entries.push(json!({
            "name": name,
            "path": if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}/{name}")
            },
            "kind": if directory { "directory" } else { "file" },
            "mode": null,
            "oid": null,
            "bytes": null,
        }));
    }
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "channel": policy.channel,
            "revision": Value::Null,
            "revision_scope": "current_channel_state",
            "path": prefix,
            "entries": entries,
            "non_claims": NON_CLAIMS,
        }),
    )
}

fn pijul_file(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
    highlight: bool,
) -> Result<ApiResponse, String> {
    if !revision.is_empty() {
        return error(400, "source_revision_unsupported");
    }
    let run = match pijul_run(policy, pijul_base_args(policy, "list")) {
        Ok(run) => run,
        Err(_) => return error(503, "source_unavailable"),
    };
    let text = match std::str::from_utf8(&run.stdout) {
        Ok(text) => text,
        Err(_) => return error(503, "source_unavailable"),
    };
    let relative = components.join("/");
    if !text.lines().any(|line| line.trim() == relative) {
        return error(404, "source_path_missing");
    }
    let mut path = policy.path.clone();
    for part in components {
        path.push(part);
    }
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(_) => return error(404, "source_path_missing"),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return error(404, "source_path_missing");
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return error(413, "source_file_too_large");
    }
    let content = match std::fs::read(&path) {
        Ok(content) => content,
        Err(_) => return error(503, "source_unavailable"),
    };
    let binary = content.contains(&0) || std::str::from_utf8(&content).is_err();
    let highlighted = highlight.then(|| {
        kiln_highlight::highlight(
            kiln_highlight::language_for_path(&relative),
            &content,
            &kiln_highlight::Bounds::DEFAULT,
        )
    });
    let mut response = json!({
        "source": policy.id,
        "kind": policy.kind,
        "label": policy.label,
        "channel": policy.channel,
        "revision": Value::Null,
        "revision_scope": "working_copy",
        "path": relative,
        "oid": Value::Null,
        "bytes": content.len(),
        "binary": binary,
    });
    if !binary
        && let Ok(text) = std::str::from_utf8(&content)
        && let Some(object) = response.as_object_mut()
    {
        object.insert("text".into(), Value::String(text.to_owned()));
    }
    if let Some(object) = response.as_object_mut() {
        object.insert("non_claims".into(), json!(NON_CLAIMS));
    }
    respond(response, highlighted)
}

fn search_cursor_tag(view: &AudienceView, query_text: &str, offset: usize) -> blake3::Hash {
    let key_hash = blake3::hash(view.binding().identity().as_str().as_bytes());
    let mut hasher = blake3::Hasher::new_keyed(key_hash.as_bytes());
    hasher.update(SEARCH_CURSOR_DOMAIN.as_bytes());
    hasher.update(b"\0");
    hasher.update(view.reference().as_bytes());
    hasher.update(b"\0");
    hasher.update(&(query_text.len() as u64).to_le_bytes());
    hasher.update(query_text.as_bytes());
    hasher.update(&(offset as u64).to_le_bytes());
    hasher.finalize()
}

fn encode_search_cursor(view: &AudienceView, query_text: &str, offset: usize) -> String {
    let pos = u32::try_from(offset).unwrap_or(u32::MAX);
    let tag = search_cursor_tag(view, query_text, offset);
    let mut out = String::with_capacity(8 + 64);
    for byte in pos.to_be_bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out.push_str(tag.to_hex().as_str());
    out
}

fn decode_search_cursor(token: &str, view: &AudienceView, query_text: &str) -> Option<usize> {
    if token.len() != 8 + 64 {
        return None;
    }
    let pos = u32::from_str_radix(token.get(..8)?, 16).ok()? as usize;
    let tag = blake3::Hash::from_hex(token.get(8..)?).ok()?;
    let expected = search_cursor_tag(view, query_text, pos);
    (tag == expected).then_some(pos)
}

fn search_snippets(bytes: &[u8], needle_lower: &str) -> Vec<Value> {
    if bytes.contains(&0) {
        return Vec::new();
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };
    let mut snippets = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.to_lowercase().contains(needle_lower) {
            let trimmed: String = line.chars().take(MAX_SNIPPET_CHARS).collect();
            snippets.push(json!({
                "line": idx + 1,
                "text": trimmed,
            }));
            if snippets.len() >= MAX_SNIPPETS_PER_FILE {
                break;
            }
        }
    }
    snippets
}

fn read_git_blob_bytes(
    repository: &Repository,
    tree: &git2::Tree<'_>,
    relative_path: &str,
) -> Result<Vec<u8>, String> {
    let entry = tree
        .get_path(Path::new(relative_path))
        .map_err(|_| "source_path_missing".to_string())?;
    if entry.kind() != Some(ObjectType::Blob) || entry.filemode() == 0o120000 {
        return Err("source_path_missing".into());
    }
    let (size, kind) = repository
        .odb()
        .and_then(|odb| odb.read_header(entry.id()))
        .map_err(|_| "source_unavailable".to_string())?;
    if kind != ObjectType::Blob {
        return Err("source_path_missing".into());
    }
    if size > MAX_FILE_BYTES {
        return Err("source_file_too_large".into());
    }
    let blob = repository
        .find_blob(entry.id())
        .map_err(|_| "source_unavailable".to_string())?;
    Ok(blob.content().to_vec())
}

fn git_raw_file(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
) -> Result<Vec<u8>, String> {
    let repository = open_git(policy).map_err(|_| "source_unavailable".to_string())?;
    let commit_obj = commit(&repository, revision).map_err(|_| "source_unavailable".to_string())?;
    let tree_obj = tree(&repository, &commit_obj).map_err(|_| "source_unavailable".to_string())?;
    read_git_blob_bytes(&repository, &tree_obj, &components.join("/"))
}

fn pijul_raw_file(
    policy: &SourcePolicy,
    revision: &str,
    components: &[&str],
) -> Result<Vec<u8>, String> {
    if !revision.is_empty() {
        return Err("source_revision_unsupported".into());
    }
    let run =
        pijul_run(policy, pijul_base_args(policy, "list")).map_err(|e| e.to_string())?;
    let text = std::str::from_utf8(&run.stdout).map_err(|_| "source_unavailable".to_string())?;
    let relative = components.join("/");
    if !text.lines().any(|line| line.trim() == relative) {
        return Err("source_path_missing".into());
    }
    let mut path = policy.path.clone();
    for part in components {
        path.push(part);
    }
    let metadata =
        std::fs::symlink_metadata(&path).map_err(|_| "source_path_missing".to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("source_path_missing".into());
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err("source_file_too_large".into());
    }
    std::fs::read(&path).map_err(|_| "source_unavailable".to_string())
}

fn raw_download_response(display_path: &str, bytes: Vec<u8>) -> ApiResponse {
    let basename = display_path
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .unwrap_or("source.bin");
    let safe_name: String = basename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    ApiResponse {
        status: 200,
        content_type: "application/octet-stream",
        body: bytes,
        download_name: Some(safe_name),
        headers: vec![("X-Content-Type-Options".into(), "nosniff".into())],
    }
}

fn git_search(
    policy: &SourcePolicy,
    revision: &str,
    query_text: &str,
    limit: usize,
    offset: usize,
) -> Result<ApiResponse, String> {
    let repository = match open_git(policy) {
        Ok(repository) => repository,
        Err(_) => return error(503, "source_unavailable"),
    };
    let commit_obj = match commit(&repository, revision) {
        Ok(commit) => commit,
        Err(_) if !revision.is_empty() => return error(400, "source_revision_invalid"),
        Err(_) => return error(503, "source_unavailable"),
    };
    let tree_obj = match tree(&repository, &commit_obj) {
        Ok(tree) => tree,
        Err(_) => return error(503, "source_unavailable"),
    };
    let needle = query_text.to_lowercase();
    let mut matches = Vec::new();
    let mut paths = Vec::new();
    tree_obj
        .walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if entry.kind() == Some(ObjectType::Blob) && entry.filemode() != 0o120000
                && let Ok(name) = entry.name()
            {
                paths.push(format!("{root}{name}"));
            }
            if paths.len() >= MAX_TREE_ENTRIES * 8 {
                return git2::TreeWalkResult::Abort;
            }
            git2::TreeWalkResult::Ok
        })
        .ok();
    paths.sort();
    for path in paths {
        let path_match = path.to_lowercase().contains(&needle);
        let snippets = read_git_blob_bytes(&repository, &tree_obj, &path)
            .map(|bytes| search_snippets(&bytes, &needle))
            .unwrap_or_default();
        if path_match || !snippets.is_empty() {
            matches.push(json!({
                "path": path,
                "path_match": path_match,
                "snippets": snippets,
            }));
            if matches.len() >= MAX_SEARCH_MATCHES {
                break;
            }
        }
    }
    if offset > matches.len() {
        return error(400, "source_cursor_invalid");
    }
    let end = offset.saturating_add(limit).min(matches.len());
    let next_cursor = (end < matches.len()).then(|| end.to_string());
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "revision": commit_obj.id().to_string(),
            "query": query_text,
            "matches": matches[offset..end],
            "total_matches": matches.len(),
            "truncated": matches.len() >= MAX_SEARCH_MATCHES,
            "next_cursor": next_cursor,
            "non_claims": NON_CLAIMS,
        }),
    )
}

fn pijul_search(
    policy: &SourcePolicy,
    revision: &str,
    query_text: &str,
    limit: usize,
    offset: usize,
) -> Result<ApiResponse, String> {
    if !revision.is_empty() {
        return error(400, "source_revision_unsupported");
    }
    let run = match pijul_run(policy, pijul_base_args(policy, "list")) {
        Ok(run) => run,
        Err(_) => return error(503, "source_unavailable"),
    };
    let text = match std::str::from_utf8(&run.stdout) {
        Ok(text) => text,
        Err(_) => return error(503, "source_unavailable"),
    };
    let needle = query_text.to_lowercase();
    let mut matches = Vec::new();
    for line in text.lines() {
        let path = line.trim();
        let Ok(comps) = components(path) else {
            continue;
        };
        let path_match = path.to_lowercase().contains(&needle);
        let snippets = pijul_raw_file(policy, revision, &comps)
            .map(|bytes| search_snippets(&bytes, &needle))
            .unwrap_or_default();
        if path_match || !snippets.is_empty() {
            matches.push(json!({
                "path": path,
                "path_match": path_match,
                "snippets": snippets,
            }));
            if matches.len() >= MAX_SEARCH_MATCHES {
                break;
            }
        }
    }
    if offset > matches.len() {
        return error(400, "source_cursor_invalid");
    }
    let end = offset.saturating_add(limit).min(matches.len());
    let next_cursor = (end < matches.len()).then(|| end.to_string());
    json_response(
        200,
        json!({
            "source": policy.id,
            "kind": policy.kind,
            "label": policy.label,
            "channel": policy.channel,
            "revision": Value::Null,
            "query": query_text,
            "matches": matches[offset..end],
            "total_matches": matches.len(),
            "truncated": matches.len() >= MAX_SEARCH_MATCHES,
            "next_cursor": next_cursor,
            "non_claims": NON_CLAIMS,
        }),
    )
}

/// Parse one UTC RFC 3339 timestamp into milliseconds since the Unix epoch.
///
/// Fractional seconds and a `Z` designator are accepted; offsets, non-UTC
/// designs and malformed values yield `None` rather than a guessed instant.
fn parse_rfc3339_ms(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 20 || bytes.get(10) != Some(&b'T') {
        return None;
    }
    let number = |start: usize, end: usize| -> Option<i64> {
        let text = value.get(start..end)?;
        if !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        text.parse::<i64>().ok()
    };
    let year = number(0, 4)?;
    let month = number(5, 7)?;
    let day = number(8, 10)?;
    let hour = number(11, 13)?;
    let minute = number(14, 16)?;
    let second = number(17, 19)?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    if value.get(19..)?.starts_with('.') {
        let rest = value.get(20..)?;
        let digits = rest
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(rest.len());
        if digits == 0 {
            return None;
        }
        if value.get(20 + digits..) != Some("Z") {
            return None;
        }
    } else if value.get(19..) != Some("Z") {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    Some(
        days.saturating_mul(86_400)
            .saturating_add(hour * 3_600)
            .saturating_add(minute * 60)
            .saturating_add(second)
            .saturating_mul(1_000),
    )
}

fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_index = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_conversion_rejects_non_utc_and_malformed_values() {
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339_ms("2025-12-31T19:10:02.401924062Z"),
            Some(1_767_208_202_000)
        );
        for invalid in [
            "2025-12-31T19:10:02+01:00",
            "2025-12-31 19:10:02Z",
            "2025-13-31T19:10:02Z",
            "2025-12-31T19:10:02",
            "",
        ] {
            assert!(parse_rfc3339_ms(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn path_components_reject_traversal_and_absolute_forms() {
        assert_eq!(components(""), Ok(Vec::new()));
        assert_eq!(components("a/b"), Ok(vec!["a", "b"]));
        for invalid in ["/etc/passwd", "../x", "a//b", "a/./b", "a\\b"] {
            assert!(components(invalid).is_err(), "{invalid}");
        }
    }
}
