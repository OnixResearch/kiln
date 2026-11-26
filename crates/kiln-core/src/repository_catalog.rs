use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{ContentId, CoreResult, Diagnostic};

/// Maximum number of repositories retained in a reconciled catalog.
pub const MAX_CATALOG_REPOS: usize = 256;
/// Maximum number of declared or observed remotes per repository.
pub const MAX_REMOTES_PER_REPO: usize = 16;
/// Maximum number of fleet local checkouts aggregated per repository.
pub const MAX_LOCAL_COPIES_PER_REPO: usize = 64;
/// Maximum byte length of a remote URL or Radicle RID input.
pub const MAX_URL_BYTES: usize = 2048;
/// Maximum byte length of a repository slug or label.
pub const MAX_SLUG_BYTES: usize = 256;
/// Maximum byte length of a local filesystem checkout path.
pub const MAX_PATH_BYTES: usize = 1024;
/// Maximum byte length of a default branch/ref name.
pub const MAX_DEFAULT_REF_BYTES: usize = 256;
/// Maximum byte length of a fleet machine identifier.
pub const MAX_MACHINE_BYTES: usize = 128;
/// Maximum number of non-claim statements carried on a repository record.
pub const MAX_NON_CLAIMS: usize = 16;

const RADICLE_RID_MIN_LEN: usize = 24;
const RADICLE_RID_MAX_LEN: usize = 64;
const MAX_DNS_HOST_BYTES: usize = 253;
const MAX_DNS_LABEL_BYTES: usize = 63;

/// Supported external repository remote providers.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvider {
    Github,
    Gitea,
    Radicle,
}

/// Role of a remote endpoint relative to Kiln's repository record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteRole {
    Canonical,
    Mirror,
}

/// Observation status of a declared or discovered remote endpoint.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteStatus {
    Declared,
    Observed,
    Stale,
    Unavailable,
}

/// Normalized remote endpoint attached to a repository record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RemoteEndpoint {
    pub provider: RemoteProvider,
    pub role: RemoteRole,
    pub url: String,
    pub native_id: String,
    pub normalized_key: String,
    pub default_ref: Option<String>,
    pub observed_revision: Option<String>,
    pub observed_at_ms: Option<u64>,
    pub status: RemoteStatus,
}

/// Local version-control backend detected for a fleet working copy.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalVcs {
    Git,
    Jj,
    Pijul,
}

/// Reachability and readability health of a fleet working copy.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyHealth {
    Available,
    Unavailable,
}

/// Read-only observation of a repository checkout on a single fleet machine.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FleetLocalCopy {
    pub machine: String,
    pub path: String,
    pub vcs: Option<LocalVcs>,
    pub head_commit: Option<String>,
    pub branch_or_bookmark: Option<String>,
    pub dirty: Option<bool>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub remotes: Vec<String>,
    pub cairn_count: Option<u32>,
    pub observed_at_ms: Option<u64>,
    pub health: CopyHealth,
    pub error: Option<String>,
}

/// Aggregated cross-fleet synchronization and health status.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetSyncStatus {
    Synced,
    Diverged,
    Dirty,
    Unknown,
    LocalOnly,
}

/// Explicit admission flags separating catalog observation from CI/fleet grants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryAdmission {
    pub catalog_imported: bool,
    pub ci_configured: bool,
    pub source_configured: bool,
    pub fleet_enrolled: bool,
}

/// Canonical Kiln repository catalog record reconciled with fleet local copies.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryRecord {
    pub identity: ContentId,
    pub slug: String,
    pub label: Option<String>,
    pub canonical: Option<RemoteEndpoint>,
    pub remotes: Vec<RemoteEndpoint>,
    pub local_copies: Vec<FleetLocalCopy>,
    pub sync_status: FleetSyncStatus,
    pub admission: RepositoryAdmission,
    pub total_cairns: u32,
    pub non_claims: Vec<String>,
}

/// Compute Kiln's deterministic BLAKE3 canonical repository identity from a normalized key.
#[must_use]
pub fn canonical_repository_identity(key: &str) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kiln.repository.v1\0");
    hasher.update(key.as_bytes());
    let hex = hasher.finalize().to_hex();
    ContentId::from_blake3_hex(hex.as_str()).unwrap_or_else(|_| unreachable!())
}

/// Normalize and validate a remote URL or Radicle RID for the given provider and role.
pub fn normalize_remote(
    provider: RemoteProvider,
    role: RemoteRole,
    raw_url: &str,
    default_ref: Option<&str>,
) -> CoreResult<RemoteEndpoint> {
    validate_raw_url_surface(raw_url)?;
    let validated_ref = validate_default_ref(default_ref)?;

    let (url, native_id, normalized_key) = match provider {
        RemoteProvider::Github => normalize_github_url(raw_url)?,
        RemoteProvider::Gitea => normalize_gitea_url(raw_url)?,
        RemoteProvider::Radicle => normalize_radicle_rid(raw_url)?,
    };

    Ok(RemoteEndpoint {
        provider,
        role,
        url,
        native_id,
        normalized_key,
        default_ref: validated_ref,
        observed_revision: None,
        observed_at_ms: None,
        status: RemoteStatus::Declared,
    })
}

/// Parse an arbitrary git remote URL or Radicle RID from a local checkout into `(provider, normalized_key)`.
///
/// Bare `owner/repo` shorthand is intentionally not accepted here because local checkout remote
/// lists should contain explicit remote URLs or Radicle RIDs rather than filesystem fragments.
#[must_use]
pub fn normalize_any_remote_url(raw: &str) -> Option<(RemoteProvider, String)> {
    if raw.starts_with("rad:") || raw.starts_with("rad://") || raw.starts_with("rad::rad://") {
        return normalize_remote(RemoteProvider::Radicle, RemoteRole::Mirror, raw, None)
            .ok()
            .map(|ep| (RemoteProvider::Radicle, ep.normalized_key));
    }

    if is_explicit_github_remote(raw) {
        return normalize_remote(RemoteProvider::Github, RemoteRole::Mirror, raw, None)
            .ok()
            .map(|ep| (RemoteProvider::Github, ep.normalized_key));
    }

    normalize_remote(RemoteProvider::Gitea, RemoteRole::Mirror, raw, None)
        .ok()
        .map(|ep| (RemoteProvider::Gitea, ep.normalized_key))
}

/// Validate a repository's remote endpoint collection against bounds, canonical uniqueness, and duplicates.
pub fn validate_remote_set(remotes: &[RemoteEndpoint]) -> CoreResult<()> {
    if remotes.len() > MAX_REMOTES_PER_REPO {
        return Err(Diagnostic::new(
            "remote_limit_exceeded",
            "remotes",
            "repository exceeds maximum number of remotes",
        ));
    }

    let mut canonical_count = 0usize;
    let mut seen: BTreeSet<(RemoteProvider, &str)> = BTreeSet::new();

    for endpoint in remotes {
        validate_raw_url_surface(&endpoint.url)?;
        if endpoint.native_id.is_empty()
            || endpoint.native_id.chars().any(char::is_control)
            || endpoint.normalized_key.is_empty()
            || endpoint.normalized_key.chars().any(char::is_control)
        {
            return Err(Diagnostic::new(
                "invalid_remote_endpoint",
                "remotes",
                "remote endpoint must carry non-empty bounded identifiers without control characters",
            ));
        }
        if endpoint.role == RemoteRole::Canonical {
            canonical_count += 1;
            if canonical_count > 1 {
                return Err(Diagnostic::new(
                    "duplicate_canonical_remote",
                    "remotes",
                    "repository may declare at most one canonical remote",
                ));
            }
        }
        if !seen.insert((endpoint.provider, endpoint.normalized_key.as_str())) {
            return Err(Diagnostic::new(
                "duplicate_remote",
                "remotes",
                "repository must not declare duplicate remotes",
            ));
        }
    }

    Ok(())
}

/// Validate a single `FleetLocalCopy` observation record's bounded fields.
pub fn validate_local_copy(copy: &FleetLocalCopy) -> CoreResult<()> {
    if copy.machine.is_empty()
        || copy.machine.len() > MAX_MACHINE_BYTES
        || copy.machine.chars().any(char::is_control)
    {
        return Err(Diagnostic::new(
            "invalid_local_copy_machine",
            "local_copy.machine",
            "local copy machine name must be non-empty, bounded, and free of control characters",
        ));
    }
    if copy.path.is_empty()
        || copy.path.len() > MAX_PATH_BYTES
        || copy.path.chars().any(char::is_control)
    {
        return Err(Diagnostic::new(
            "invalid_local_copy_path",
            "local_copy.path",
            "local copy path must be non-empty, bounded, and free of control characters",
        ));
    }
    Ok(())
}

/// Derive a bounded human-readable repository slug from a normalized remote endpoint.
#[must_use]
pub fn repository_slug_from_endpoint(endpoint: &RemoteEndpoint) -> String {
    let raw = match endpoint.provider {
        RemoteProvider::Github | RemoteProvider::Radicle => endpoint.native_id.as_str(),
        RemoteProvider::Gitea => endpoint
            .native_id
            .split_once('/')
            .map_or(endpoint.native_id.as_str(), |(_, owner_repo)| owner_repo),
    };
    truncate_utf8(raw, MAX_SLUG_BYTES)
}

/// Build default honest non-claim statements for a repository record.
#[must_use]
pub fn default_non_claims(has_radicle: bool) -> Vec<String> {
    let mut claims = vec![
        "Catalog registration does not grant CI workflow execution or Fleet agent dispatch."
            .to_string(),
        "Observed remotes and local copies are read-only snapshots and do not prove upstream reachability or publication."
            .to_string(),
    ];
    if has_radicle {
        claims.push(
            "Radicle RIDs are recorded as read-only declarations; Kiln performs no Radicle initialization, signing, push, or mirror publication."
                .to_string(),
        );
    }
    claims
}

/// Construct a validated `RepositoryRecord` from a primary remote endpoint and optional mirrors.
pub fn build_repository_record(
    primary: RemoteEndpoint,
    mirrors: Vec<RemoteEndpoint>,
    label: Option<String>,
    admission: RepositoryAdmission,
) -> CoreResult<RepositoryRecord> {
    let mut remotes = Vec::with_capacity(1 + mirrors.len());
    remotes.push(primary);
    remotes.extend(mirrors);
    validate_remote_set(&remotes)?;

    if let Some(ref text) = label
        && (text.is_empty() || text.len() > MAX_SLUG_BYTES || text.chars().any(char::is_control))
    {
        return Err(Diagnostic::new(
            "invalid_repository_label",
            "label",
            "repository label must be non-empty, bounded, and free of control characters",
        ));
    }

    let canonical = remotes
        .iter()
        .find(|r| r.role == RemoteRole::Canonical)
        .cloned();
    let key_source = canonical
        .as_ref()
        .or_else(|| remotes.first())
        .ok_or_else(|| Diagnostic::new("empty_remotes", "remotes", "repository has no remotes"))?;
    let identity = canonical_repository_identity(&key_source.normalized_key);
    let slug = repository_slug_from_endpoint(key_source);
    let has_radicle = remotes
        .iter()
        .any(|r| r.provider == RemoteProvider::Radicle);

    Ok(RepositoryRecord {
        identity,
        slug,
        label,
        canonical,
        remotes,
        local_copies: Vec::new(),
        sync_status: FleetSyncStatus::Unknown,
        admission,
        total_cairns: 0,
        non_claims: default_non_claims(has_radicle),
    })
}

/// Attach a validated remote or mirror endpoint to an existing `RepositoryRecord`.
pub fn attach_remote_to_record(
    record: &mut RepositoryRecord,
    endpoint: RemoteEndpoint,
) -> CoreResult<()> {
    let mut candidate_remotes = record.remotes.clone();
    if let Some(ref c) = record.canonical
        && !candidate_remotes
            .iter()
            .any(|r| r.provider == c.provider && r.normalized_key == c.normalized_key)
    {
        candidate_remotes.push(c.clone());
    }
    candidate_remotes.push(endpoint.clone());
    validate_remote_set(&candidate_remotes)?;

    record.remotes = candidate_remotes;
    record.canonical = record
        .remotes
        .iter()
        .find(|r| r.role == RemoteRole::Canonical)
        .cloned();
    let has_radicle = record
        .remotes
        .iter()
        .any(|r| r.provider == RemoteProvider::Radicle);
    merge_non_claims(&mut record.non_claims, default_non_claims(has_radicle));
    Ok(())
}

/// Compute the honest aggregated `FleetSyncStatus` for a repository's copies and remotes.
///
/// Priority: `Diverged` > `Dirty` > `Unknown` > `LocalOnly` (when no remotes are attached and
/// copies exist) > `Synced`. Unavailable or unreadable copies never collapse into `Synced`.
#[must_use]
pub fn compute_sync_status(
    has_remotes: bool,
    copies: &[FleetLocalCopy],
    remotes: &[RemoteEndpoint],
) -> FleetSyncStatus {
    let mut has_unavailable_or_missing_head = false;
    let mut has_dirty = false;
    let mut has_diverged = false;
    let mut distinct_heads: BTreeSet<&str> = BTreeSet::new();

    for copy in copies {
        if copy.health == CopyHealth::Unavailable {
            has_unavailable_or_missing_head = true;
            continue;
        }
        if copy.ahead.unwrap_or(0) > 0 || copy.behind.unwrap_or(0) > 0 {
            has_diverged = true;
        }
        if copy.dirty == Some(true) {
            has_dirty = true;
        }
        match copy.head_commit.as_deref() {
            Some(head) if !head.is_empty() => {
                distinct_heads.insert(head);
            }
            _ => {
                has_unavailable_or_missing_head = true;
            }
        }
    }

    if distinct_heads.len() > 1 {
        has_diverged = true;
    }

    if !has_diverged && !distinct_heads.is_empty() {
        for remote in remotes {
            if let Some(observed) = remote.observed_revision.as_deref()
                && !observed.is_empty()
                && distinct_heads
                    .iter()
                    .any(|head| !commits_compatible(head, observed))
            {
                has_diverged = true;
                break;
            }
        }
    }

    if has_diverged {
        return FleetSyncStatus::Diverged;
    }
    if has_dirty {
        return FleetSyncStatus::Dirty;
    }
    if has_unavailable_or_missing_head {
        return FleetSyncStatus::Unknown;
    }
    if !has_remotes {
        if copies.is_empty() {
            return FleetSyncStatus::Unknown;
        }
        return FleetSyncStatus::LocalOnly;
    }
    if copies.is_empty() {
        return FleetSyncStatus::Unknown;
    }
    FleetSyncStatus::Synced
}

/// Reconcile imported catalog repository records with observed fleet local copies.
///
/// Matches each `FleetLocalCopy` by normalizing its remote URLs/RIDs to imported records;
/// unmatched copies are grouped deterministically by normalized remote key (when present) or
/// `(slug, path)` into un-imported / `LocalOnly` records.
#[must_use]
pub fn reconcile_catalog(
    imported: &[RepositoryRecord],
    local_copies: &[FleetLocalCopy],
) -> Vec<RepositoryRecord> {
    let mut records: Vec<RepositoryRecord> = Vec::with_capacity(imported.len());
    for entry in imported.iter().take(MAX_CATALOG_REPOS) {
        let mut record = entry.clone();
        if let Some(ref c) = record.canonical
            && !record
                .remotes
                .iter()
                .any(|r| r.provider == c.provider && r.normalized_key == c.normalized_key)
            && record.remotes.len() < MAX_REMOTES_PER_REPO
        {
            record.remotes.push(c.clone());
        }
        record.canonical = record
            .remotes
            .iter()
            .find(|r| r.role == RemoteRole::Canonical)
            .cloned();
        records.push(record);
    }

    // Track unmatched copies to group into discovered records.
    struct DiscoveredGroup {
        keys: BTreeSet<(RemoteProvider, String)>,
        endpoints: Vec<RemoteEndpoint>,
        copies: Vec<FleetLocalCopy>,
        fallback_slug: String,
        fallback_path: String,
    }

    let mut unmatched_remote_groups: Vec<DiscoveredGroup> = Vec::new();
    let mut unmatched_local_only: BTreeMap<(String, String), Vec<FleetLocalCopy>> = BTreeMap::new();

    for copy in local_copies {
        let normalized_keys = extract_normalized_keys(&copy.remotes);

        if let Some(record_idx) = find_matching_record(&records, &normalized_keys, copy) {
            if let Some(target) = records.get_mut(record_idx) {
                upsert_local_copy(&mut target.local_copies, copy.clone());
            }
            continue;
        }

        if normalized_keys.is_empty() {
            let slug = path_slug(&copy.path);
            let group_key = (slug, copy.path.clone());
            let group = unmatched_local_only.entry(group_key).or_default();
            upsert_local_copy(group, copy.clone());
        } else {
            let key_set: BTreeSet<(RemoteProvider, String)> =
                normalized_keys.iter().cloned().collect();
            if let Some(group) = unmatched_remote_groups
                .iter_mut()
                .find(|g| g.keys.iter().any(|k| key_set.contains(k)))
            {
                group.keys.extend(key_set);
                for raw in &copy.remotes {
                    if let Some((provider, _)) = normalize_any_remote_url(raw)
                        && let Ok(mut ep) =
                            normalize_remote(provider, RemoteRole::Mirror, raw, None)
                    {
                        ep.status = RemoteStatus::Observed;
                        if !group.endpoints.iter().any(|e| {
                            e.provider == ep.provider && e.normalized_key == ep.normalized_key
                        }) && group.endpoints.len() < MAX_REMOTES_PER_REPO
                        {
                            group.endpoints.push(ep);
                        }
                    }
                }
                upsert_local_copy(&mut group.copies, copy.clone());
            } else {
                let mut endpoints = Vec::new();
                for raw in &copy.remotes {
                    if let Some((provider, _)) = normalize_any_remote_url(raw)
                        && let Ok(mut ep) =
                            normalize_remote(provider, RemoteRole::Mirror, raw, None)
                    {
                        ep.status = RemoteStatus::Observed;
                        if !endpoints.iter().any(|e: &RemoteEndpoint| {
                            e.provider == ep.provider && e.normalized_key == ep.normalized_key
                        }) && endpoints.len() < MAX_REMOTES_PER_REPO
                        {
                            endpoints.push(ep);
                        }
                    }
                }
                unmatched_remote_groups.push(DiscoveredGroup {
                    keys: key_set,
                    endpoints,
                    copies: vec![copy.clone()],
                    fallback_slug: path_slug(&copy.path),
                    fallback_path: copy.path.clone(),
                });
            }
        }
    }

    for mut group in unmatched_remote_groups {
        if records.len() >= MAX_CATALOG_REPOS {
            break;
        }
        group.endpoints.sort_by(|a, b| {
            provider_priority(a.provider)
                .cmp(&provider_priority(b.provider))
                .then_with(|| a.normalized_key.cmp(&b.normalized_key))
        });
        let primary = group.endpoints.first();
        let identity_key = primary.map_or_else(
            || format!("local:{}:{}", group.fallback_slug, group.fallback_path),
            |ep| ep.normalized_key.clone(),
        );
        let slug = primary.map_or_else(
            || group.fallback_slug.clone(),
            repository_slug_from_endpoint,
        );
        let has_radicle = group
            .endpoints
            .iter()
            .any(|r| r.provider == RemoteProvider::Radicle);
        records.push(RepositoryRecord {
            identity: canonical_repository_identity(&identity_key),
            slug,
            label: None,
            canonical: None,
            remotes: group.endpoints,
            local_copies: group.copies,
            sync_status: FleetSyncStatus::Unknown,
            admission: RepositoryAdmission::default(),
            total_cairns: 0,
            non_claims: default_non_claims(has_radicle),
        });
    }

    for ((slug, path), copies) in unmatched_local_only {
        if records.len() >= MAX_CATALOG_REPOS {
            break;
        }
        let identity_key = format!("local:{slug}:{path}");
        records.push(RepositoryRecord {
            identity: canonical_repository_identity(&identity_key),
            slug,
            label: None,
            canonical: None,
            remotes: Vec::new(),
            local_copies: copies,
            sync_status: FleetSyncStatus::LocalOnly,
            admission: RepositoryAdmission::default(),
            total_cairns: 0,
            non_claims: default_non_claims(false),
        });
    }

    for record in &mut records {
        record.remotes.sort_by(|a, b| {
            remote_role_rank(a.role)
                .cmp(&remote_role_rank(b.role))
                .then_with(|| a.provider.cmp(&b.provider))
                .then_with(|| a.normalized_key.cmp(&b.normalized_key))
        });
        record.remotes.truncate(MAX_REMOTES_PER_REPO);
        record
            .local_copies
            .sort_by(|a, b| a.machine.cmp(&b.machine).then_with(|| a.path.cmp(&b.path)));
        record.local_copies.truncate(MAX_LOCAL_COPIES_PER_REPO);

        let has_remotes = record.canonical.is_some() || !record.remotes.is_empty();
        record.sync_status =
            compute_sync_status(has_remotes, &record.local_copies, &record.remotes);
        record.total_cairns = record.local_copies.iter().fold(0u32, |acc, c| {
            acc.saturating_add(c.cairn_count.unwrap_or(0))
        });
        let has_radicle = record
            .remotes
            .iter()
            .any(|r| r.provider == RemoteProvider::Radicle)
            || record
                .canonical
                .as_ref()
                .is_some_and(|c| c.provider == RemoteProvider::Radicle);
        merge_non_claims(&mut record.non_claims, default_non_claims(has_radicle));
    }

    records.sort_by(|a, b| {
        a.slug
            .cmp(&b.slug)
            .then_with(|| a.identity.cmp(&b.identity))
    });
    records.truncate(MAX_CATALOG_REPOS);
    records
}

fn validate_raw_url_surface(raw_url: &str) -> CoreResult<()> {
    if raw_url.is_empty() {
        return Err(Diagnostic::new(
            "empty_remote_url",
            "url",
            "remote URL must not be empty",
        ));
    }
    if raw_url.len() > MAX_URL_BYTES {
        return Err(Diagnostic::new(
            "remote_url_too_long",
            "url",
            "remote URL exceeds MAX_URL_BYTES",
        ));
    }
    if raw_url.chars().any(char::is_control) {
        return Err(Diagnostic::new(
            "remote_url_control_chars",
            "url",
            "remote URL must not contain control characters",
        ));
    }
    if raw_url.chars().any(char::is_whitespace) {
        return Err(Diagnostic::new(
            "remote_url_whitespace",
            "url",
            "remote URL must not contain whitespace",
        ));
    }
    if raw_url.contains('?') || raw_url.contains('#') || raw_url.contains('\\') {
        return Err(Diagnostic::new(
            "remote_url_invalid_syntax",
            "url",
            "remote URL must not contain query strings, fragments, or backslashes",
        ));
    }
    Ok(())
}

fn validate_default_ref(default_ref: Option<&str>) -> CoreResult<Option<String>> {
    let Some(r) = default_ref else {
        return Ok(None);
    };
    if r.is_empty()
        || r.len() > MAX_DEFAULT_REF_BYTES
        || r.chars().any(char::is_control)
        || r.chars().any(char::is_whitespace)
        || r.contains("..")
    {
        return Err(Diagnostic::new(
            "invalid_default_ref",
            "default_ref",
            "default_ref must be a bounded, non-empty ref without whitespace, control characters, or traversal",
        ));
    }
    Ok(Some(r.to_string()))
}

fn is_explicit_github_remote(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.starts_with("https://github.com/")
        || lower.starts_with("ssh://git@github.com/")
        || lower.starts_with("git@github.com:")
        || (lower.starts_with("https://") && lower.contains("@github.com/"))
        || (lower.starts_with("ssh://") && lower.contains("@github.com/"))
}

fn normalize_github_url(raw: &str) -> CoreResult<(String, String, String)> {
    let (scheme_kind, owner_repo_raw) =
        if let Some(rest) = strip_prefix_ignore_ascii_case(raw, "https://") {
            let Some((authority, path)) = rest.split_once('/') else {
                return Err(Diagnostic::new(
                    "invalid_github_url",
                    "url",
                    "GitHub HTTPS URL must include owner/repo",
                ));
            };
            if authority.contains('@') {
                return Err(Diagnostic::new(
                    "embedded_credentials",
                    "url",
                    "HTTPS remote URL must not embed credentials",
                ));
            }
            if !authority.eq_ignore_ascii_case("github.com") {
                return Err(Diagnostic::new(
                    "invalid_github_host",
                    "url",
                    "GitHub HTTPS URL must target github.com",
                ));
            }
            (GithubUrlKind::Https, path)
        } else if let Some(rest) = strip_prefix_ignore_ascii_case(raw, "ssh://") {
            let Some((authority, path)) = rest.split_once('/') else {
                return Err(Diagnostic::new(
                    "invalid_github_url",
                    "url",
                    "GitHub SSH URL must include owner/repo",
                ));
            };
            let Some((userinfo, host)) = authority.split_once('@') else {
                return Err(Diagnostic::new(
                    "invalid_ssh_userinfo",
                    "url",
                    "SSH GitHub URL must use git@github.com",
                ));
            };
            if userinfo != "git" {
                return Err(Diagnostic::new(
                    "embedded_credentials",
                    "url",
                    "SSH remote URL must use git@ without an embedded password or custom token",
                ));
            }
            if !host.eq_ignore_ascii_case("github.com") {
                return Err(Diagnostic::new(
                    "invalid_github_host",
                    "url",
                    "GitHub SSH URL must target github.com",
                ));
            }
            (GithubUrlKind::SshScheme, path)
        } else if let Some((left, path)) = raw.split_once(':') {
            let Some((userinfo, host)) = left.split_once('@') else {
                return Err(Diagnostic::new(
                    "invalid_github_url",
                    "url",
                    "unsupported GitHub remote URL scheme",
                ));
            };
            if userinfo != "git" {
                return Err(Diagnostic::new(
                    "embedded_credentials",
                    "url",
                    "SSH remote URL must use git@ without an embedded password or custom token",
                ));
            }
            if !host.eq_ignore_ascii_case("github.com") {
                return Err(Diagnostic::new(
                    "invalid_github_host",
                    "url",
                    "GitHub SSH remote must target github.com",
                ));
            }
            (GithubUrlKind::ScpSsh, path)
        } else if !raw.contains('@') {
            (GithubUrlKind::Https, raw)
        } else {
            return Err(Diagnostic::new(
                "invalid_github_url",
                "url",
                "invalid GitHub repository specifier",
            ));
        };

    let (owner, repo) = parse_owner_repo(owner_repo_raw)?;
    let native_id = format!("{owner}/{repo}");
    let normalized_key = format!(
        "github:{}/{}",
        owner.to_ascii_lowercase(),
        repo.to_ascii_lowercase()
    );
    let url = match scheme_kind {
        GithubUrlKind::Https => format!("https://github.com/{owner}/{repo}.git"),
        GithubUrlKind::SshScheme => format!("ssh://git@github.com/{owner}/{repo}.git"),
        GithubUrlKind::ScpSsh => format!("git@github.com:{owner}/{repo}.git"),
    };
    Ok((url, native_id, normalized_key))
}

#[derive(Clone, Copy)]
enum GithubUrlKind {
    Https,
    SshScheme,
    ScpSsh,
}

fn normalize_gitea_url(raw: &str) -> CoreResult<(String, String, String)> {
    enum GiteaScheme {
        Https,
        SshScheme,
        ScpSsh,
    }

    let (scheme, authority, path) =
        if let Some(rest) = strip_prefix_ignore_ascii_case(raw, "https://") {
            let Some((authority, path)) = rest.split_once('/') else {
                return Err(Diagnostic::new(
                    "invalid_gitea_url",
                    "url",
                    "Gitea HTTPS URL must include host and owner/repo",
                ));
            };
            if authority.contains('@') {
                return Err(Diagnostic::new(
                    "embedded_credentials",
                    "url",
                    "HTTPS remote URL must not embed credentials",
                ));
            }
            (GiteaScheme::Https, authority, path)
        } else if let Some(rest) = strip_prefix_ignore_ascii_case(raw, "ssh://") {
            let Some((authority, path)) = rest.split_once('/') else {
                return Err(Diagnostic::new(
                    "invalid_gitea_url",
                    "url",
                    "Gitea SSH URL must include host and owner/repo",
                ));
            };
            let Some((userinfo, host_port)) = authority.split_once('@') else {
                return Err(Diagnostic::new(
                    "invalid_ssh_userinfo",
                    "url",
                    "SSH Gitea URL must use git@<host>",
                ));
            };
            if userinfo != "git" {
                return Err(Diagnostic::new(
                    "embedded_credentials",
                    "url",
                    "SSH remote URL must use git@ without an embedded password or custom token",
                ));
            }
            (GiteaScheme::SshScheme, host_port, path)
        } else if let Some((left, path)) = raw.split_once(':') {
            if left.contains('/') {
                return Err(Diagnostic::new(
                    "invalid_gitea_url",
                    "url",
                    "unsupported Gitea remote URL scheme",
                ));
            }
            let Some((userinfo, host)) = left.split_once('@') else {
                return Err(Diagnostic::new(
                    "invalid_gitea_url",
                    "url",
                    "unsupported Gitea remote URL scheme",
                ));
            };
            if userinfo != "git" {
                return Err(Diagnostic::new(
                    "embedded_credentials",
                    "url",
                    "SSH remote URL must use git@ without an embedded password or custom token",
                ));
            }
            (GiteaScheme::ScpSsh, host, path)
        } else {
            return Err(Diagnostic::new(
                "invalid_gitea_url",
                "url",
                "Gitea remote requires https://, ssh://git@, or git@<host>: scheme",
            ));
        };

    let (host_lower, port) = match scheme {
        GiteaScheme::ScpSsh => {
            if authority.contains(':') {
                return Err(Diagnostic::new(
                    "invalid_gitea_host",
                    "url",
                    "SCP-style Gitea SSH remote must not embed a port or extra colon",
                ));
            }
            (validate_gitea_host(authority)?, None)
        }
        GiteaScheme::Https | GiteaScheme::SshScheme => split_and_validate_host_port(authority)?,
    };

    let (owner, repo) = parse_owner_repo(path)?;
    let native_id = format!("{host_lower}/{owner}/{repo}");
    let normalized_key = format!(
        "gitea:{host_lower}/{}/{}",
        owner.to_ascii_lowercase(),
        repo.to_ascii_lowercase()
    );
    let port_suffix = port.map_or_else(String::new, |p| format!(":{p}"));
    let url = match scheme {
        GiteaScheme::Https => format!("https://{host_lower}{port_suffix}/{owner}/{repo}.git"),
        GiteaScheme::SshScheme => {
            format!("ssh://git@{host_lower}{port_suffix}/{owner}/{repo}.git")
        }
        GiteaScheme::ScpSsh => format!("git@{host_lower}:{owner}/{repo}.git"),
    };
    Ok((url, native_id, normalized_key))
}

fn split_and_validate_host_port(authority: &str) -> CoreResult<(String, Option<u16>)> {
    if authority.starts_with('[') || authority.ends_with(']') {
        return Err(Diagnostic::new(
            "ssrf_ip_literal_rejected",
            "url",
            "IPv6 address literals are not allowed for Gitea hosts",
        ));
    }
    if let Some((host, port_str)) = authority.split_once(':') {
        if port_str.contains(':')
            || port_str.is_empty()
            || !port_str.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(Diagnostic::new(
                "invalid_gitea_port",
                "url",
                "Gitea URL port must be a valid TCP port number",
            ));
        }
        let port: u16 = port_str.parse().map_err(|_| {
            Diagnostic::new(
                "invalid_gitea_port",
                "url",
                "Gitea URL port is out of range",
            )
        })?;
        if port == 0 {
            return Err(Diagnostic::new(
                "invalid_gitea_port",
                "url",
                "Gitea URL port must be non-zero",
            ));
        }
        let host_lower = validate_gitea_host(host)?;
        Ok((host_lower, Some(port)))
    } else {
        let host_lower = validate_gitea_host(authority)?;
        Ok((host_lower, None))
    }
}

fn validate_gitea_host(host: &str) -> CoreResult<String> {
    if host.is_empty() || host.len() > MAX_DNS_HOST_BYTES {
        return Err(Diagnostic::new(
            "invalid_gitea_host",
            "url",
            "Gitea host must be non-empty and within DNS length bounds",
        ));
    }
    let lower = host.to_ascii_lowercase();
    if lower.starts_with('[') || lower.contains(':') {
        return Err(Diagnostic::new(
            "ssrf_ip_literal_rejected",
            "url",
            "IPv6 address literals are not allowed for Gitea hosts",
        ));
    }
    if lower == "localhost"
        || lower.ends_with(".localhost")
        || lower.starts_with("localhost.")
        || lower == "ip6-localhost"
        || lower == "broadcasthost"
    {
        return Err(Diagnostic::new(
            "ssrf_loopback_rejected",
            "url",
            "localhost and loopback hostnames are not allowed for Gitea remotes",
        ));
    }
    let labels: Vec<&str> = lower.split('.').collect();
    if labels.len() < 2 {
        return Err(Diagnostic::new(
            "invalid_gitea_host",
            "url",
            "Gitea host must be a valid dotted DNS or tailnet hostname",
        ));
    }
    for label in &labels {
        if label.is_empty()
            || label.len() > MAX_DNS_LABEL_BYTES
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(Diagnostic::new(
                "invalid_gitea_host",
                "url",
                "Gitea hostname contains invalid DNS label",
            ));
        }
    }
    // Requiring an ASCII letter in the final label rejects IPv4 dotted-quad SSRF literals
    // such as 127.0.0.1, 10.0.0.1, 169.254.169.254, and 192.168.1.1.
    let Some(tld) = labels.last() else {
        return Err(Diagnostic::new(
            "invalid_gitea_host",
            "url",
            "Gitea host missing TLD",
        ));
    };
    if !tld.bytes().any(|b| b.is_ascii_lowercase()) {
        return Err(Diagnostic::new(
            "ssrf_ip_literal_rejected",
            "url",
            "bare IPv4 address literals are not allowed for Gitea hosts",
        ));
    }
    Ok(lower)
}

fn normalize_radicle_rid(raw: &str) -> CoreResult<(String, String, String)> {
    let body = raw
        .strip_prefix("rad::rad://")
        .or_else(|| raw.strip_prefix("rad://"))
        .or_else(|| raw.strip_prefix("rad:"))
        .ok_or_else(|| {
            Diagnostic::new(
                "invalid_radicle_rid",
                "url",
                "Radicle RID must begin with rad:z..., rad://z..., or rad::rad://z...",
            )
        })?;

    if !body.starts_with('z') {
        return Err(Diagnostic::new(
            "invalid_radicle_rid",
            "url",
            "Radicle RID must use multibase 'z' (base58btc) prefix",
        ));
    }
    if !(RADICLE_RID_MIN_LEN..=RADICLE_RID_MAX_LEN).contains(&body.len()) {
        return Err(Diagnostic::new(
            "invalid_radicle_rid_length",
            "url",
            "Radicle RID length must be between 24 and 64 characters",
        ));
    }
    if !body.bytes().all(is_base58btc_byte) {
        return Err(Diagnostic::new(
            "invalid_radicle_rid_base58",
            "url",
            "Radicle RID contains invalid base58btc characters",
        ));
    }

    let native_id = format!("rad:{body}");
    let normalized_key = format!("radicle:{native_id}");
    Ok((native_id.clone(), native_id, normalized_key))
}

const fn is_base58btc_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() && !matches!(b, b'0' | b'O' | b'I' | b'l')
}

fn parse_owner_repo(raw_path: &str) -> CoreResult<(&str, &str)> {
    let trimmed = raw_path.strip_suffix('/').unwrap_or(raw_path);
    let Some((owner, repo_raw)) = trimmed.split_once('/') else {
        return Err(Diagnostic::new(
            "invalid_repo_path",
            "url",
            "repository path must be in <owner>/<repo> format",
        ));
    };
    if repo_raw.contains('/') {
        return Err(Diagnostic::new(
            "invalid_repo_path",
            "url",
            "repository path must not contain extra segments beyond <owner>/<repo>",
        ));
    }
    let repo = repo_raw.strip_suffix(".git").unwrap_or(repo_raw);
    if !is_valid_slug_segment(owner) || !is_valid_slug_segment(repo) {
        return Err(Diagnostic::new(
            "invalid_repo_segment",
            "url",
            "repository owner and name must be valid non-empty slug segments",
        ));
    }
    Ok((owner, repo))
}

fn is_valid_slug_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= MAX_SLUG_BYTES
        && segment != "."
        && segment != ".."
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn strip_prefix_ignore_ascii_case<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
    let head = input.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        input.get(prefix.len()..)
    } else {
        None
    }
}

fn commits_compatible(left: &str, right: &str) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

fn extract_normalized_keys(remotes: &[String]) -> Vec<(RemoteProvider, String)> {
    let mut keys = Vec::new();
    for raw in remotes {
        if let Some(pair) = normalize_any_remote_url(raw)
            && !keys.contains(&pair)
        {
            keys.push(pair);
        }
    }
    keys
}

fn find_matching_record(
    records: &[RepositoryRecord],
    copy_keys: &[(RemoteProvider, String)],
    copy: &FleetLocalCopy,
) -> Option<usize> {
    if !copy_keys.is_empty() {
        // Prefer canonical endpoint matches first for deterministic attribution.
        for (idx, record) in records.iter().enumerate() {
            if let Some(ref c) = record.canonical
                && copy_keys
                    .iter()
                    .any(|(p, k)| *p == c.provider && k == &c.normalized_key)
            {
                return Some(idx);
            }
        }
        for (idx, record) in records.iter().enumerate() {
            if record.remotes.iter().any(|r| {
                copy_keys
                    .iter()
                    .any(|(p, k)| *p == r.provider && k == &r.normalized_key)
            }) {
                return Some(idx);
            }
        }
        return None;
    }

    // For local-only records that were previously recorded without remotes, match by (machine, path) or exact path.
    for (idx, record) in records.iter().enumerate() {
        if record.canonical.is_none()
            && record.remotes.is_empty()
            && record
                .local_copies
                .iter()
                .any(|c| c.machine == copy.machine && c.path == copy.path)
        {
            return Some(idx);
        }
    }
    None
}

fn upsert_local_copy(list: &mut Vec<FleetLocalCopy>, copy: FleetLocalCopy) {
    if let Some(existing) = list
        .iter_mut()
        .find(|c| c.machine == copy.machine && c.path == copy.path)
    {
        *existing = copy;
    } else if list.len() < MAX_LOCAL_COPIES_PER_REPO {
        list.push(copy);
    }
}

fn path_slug(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    let base = trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(if trimmed.is_empty() {
            "local-repo"
        } else {
            trimmed
        });
    truncate_utf8(base, MAX_SLUG_BYTES)
}

fn truncate_utf8(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }
    let mut end = 0usize;
    for (idx, ch) in input.char_indices() {
        let next = idx + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    input.get(..end).unwrap_or_default().to_string()
}

const fn provider_priority(provider: RemoteProvider) -> u8 {
    match provider {
        RemoteProvider::Radicle => 0,
        RemoteProvider::Github => 1,
        RemoteProvider::Gitea => 2,
    }
}

const fn remote_role_rank(role: RemoteRole) -> u8 {
    match role {
        RemoteRole::Canonical => 0,
        RemoteRole::Mirror => 1,
    }
}

fn merge_non_claims(target: &mut Vec<String>, defaults: Vec<String>) {
    for claim in defaults {
        if !target.contains(&claim) && target.len() < MAX_NON_CLAIMS {
            target.push(claim);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_copy(machine: &str, path: &str, remotes: Vec<&str>) -> FleetLocalCopy {
        FleetLocalCopy {
            machine: machine.to_string(),
            path: path.to_string(),
            vcs: Some(LocalVcs::Jj),
            head_commit: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            branch_or_bookmark: Some("main".to_string()),
            dirty: Some(false),
            ahead: Some(0),
            behind: Some(0),
            remotes: remotes.into_iter().map(str::to_string).collect(),
            cairn_count: Some(2),
            observed_at_ms: Some(1_700_000_000_000),
            health: CopyHealth::Available,
            error: None,
        }
    }

    #[test]
    fn normalizes_github_shorthand_https_and_ssh() {
        let short = normalize_remote(
            RemoteProvider::Github,
            RemoteRole::Canonical,
            "OnixResearch/kiln",
            Some("main"),
        );
        let Ok(short) = short else {
            unreachable!("valid GitHub shorthand should normalize");
        };
        assert_eq!(short.native_id, "OnixResearch/kiln");
        assert_eq!(short.normalized_key, "github:onixresearch/kiln");
        assert_eq!(short.url, "https://github.com/OnixResearch/kiln.git");
        assert_eq!(short.default_ref.as_deref(), Some("main"));

        let https = normalize_remote(
            RemoteProvider::Github,
            RemoteRole::Mirror,
            "https://github.com/OnixResearch/kiln.git",
            None,
        );
        let Ok(https) = https else {
            unreachable!("valid GitHub HTTPS URL should normalize");
        };
        assert_eq!(https.normalized_key, short.normalized_key);

        let ssh_url = normalize_remote(
            RemoteProvider::Github,
            RemoteRole::Mirror,
            "ssh://git@github.com/OnixResearch/kiln.git",
            None,
        );
        let Ok(ssh_url) = ssh_url else {
            unreachable!("valid GitHub ssh:// URL should normalize");
        };
        assert_eq!(ssh_url.url, "ssh://git@github.com/OnixResearch/kiln.git");
        assert_eq!(ssh_url.normalized_key, short.normalized_key);

        let scp_ssh = normalize_remote(
            RemoteProvider::Github,
            RemoteRole::Mirror,
            "git@github.com:OnixResearch/kiln",
            None,
        );
        let Ok(scp_ssh) = scp_ssh else {
            unreachable!("valid GitHub SCP SSH URL should normalize");
        };
        assert_eq!(scp_ssh.url, "git@github.com:OnixResearch/kiln.git");
        assert_eq!(scp_ssh.normalized_key, short.normalized_key);
    }

    #[test]
    fn normalizes_gitea_urls_including_custom_port_and_tailnet() {
        let https = normalize_remote(
            RemoteProvider::Gitea,
            RemoteRole::Canonical,
            "https://Forge.Example.Org:3000/Onix/Kiln.git",
            None,
        );
        let Ok(https) = https else {
            unreachable!("valid Gitea HTTPS URL with custom port should normalize");
        };
        assert_eq!(https.native_id, "forge.example.org/Onix/Kiln");
        assert_eq!(https.normalized_key, "gitea:forge.example.org/onix/kiln");
        assert_eq!(https.url, "https://forge.example.org:3000/Onix/Kiln.git");

        let tailnet = normalize_remote(
            RemoteProvider::Gitea,
            RemoteRole::Mirror,
            "ssh://git@git.bison-tailor.ts.net:2222/brittonr/nixcfg",
            None,
        );
        let Ok(tailnet) = tailnet else {
            unreachable!("valid tailnet Gitea SSH URL should normalize");
        };
        assert_eq!(
            tailnet.normalized_key,
            "gitea:git.bison-tailor.ts.net/brittonr/nixcfg"
        );

        let scp = normalize_remote(
            RemoteProvider::Gitea,
            RemoteRole::Mirror,
            "git@git.example.org:team/service.git",
            None,
        );
        let Ok(scp) = scp else {
            unreachable!("valid Gitea SCP SSH URL should normalize");
        };
        assert_eq!(scp.normalized_key, "gitea:git.example.org/team/service");
    }

    #[test]
    fn normalizes_radicle_rids_and_preserves_base58_case() {
        let rid = "z2wsvXm5S2sJGvuV1k5JHiwi1PbKE";
        for input in [
            format!("rad:{rid}"),
            format!("rad://{rid}"),
            format!("rad::rad://{rid}"),
        ] {
            let parsed =
                normalize_remote(RemoteProvider::Radicle, RemoteRole::Canonical, &input, None);
            let Ok(ep) = parsed else {
                unreachable!("valid Radicle RID {input} should normalize");
            };
            assert_eq!(ep.native_id, format!("rad:{rid}"));
            assert_eq!(ep.normalized_key, format!("radicle:rad:{rid}"));
            assert_eq!(ep.url, format!("rad:{rid}"));
        }
    }

    #[test]
    fn rejects_malformed_urls_credentials_ssrf_and_bad_rids() {
        // Empty, whitespace, control char, oversized
        assert!(normalize_remote(RemoteProvider::Github, RemoteRole::Canonical, "", None).is_err());
        assert!(
            normalize_remote(
                RemoteProvider::Github,
                RemoteRole::Canonical,
                "https://github.com/a/b c",
                None
            )
            .is_err()
        );
        assert!(
            normalize_remote(
                RemoteProvider::Github,
                RemoteRole::Canonical,
                "https://github.com/a/b\n",
                None
            )
            .is_err()
        );
        let huge = format!("https://github.com/a/{}", "x".repeat(MAX_URL_BYTES));
        assert!(
            normalize_remote(RemoteProvider::Github, RemoteRole::Canonical, &huge, None).is_err()
        );

        // Embedded credentials
        assert!(
            normalize_remote(
                RemoteProvider::Github,
                RemoteRole::Canonical,
                "https://ghp_secret@github.com/OnixResearch/kiln.git",
                None
            )
            .is_err()
        );
        assert!(
            normalize_remote(
                RemoteProvider::Github,
                RemoteRole::Canonical,
                "https://user:pass@github.com/OnixResearch/kiln.git",
                None
            )
            .is_err()
        );
        assert!(
            normalize_remote(
                RemoteProvider::Github,
                RemoteRole::Canonical,
                "ssh://git:pass@github.com/OnixResearch/kiln.git",
                None
            )
            .is_err()
        );
        assert!(
            normalize_remote(
                RemoteProvider::Gitea,
                RemoteRole::Canonical,
                "https://user:pass@git.example.org/org/repo.git",
                None
            )
            .is_err()
        );

        // SSRF literals for Gitea
        for ssrf in [
            "https://127.0.0.1/org/repo.git",
            "https://169.254.169.254/org/repo.git",
            "https://10.0.0.5:3000/org/repo.git",
            "https://[::1]/org/repo.git",
            "https://localhost/org/repo.git",
            "https://sub.localhost/org/repo.git",
            "https://gitea/org/repo.git",
            "http://git.example.org/org/repo.git",
        ] {
            assert!(
                normalize_remote(RemoteProvider::Gitea, RemoteRole::Canonical, ssrf, None).is_err(),
                "expected SSRF/insecure Gitea URL to be rejected: {ssrf}"
            );
        }

        // Bad Radicle RIDs: wrong prefix, invalid Base58 chars (0, O, I, l), too short/long
        for bad_rid in [
            "rad:x2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
            "rad:z0OIlXm5S2sJGvuV1k5JHiwi1PbKE",
            "rad:zshort12345",
            &format!("rad:z{}", "a".repeat(70)),
            "https://app.radicle.xyz/nodes/ash.radicle.garden/rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
        ] {
            assert!(
                normalize_remote(
                    RemoteProvider::Radicle,
                    RemoteRole::Canonical,
                    bad_rid,
                    None
                )
                .is_err(),
                "expected malformed Radicle RID to be rejected: {bad_rid}"
            );
        }
    }

    #[test]
    fn canonical_identity_is_deterministic_blake3() {
        let id1 = canonical_repository_identity("github:onixresearch/kiln");
        let id2 = canonical_repository_identity("github:onixresearch/kiln");
        let id3 = canonical_repository_identity("radicle:rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
        assert!(id1.validate().is_ok());
    }

    #[test]
    fn validate_remote_set_rejects_duplicate_canonical_and_duplicate_remotes() {
        let Ok(r1) = normalize_remote(
            RemoteProvider::Radicle,
            RemoteRole::Canonical,
            "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
            None,
        ) else {
            unreachable!();
        };
        let Ok(r2) = normalize_remote(
            RemoteProvider::Github,
            RemoteRole::Canonical,
            "OnixResearch/kiln",
            None,
        ) else {
            unreachable!();
        };
        assert!(validate_remote_set(&[r1.clone(), r2]).is_err());

        let Ok(m1) = normalize_remote(
            RemoteProvider::Github,
            RemoteRole::Mirror,
            "https://github.com/OnixResearch/kiln",
            None,
        ) else {
            unreachable!();
        };
        let Ok(m2) = normalize_remote(
            RemoteProvider::Github,
            RemoteRole::Mirror,
            "git@github.com:onixresearch/kiln.git",
            None,
        ) else {
            unreachable!();
        };
        assert!(validate_remote_set(&[r1, m1.clone(), m2]).is_err());

        let mut too_many = Vec::new();
        for idx in 0..=MAX_REMOTES_PER_REPO {
            let Ok(ep) = normalize_remote(
                RemoteProvider::Github,
                RemoteRole::Mirror,
                &format!("org/repo-{idx}"),
                None,
            ) else {
                unreachable!();
            };
            too_many.push(ep);
        }
        assert!(validate_remote_set(&too_many).is_err());
        assert!(validate_remote_set(&[m1]).is_ok());
    }

    #[test]
    fn normalize_any_remote_url_parses_checkout_remotes() {
        assert_eq!(
            normalize_any_remote_url("git@github.com:OnixResearch/kiln.git"),
            Some((
                RemoteProvider::Github,
                "github:onixresearch/kiln".to_string()
            ))
        );
        assert_eq!(
            normalize_any_remote_url("rad://z2wsvXm5S2sJGvuV1k5JHiwi1PbKE"),
            Some((
                RemoteProvider::Radicle,
                "radicle:rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE".to_string()
            ))
        );
        assert_eq!(
            normalize_any_remote_url("https://git.bison-tailor.ts.net/brittonr/kiln.git"),
            Some((
                RemoteProvider::Gitea,
                "gitea:git.bison-tailor.ts.net/brittonr/kiln".to_string()
            ))
        );
        assert_eq!(normalize_any_remote_url("/home/brittonr/local-repo"), None);
        assert_eq!(
            normalize_any_remote_url("https://token@github.com/OnixResearch/kiln.git"),
            None
        );
    }

    #[test]
    fn reconcile_catalog_matches_copies_and_computes_honest_sync_status() {
        let Ok(rad) = normalize_remote(
            RemoteProvider::Radicle,
            RemoteRole::Canonical,
            "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
            Some("main"),
        ) else {
            unreachable!();
        };
        let Ok(gh) = normalize_remote(
            RemoteProvider::Github,
            RemoteRole::Mirror,
            "OnixResearch/kiln",
            Some("main"),
        ) else {
            unreachable!();
        };
        let Ok(record) = build_repository_record(
            rad,
            vec![gh],
            Some("Kiln".to_string()),
            RepositoryAdmission {
                catalog_imported: true,
                ci_configured: true,
                source_configured: true,
                fleet_enrolled: true,
            },
        ) else {
            unreachable!();
        };

        // 1. All copies available, clean, matching HEAD -> Synced
        let c1 = sample_copy(
            "britton-desktop",
            "/home/brittonr/git/OnixResearch/kiln",
            vec!["https://github.com/OnixResearch/kiln.git"],
        );
        let mut c2 = sample_copy(
            "aspen3",
            "/home/brittonr/git/kiln",
            vec!["rad://z2wsvXm5S2sJGvuV1k5JHiwi1PbKE"],
        );
        c2.cairn_count = Some(3);

        let synced = reconcile_catalog(core::slice::from_ref(&record), &[c1.clone(), c2.clone()]);
        assert_eq!(synced.len(), 1);
        let Some(first) = synced.first() else {
            unreachable!();
        };
        assert_eq!(first.sync_status, FleetSyncStatus::Synced);
        assert_eq!(first.total_cairns, 5);
        assert_eq!(first.local_copies.len(), 2);

        // 2. Unavailable copy must NEVER collapse into Synced -> Unknown
        let mut offline = sample_copy(
            "aspen1",
            "/home/brittonr/git/kiln",
            vec!["https://github.com/OnixResearch/kiln.git"],
        );
        offline.health = CopyHealth::Unavailable;
        offline.head_commit = None;
        offline.cairn_count = None;
        offline.error = Some("SSH unreachable".to_string());
        let with_offline = reconcile_catalog(
            core::slice::from_ref(&record),
            &[c1.clone(), offline.clone()],
        );
        assert_eq!(
            with_offline.first().map(|r| r.sync_status),
            Some(FleetSyncStatus::Unknown)
        );

        // 3. Dirty + Unavailable -> Dirty (prioritizes Dirty > Unknown)
        let mut dirty_c1 = c1.clone();
        dirty_c1.dirty = Some(true);
        let with_dirty_and_offline =
            reconcile_catalog(core::slice::from_ref(&record), &[dirty_c1, offline.clone()]);
        assert_eq!(
            with_dirty_and_offline.first().map(|r| r.sync_status),
            Some(FleetSyncStatus::Dirty)
        );

        // 4. Diverged + Dirty + Unavailable -> Diverged (highest priority)
        let mut diverged_c2 = c2.clone();
        diverged_c2.head_commit = Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string());
        let mut dirty_c1_for_div = c1;
        dirty_c1_for_div.dirty = Some(true);
        let with_diverged = reconcile_catalog(
            core::slice::from_ref(&record),
            &[dirty_c1_for_div, diverged_c2, offline],
        );
        assert_eq!(
            with_diverged.first().map(|r| r.sync_status),
            Some(FleetSyncStatus::Diverged)
        );
    }

    #[test]
    fn reconcile_catalog_groups_unmatched_local_only_and_observed_remote_copies() {
        let local_only = sample_copy("britton-desktop", "/home/brittonr/scratch/notes", vec![]);
        let discovered_remote = sample_copy(
            "aspen3",
            "/home/brittonr/src/cairn",
            vec!["git@github.com:OnixResearch/cairn.git"],
        );
        let out = reconcile_catalog(&[], &[local_only, discovered_remote]);
        assert_eq!(out.len(), 2);

        let local_rec = out.iter().find(|r| r.slug == "notes");
        let Some(local_rec) = local_rec else {
            unreachable!("expected local-only record for notes");
        };
        assert_eq!(local_rec.sync_status, FleetSyncStatus::LocalOnly);
        assert!(!local_rec.admission.catalog_imported);

        let remote_rec = out.iter().find(|r| r.slug == "OnixResearch/cairn");
        let Some(remote_rec) = remote_rec else {
            unreachable!("expected observed-remote record for OnixResearch/cairn");
        };
        assert_eq!(
            remote_rec.identity,
            canonical_repository_identity("github:onixresearch/cairn")
        );
        assert_eq!(remote_rec.sync_status, FleetSyncStatus::Synced);
        assert!(!remote_rec.admission.catalog_imported);
    }

    #[test]
    fn serde_round_trip_matches_expected_snake_case_shape() {
        let Ok(rad) = normalize_remote(
            RemoteProvider::Radicle,
            RemoteRole::Canonical,
            "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
            None,
        ) else {
            unreachable!();
        };
        let Ok(rec) = build_repository_record(rad, vec![], None, RepositoryAdmission::default())
        else {
            unreachable!();
        };
        let Ok(json) = serde_json::to_string(&rec) else {
            unreachable!();
        };
        assert!(json.contains("\"provider\":\"radicle\""));
        assert!(json.contains("\"role\":\"canonical\""));
        assert!(json.contains("\"status\":\"declared\""));
        assert!(json.contains("\"sync_status\":\"unknown\""));

        let Ok(roundtrip) = serde_json::from_str::<RepositoryRecord>(&json) else {
            unreachable!();
        };
        assert_eq!(roundtrip, rec);
    }
}
