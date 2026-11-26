//! Read-only Fleet local-copy census across configured machines and scan roots.
//!
//! Inspects configured checkouts in `inventory.json` and optional shallow
//! `repository_scan_roots` (`1..=2` depth) on the coordinator host without
//! mutating working trees, running remote fetches, or invoking Radicle
//! mutations. Remote fleet hosts that cannot be observed locally are reported
//! explicitly with `health: "unavailable"` and a clear diagnostic.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use kiln_adapter_herdr::{HerdrInventory, HerdrMachine};
use kiln_core::repository_catalog::{
    CopyHealth, FleetLocalCopy, LocalVcs, MAX_PATH_BYTES, validate_local_copy,
};
use serde::{Deserialize, Serialize};

use crate::config::{FleetConfig, read_bounded};
use crate::exec::Child;

pub const REPOSITORIES_SCHEMA: &str = "kiln.fleet-repositories.v1";
pub const MAX_LOCAL_COPIES_PER_HOST: usize = 128;
pub const MAX_SCAN_DEPTH: usize = 2;
pub const MAX_SCAN_ENTRIES_PER_DIR: usize = 256;
pub const MAX_REMOTES_PER_COPY: usize = 16;
pub const MAX_CAIRN_CHANGES: usize = 256;
pub const MAX_GIT_CONFIG_BYTES: u64 = 256 * 1_024;
pub const MAX_INVENTORY_BYTES: u64 = 1_024 * 1_024;
pub const MAX_ERROR_BYTES: usize = 240;
pub const MAX_VCS_OUTPUT_BYTES: usize = 256 * 1_024;
pub const VCS_INSPECT_TIMEOUT_MS: u64 = 5_000;
pub const COMMIT_HEX_LENGTHS: [usize; 2] = [40, 64];
const MAX_OPERATION_HEADS_SCAN: usize = 64;
const OPERATION_ID_HEX: usize = 128;

/// Reachability status of one fleet host during repository census.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostHealth {
    Available,
    Unavailable,
}

/// Per-host census summary in `GET /v1/repositories`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostCensus {
    pub machine: String,
    pub health: HostHealth,
    pub error: Option<String>,
    pub copy_count: usize,
}

/// Bounded response payload for `GET /v1/repositories`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FleetRepositoriesResponse {
    pub schema: String,
    pub observed_at_ms: u64,
    pub hosts: Vec<HostCensus>,
    pub copies: Vec<FleetLocalCopy>,
    pub bounded: bool,
    pub truncated: bool,
}

#[derive(Clone, Debug, Default)]
struct VcsTools {
    git: Option<PathBuf>,
    jj: Option<PathBuf>,
    pijul: Option<PathBuf>,
    environment: Vec<(OsString, OsString)>,
}

/// Collect the read-only Fleet local-copy census for the coordinator and all
/// configured peer/inventory hosts.
// r[impl kiln.fleet.local_copy_census]
#[must_use]
pub fn collect_repositories(
    config: &FleetConfig,
    attached_inventory: Option<&HerdrInventory>,
    now_ms: u64,
) -> FleetRepositoriesResponse {
    let (loaded_inventory, inventory_error) = match attached_inventory {
        Some(inv) => (Some(inv.clone()), None),
        None => load_inventory_if_present(&config.inventory),
    };
    let inventory = loaded_inventory.as_ref();
    let tools = resolve_tools(config, inventory);

    let mut all_machines: BTreeSet<String> =
        config.machines().into_iter().map(str::to_owned).collect();
    if let Some(inv) = inventory {
        for machine in &inv.machines {
            all_machines.insert(machine.label.clone());
        }
    }

    let coordinator_inventory = inventory.and_then(|inv| inv.machine(&config.machine));
    let (local_candidates, local_scan_truncated) =
        discover_local_checkouts(coordinator_inventory, &config.repository_scan_roots);

    let mut local_copies = Vec::with_capacity(local_candidates.len());
    for candidate in &local_candidates {
        let copy = inspect_local_checkout(&config.machine, candidate, &tools, now_ms);
        if validate_local_copy(&copy).is_ok() {
            local_copies.push(copy);
        }
    }

    let mut remote_hosts = Vec::new();
    for machine_name in &all_machines {
        if machine_name == &config.machine {
            continue;
        }
        let inv_machine = inventory.and_then(|inv| inv.machine(machine_name));
        let (error, copies, remote_truncated) = project_remote_host(
            machine_name,
            inv_machine,
            inventory_error.as_deref(),
            now_ms,
        );
        remote_hosts.push((machine_name.clone(), error, copies, remote_truncated));
    }

    assemble_census(
        &config.machine,
        all_machines,
        local_copies,
        inventory_error.filter(|_| coordinator_inventory.is_none() && local_candidates.is_empty()),
        local_scan_truncated,
        remote_hosts,
        now_ms,
    )
}

/// Pure assembly of host summaries, copies, and truncation state.
#[must_use]
pub fn assemble_census(
    coordinator_machine: &str,
    all_machines: BTreeSet<String>,
    mut local_copies: Vec<FleetLocalCopy>,
    local_error: Option<String>,
    mut truncated: bool,
    remote_hosts: Vec<(String, String, Vec<FleetLocalCopy>, bool)>,
    now_ms: u64,
) -> FleetRepositoriesResponse {
    local_copies.sort_by(|a, b| a.machine.cmp(&b.machine).then_with(|| a.path.cmp(&b.path)));
    if local_copies.len() > MAX_LOCAL_COPIES_PER_HOST {
        local_copies.truncate(MAX_LOCAL_COPIES_PER_HOST);
        truncated = true;
    }

    let mut remote_map: BTreeMap<String, (String, Vec<FleetLocalCopy>)> = BTreeMap::new();
    for (machine, error, mut copies, remote_truncated) in remote_hosts {
        if remote_truncated || copies.len() > MAX_LOCAL_COPIES_PER_HOST {
            truncated = true;
        }
        copies.sort_by(|a, b| a.path.cmp(&b.path));
        copies.truncate(MAX_LOCAL_COPIES_PER_HOST);
        remote_map.insert(machine, (error, copies));
    }

    let mut hosts = Vec::with_capacity(all_machines.len());
    let mut copies = Vec::new();

    for machine in all_machines {
        if machine == coordinator_machine {
            let count = local_copies.len();
            let health = if local_error.is_some() && count == 0 {
                HostHealth::Unavailable
            } else {
                HostHealth::Available
            };
            hosts.push(HostCensus {
                machine,
                health,
                error: if health == HostHealth::Unavailable {
                    local_error.clone()
                } else {
                    None
                },
                copy_count: count,
            });
            copies.extend(local_copies.iter().cloned());
        } else if let Some((error, mut remote_copies)) = remote_map.remove(&machine) {
            let count = remote_copies.len();
            hosts.push(HostCensus {
                machine,
                health: HostHealth::Unavailable,
                error: Some(error),
                copy_count: count,
            });
            copies.append(&mut remote_copies);
        } else {
            hosts.push(HostCensus {
                machine,
                health: HostHealth::Unavailable,
                error: Some("machine is not configured in herdr inventory".to_owned()),
                copy_count: 0,
            });
        }
    }

    copies.sort_by(|a, b| a.machine.cmp(&b.machine).then_with(|| a.path.cmp(&b.path)));

    FleetRepositoriesResponse {
        schema: REPOSITORIES_SCHEMA.to_owned(),
        observed_at_ms: now_ms,
        hosts,
        copies,
        bounded: true,
        truncated,
    }
}

fn load_inventory_if_present(path: &Path) -> (Option<HerdrInventory>, Option<String>) {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => (
            None,
            Some("fleet inventory path must not be a symlink".to_owned()),
        ),
        Ok(_) => match read_bounded(path, MAX_INVENTORY_BYTES)
            .and_then(|bytes| {
                std::str::from_utf8(&bytes)
                    .map(str::to_owned)
                    .map_err(|_| "fleet inventory is not UTF-8".to_owned())
            })
            .and_then(|text| HerdrInventory::from_json(&text))
        {
            Ok(inv) => (Some(inv), None),
            Err(err) => (None, Some(sanitize_error(&err))),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (None, None),
        Err(err) => (
            None,
            Some(sanitize_error(&format!(
                "fleet inventory is unreadable: {err}"
            ))),
        ),
    }
}

fn project_remote_host(
    machine_name: &str,
    inv_machine: Option<&HerdrMachine>,
    inventory_error: Option<&str>,
    now_ms: u64,
) -> (String, Vec<FleetLocalCopy>, bool) {
    let Some(machine) = inv_machine else {
        let error = inventory_error.map_or_else(
            || "machine is not configured in herdr inventory".to_owned(),
            str::to_owned,
        );
        return (error, Vec::new(), false);
    };

    let error = remote_host_error(machine);
    let total = machine.repositories.len();
    let truncated = total > MAX_LOCAL_COPIES_PER_HOST;
    let mut copies = Vec::with_capacity(total.min(MAX_LOCAL_COPIES_PER_HOST));
    for checkout_path in machine
        .repositories
        .values()
        .take(MAX_LOCAL_COPIES_PER_HOST)
    {
        let copy = FleetLocalCopy {
            machine: machine_name.to_owned(),
            path: truncate_utf8(checkout_path, MAX_PATH_BYTES),
            vcs: None,
            head_commit: None,
            branch_or_bookmark: None,
            dirty: None,
            ahead: None,
            behind: None,
            remotes: Vec::new(),
            cairn_count: None,
            observed_at_ms: Some(now_ms),
            health: CopyHealth::Unavailable,
            error: Some(error.clone()),
        };
        if validate_local_copy(&copy).is_ok() {
            copies.push(copy);
        }
    }
    (error, copies, truncated)
}

/// Pure diagnostic explaining why a remote host cannot be locally observed.
#[must_use]
pub fn remote_host_error(machine: &HerdrMachine) -> String {
    match (&machine.herdr_machine, &machine.ssh_target) {
        (None, None) => {
            "remote host has no herdr_machine or ssh_target configured for checkout census"
                .to_owned()
        }
        (Some(herdr), _) => {
            format!("remote host {herdr} is unreachable or offline for read-only repository census")
        }
        (None, Some(ssh)) => {
            format!("remote host ({ssh}) is unreachable or offline for read-only repository census")
        }
    }
}

fn discover_local_checkouts(
    coordinator_inventory: Option<&HerdrMachine>,
    scan_roots: &[PathBuf],
) -> (Vec<PathBuf>, bool) {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut truncated = false;

    if let Some(machine) = coordinator_inventory {
        for raw_path in machine.repositories.values() {
            if candidates.len() >= MAX_LOCAL_COPIES_PER_HOST {
                truncated = true;
                break;
            }
            let path = PathBuf::from(raw_path);
            let key = canonical_or_owned(&path);
            if seen.insert(key) {
                candidates.push(path);
            }
        }
    }

    for root in scan_roots {
        if candidates.len() >= MAX_LOCAL_COPIES_PER_HOST {
            truncated = true;
            break;
        }
        let Ok(meta) = fs::symlink_metadata(root) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        if scan_shallow_root(root, &mut candidates, &mut seen) {
            truncated = true;
        }
    }

    (candidates, truncated)
}

fn scan_shallow_root(
    root: &Path,
    candidates: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
) -> bool {
    let mut truncated = false;
    for depth1 in sorted_dir_entries(root, &mut truncated) {
        if candidates.len() >= MAX_LOCAL_COPIES_PER_HOST {
            return true;
        }
        if detect_vcs(&depth1).is_some() {
            let key = canonical_or_owned(&depth1);
            if seen.insert(key) {
                candidates.push(depth1);
            }
            // Never descend into subdirectories of an already-detected repository.
            continue;
        }
        if MAX_SCAN_DEPTH >= 2 {
            for depth2 in sorted_dir_entries(&depth1, &mut truncated) {
                if candidates.len() >= MAX_LOCAL_COPIES_PER_HOST {
                    return true;
                }
                if detect_vcs(&depth2).is_some() {
                    let key = canonical_or_owned(&depth2);
                    if seen.insert(key) {
                        candidates.push(depth2);
                    }
                }
            }
        }
    }
    truncated
}

fn sorted_dir_entries(dir: &Path, truncated: &mut bool) -> Vec<PathBuf> {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for (index, entry) in read_dir.enumerate() {
        if index >= MAX_SCAN_ENTRIES_PER_DIR {
            *truncated = true;
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        entries.push(path);
    }
    entries.sort();
    entries
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Detect VCS kind in priority order: `.jj` -> `jj`, `.pijul` -> `pijul`, `.git` -> `git`.
#[must_use]
pub fn detect_vcs(path: &Path) -> Option<LocalVcs> {
    if is_plain_dir(&path.join(".jj")) {
        return Some(LocalVcs::Jj);
    }
    if is_plain_dir(&path.join(".pijul")) {
        return Some(LocalVcs::Pijul);
    }
    if is_git_marker(&path.join(".git")) {
        return Some(LocalVcs::Git);
    }
    None
}

fn is_plain_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| !meta.file_type().is_symlink() && meta.is_dir())
}

fn is_git_marker(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|meta| !meta.file_type().is_symlink() && (meta.is_dir() || meta.is_file()))
}

fn resolve_tools(config: &FleetConfig, inventory: Option<&HerdrInventory>) -> VcsTools {
    let mut environment: Vec<(OsString, OsString)> = Vec::new();
    let mut search_paths: Vec<PathBuf> = Vec::new();

    if let Some(inv) = inventory {
        for (key, value) in inv.environment.entries() {
            environment.push((OsString::from(key), OsString::from(value)));
        }
        search_paths.extend(std::env::split_paths(&inv.environment.path));
    } else {
        if let Some(home) = std::env::var_os("HOME") {
            environment.push((OsString::from("HOME"), home));
        }
        if let Some(path) = std::env::var_os("PATH") {
            environment.push((OsString::from("PATH"), path));
        }
    }
    if let Some(env_path) = std::env::var_os("PATH") {
        search_paths.extend(std::env::split_paths(&env_path));
    }

    let git = config
        .capture
        .as_ref()
        .map(|capture| capture.git.clone())
        .filter(|path| path.is_file())
        .or_else(|| find_binary("git", &search_paths));
    let jj = find_binary("jj", &search_paths);
    let pijul = find_binary("pijul", &search_paths);

    VcsTools {
        git,
        jj,
        pijul,
        environment,
    }
}

fn find_binary(name: &str, search_paths: &[PathBuf]) -> Option<PathBuf> {
    for dir in search_paths {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    for fallback in [
        "/run/current-system/sw/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
    ] {
        let candidate = Path::new(fallback).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn inspect_local_checkout(
    machine: &str,
    path: &Path,
    tools: &VcsTools,
    now_ms: u64,
) -> FleetLocalCopy {
    let path_string = truncate_utf8(&path.to_string_lossy(), MAX_PATH_BYTES);
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) => {
            return unavailable_copy(
                machine,
                path_string,
                None,
                None,
                now_ms,
                format!("checkout path is unavailable: {err}"),
            );
        }
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return unavailable_copy(
            machine,
            path_string,
            None,
            None,
            now_ms,
            "checkout path must be a plain directory",
        );
    }

    let cairn_count = count_cairn_changes(path);
    let Some(vcs) = detect_vcs(path) else {
        return unavailable_copy(
            machine,
            path_string,
            None,
            cairn_count,
            now_ms,
            "checkout directory contains no .jj, .pijul, or .git repository",
        );
    };

    match vcs {
        LocalVcs::Git => {
            inspect_git_checkout(machine, path, path_string, cairn_count, tools, now_ms)
        }
        LocalVcs::Jj => inspect_jj_checkout(machine, path, path_string, cairn_count, tools, now_ms),
        LocalVcs::Pijul => {
            inspect_pijul_checkout(machine, path, path_string, cairn_count, tools, now_ms)
        }
    }
}

fn unavailable_copy(
    machine: &str,
    path: String,
    vcs: Option<LocalVcs>,
    cairn_count: Option<u32>,
    now_ms: u64,
    error: impl AsRef<str>,
) -> FleetLocalCopy {
    FleetLocalCopy {
        machine: machine.to_owned(),
        path,
        vcs,
        head_commit: None,
        branch_or_bookmark: None,
        dirty: None,
        ahead: None,
        behind: None,
        remotes: Vec::new(),
        cairn_count,
        observed_at_ms: Some(now_ms),
        health: CopyHealth::Unavailable,
        error: Some(sanitize_error(error.as_ref())),
    }
}

/// Count immediate subdirectories under `<repo>/.cairn/changes/` (read-only).
#[must_use]
pub fn count_cairn_changes(repo_path: &Path) -> Option<u32> {
    let changes_dir = repo_path.join(".cairn").join("changes");
    let meta = match fs::symlink_metadata(&changes_dir) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Some(0),
        Err(_) => return None,
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return None;
    }
    let Ok(entries) = fs::read_dir(&changes_dir) else {
        return None;
    };
    let mut count = 0u32;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_CAIRN_CHANGES {
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(entry_meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !entry_meta.file_type().is_symlink() && entry_meta.is_dir() {
            count = count.saturating_add(1);
        }
    }
    Some(count)
}

fn run_git_readonly(
    git_bin: &Path,
    working_dir: &Path,
    git_dir_override: Option<&Path>,
    args: &[&str],
    tools: &VcsTools,
) -> Result<String, String> {
    let mut child = Child::new(
        git_bin,
        working_dir,
        VCS_INSPECT_TIMEOUT_MS,
        MAX_VCS_OUTPUT_BYTES,
    );
    for (name, value) in &tools.environment {
        child = child.env(name, value);
    }
    // Strict read-only git flags: never write index.lock and never prompt.
    child = child
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
        .arg("-c")
        .arg("safe.directory=*");
    if let Some(git_dir) = git_dir_override {
        child = child.arg("--git-dir").arg(git_dir);
    } else {
        child = child.arg("-C").arg(working_dir);
    }
    child = child.args(args.iter().copied());
    child.run()?.require_success()?.stdout_text()
}

fn run_git_readonly_raw(
    git_bin: &Path,
    working_dir: &Path,
    args: &[&str],
    tools: &VcsTools,
) -> Result<String, String> {
    let mut child = Child::new(
        git_bin,
        working_dir,
        VCS_INSPECT_TIMEOUT_MS,
        MAX_VCS_OUTPUT_BYTES,
    );
    for (name, value) in &tools.environment {
        child = child.env(name, value);
    }
    child = child
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
        .arg("-c")
        .arg("safe.directory=*")
        .arg("-C")
        .arg(working_dir)
        .args(args.iter().copied());
    let finished = child.run()?.require_success()?;
    std::str::from_utf8(&finished.stdout)
        .map(str::to_owned)
        .map_err(|_| "git output is not UTF-8".to_owned())
}

fn inspect_git_checkout(
    machine: &str,
    path: &Path,
    path_string: String,
    cairn_count: Option<u32>,
    tools: &VcsTools,
    now_ms: u64,
) -> FleetLocalCopy {
    let mut head_commit = None;
    let mut branch_or_bookmark = None;
    let mut dirty = None;
    let mut ahead = None;
    let mut behind = None;
    let mut remotes = Vec::new();

    if let Some(git_bin) = tools.git.as_deref() {
        if let Ok(rev) = run_git_readonly(
            git_bin,
            path,
            None,
            &["rev-parse", "--verify", "HEAD"],
            tools,
        ) && is_commit_hex(&rev)
        {
            head_commit = Some(rev);
        }
        if let Ok(branch) = run_git_readonly(
            git_bin,
            path,
            None,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
            tools,
        ) && !branch.is_empty()
        {
            branch_or_bookmark = Some(branch);
        }
        if let Ok(porcelain) =
            run_git_readonly_raw(git_bin, path, &["status", "--porcelain=v1"], tools)
        {
            dirty = Some(parse_git_status_dirty(&porcelain));
        }
        if let Ok(counts) = run_git_readonly(
            git_bin,
            path,
            None,
            &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
            tools,
        ) {
            (ahead, behind) = parse_git_ahead_behind(&counts);
        }
        if let Ok(remote_out) = run_git_readonly_raw(git_bin, path, &["remote", "-v"], tools) {
            remotes = parse_git_remote_v(&remote_out);
        }
    }

    if remotes.is_empty() {
        remotes = read_git_config_remotes_at(&path.join(".git").join("config"));
    }
    if branch_or_bookmark.is_none()
        && head_commit.is_none()
        && let Some((fallback_commit, fallback_branch)) = read_git_head_fallback(path)
    {
        head_commit = fallback_commit;
        branch_or_bookmark = fallback_branch;
    }

    FleetLocalCopy {
        machine: machine.to_owned(),
        path: path_string,
        vcs: Some(LocalVcs::Git),
        head_commit,
        branch_or_bookmark,
        dirty,
        ahead,
        behind,
        remotes,
        cairn_count,
        observed_at_ms: Some(now_ms),
        health: CopyHealth::Available,
        error: None,
    }
}

fn inspect_jj_checkout(
    machine: &str,
    path: &Path,
    path_string: String,
    cairn_count: Option<u32>,
    tools: &VcsTools,
    now_ms: u64,
) -> FleetLocalCopy {
    let mut head_commit = None;
    let mut branch_or_bookmark = None;
    let mut dirty = None;
    let mut remotes = Vec::new();

    if let Some(jj_bin) = tools.jj.as_deref()
        && path.join(".jj").join("repo").exists()
        && let Ok(output) = run_jj_readonly_log(jj_bin, path, tools)
    {
        let (jj_commit, jj_bookmark, jj_dirty) = parse_jj_log_line(&output);
        head_commit = jj_commit;
        branch_or_bookmark = jj_bookmark;
        dirty = jj_dirty;
    }

    let git_store = find_jj_git_store(path);
    if let Some(git_bin) = tools.git.as_deref() {
        if is_git_marker(&path.join(".git")) {
            if let Ok(porcelain) =
                run_git_readonly_raw(git_bin, path, &["status", "--porcelain=v1"], tools)
            {
                let git_dirty = parse_git_status_dirty(&porcelain);
                dirty = Some(dirty.unwrap_or(false) || git_dirty);
            }
            if head_commit.is_none()
                && let Ok(rev) = run_git_readonly(
                    git_bin,
                    path,
                    None,
                    &["rev-parse", "--verify", "HEAD"],
                    tools,
                )
                && is_commit_hex(&rev)
            {
                head_commit = Some(rev);
            }
            if branch_or_bookmark.is_none()
                && let Ok(branch) = run_git_readonly(
                    git_bin,
                    path,
                    None,
                    &["symbolic-ref", "--quiet", "--short", "HEAD"],
                    tools,
                )
                && !branch.is_empty()
            {
                branch_or_bookmark = Some(branch);
            }
            if let Ok(remote_out) = run_git_readonly_raw(git_bin, path, &["remote", "-v"], tools) {
                remotes = parse_git_remote_v(&remote_out);
            }
        } else if let Some(ref store) = git_store
            && let Ok(remote_out) =
                run_git_readonly(git_bin, path, Some(store), &["remote", "-v"], tools)
        {
            remotes = parse_git_remote_v(&remote_out);
        }
    }

    if remotes.is_empty() {
        if let Some(ref store) = git_store {
            remotes = read_git_config_remotes_at(&store.join("config"));
        }
        if remotes.is_empty() && is_plain_dir(&path.join(".git")) {
            remotes = read_git_config_remotes_at(&path.join(".git").join("config"));
        }
    }
    if head_commit.is_none()
        && branch_or_bookmark.is_none()
        && is_plain_dir(&path.join(".git"))
        && let Some((fallback_commit, fallback_branch)) = read_git_head_fallback(path)
    {
        head_commit = fallback_commit;
        branch_or_bookmark = fallback_branch;
    }

    FleetLocalCopy {
        machine: machine.to_owned(),
        path: path_string,
        vcs: Some(LocalVcs::Jj),
        head_commit,
        branch_or_bookmark,
        dirty,
        ahead: None,
        behind: None,
        remotes,
        cairn_count,
        observed_at_ms: Some(now_ms),
        health: CopyHealth::Available,
        error: None,
    }
}

fn run_jj_readonly_log(
    jj_bin: &Path,
    workspace: &Path,
    tools: &VcsTools,
) -> Result<String, String> {
    let mut child = Child::new(
        jj_bin,
        workspace,
        VCS_INSPECT_TIMEOUT_MS,
        MAX_VCS_OUTPUT_BYTES,
    );
    for (name, value) in &tools.environment {
        child = child.env(name, value);
    }
    child = child
        .env("JJ_CONFIG", "/dev/null")
        .arg("--repository")
        .arg(workspace)
        .arg("--ignore-working-copy");
    if let Some(op) = single_jj_operation_head(workspace) {
        child = child.arg("--at-op").arg(op);
    }
    let template = r#"commit_id ++ "\t" ++ empty ++ "\t" ++ local_bookmarks.map(|b| b.name()).join(",") ++ "\t" ++ parents.map(|p| p.local_bookmarks().map(|b| b.name()).join(",")).join(",") ++ "\n""#;
    child = child.args([
        "--no-pager",
        "--color",
        "never",
        "log",
        "-r",
        "@",
        "--no-graph",
        "-T",
        template,
    ]);
    child.run()?.require_success()?.stdout_text()
}

fn single_jj_operation_head(workspace: &Path) -> Option<String> {
    let repo_dir = resolve_jj_repo_dir(workspace)?;
    let heads_dir = repo_dir.join("op_heads").join("heads");
    let entries = fs::read_dir(heads_dir).ok()?;
    let mut names = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= MAX_OPERATION_HEADS_SCAN {
            return None;
        }
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.len() == OPERATION_ID_HEX
            && name
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            names.push(name);
        } else {
            return None;
        }
    }
    match names.as_slice() {
        [single] => Some(single.clone()),
        _ => None,
    }
}

fn resolve_jj_repo_dir(workspace: &Path) -> Option<PathBuf> {
    let dot = workspace.join(".jj");
    let repo = dot.join("repo");
    let meta = fs::symlink_metadata(&repo).ok()?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        return Some(repo);
    }
    if meta.is_file() && !meta.file_type().is_symlink() {
        let bytes = read_bounded(&repo, MAX_GIT_CONFIG_BYTES).ok()?;
        let text = std::str::from_utf8(&bytes)
            .ok()?
            .trim_end_matches(['\r', '\n']);
        let target = PathBuf::from(text);
        return Some(if target.is_absolute() {
            target
        } else {
            dot.join(target)
        });
    }
    None
}

fn find_jj_git_store(workspace: &Path) -> Option<PathBuf> {
    let repo = resolve_jj_repo_dir(workspace)?;
    let store = repo.join("store");
    let backend_type = read_bounded(&store.join("type"), MAX_GIT_CONFIG_BYTES).ok()?;
    if std::str::from_utf8(&backend_type).ok()?.trim() != "git" {
        return None;
    }
    let git_target = read_bounded(&store.join("git_target"), MAX_GIT_CONFIG_BYTES).ok()?;
    let target_text = std::str::from_utf8(&git_target)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    let target = PathBuf::from(target_text);
    let resolved = if target.is_absolute() {
        target
    } else {
        store.join(target)
    };
    resolved.is_dir().then_some(resolved)
}

fn inspect_pijul_checkout(
    machine: &str,
    path: &Path,
    path_string: String,
    cairn_count: Option<u32>,
    tools: &VcsTools,
    now_ms: u64,
) -> FleetLocalCopy {
    let mut head_commit = None;
    let mut branch_or_bookmark = None;
    let mut dirty = None;
    let mut remotes = Vec::new();

    if let Some(pijul_bin) = tools.pijul.as_deref() {
        if let Ok(out) = run_pijul_readonly(pijul_bin, path, &["channel"], tools) {
            branch_or_bookmark = parse_pijul_channel(&out);
        }
        if let Ok(out) = run_pijul_readonly(pijul_bin, path, &["log", "--limit", "1"], tools) {
            head_commit = parse_pijul_log_hash(&out);
        }
        if let Ok(out) = run_pijul_readonly(pijul_bin, path, &["diff", "--short"], tools) {
            dirty = Some(!out.trim().is_empty());
        }
    }

    if let Ok(bytes) = read_bounded(&path.join(".pijul").join("config"), MAX_GIT_CONFIG_BYTES)
        && let Ok(text) = std::str::from_utf8(&bytes)
    {
        remotes = parse_pijul_config_remotes(text);
    }

    FleetLocalCopy {
        machine: machine.to_owned(),
        path: path_string,
        vcs: Some(LocalVcs::Pijul),
        head_commit,
        branch_or_bookmark,
        dirty,
        ahead: None,
        behind: None,
        remotes,
        cairn_count,
        observed_at_ms: Some(now_ms),
        health: CopyHealth::Available,
        error: None,
    }
}

fn run_pijul_readonly(
    pijul_bin: &Path,
    repo: &Path,
    args: &[&str],
    tools: &VcsTools,
) -> Result<String, String> {
    let mut child = Child::new(
        pijul_bin,
        repo,
        VCS_INSPECT_TIMEOUT_MS,
        MAX_VCS_OUTPUT_BYTES,
    );
    for (name, value) in &tools.environment {
        child = child.env(name, value);
    }
    child = child
        .args(args.iter().copied())
        .arg("--repository")
        .arg(repo);
    child.run()?.require_success()?.stdout_text()
}

fn read_git_config_remotes_at(path: &Path) -> Vec<String> {
    let Ok(bytes) = read_bounded(path, MAX_GIT_CONFIG_BYTES) else {
        return Vec::new();
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Vec::new();
    };
    parse_git_config_remotes(text)
}

fn read_git_head_fallback(repo_path: &Path) -> Option<(Option<String>, Option<String>)> {
    let git_dir = repo_path.join(".git");
    let head_path = git_dir.join("HEAD");
    let bytes = read_bounded(&head_path, MAX_GIT_CONFIG_BYTES).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let (mut commit, branch) = parse_git_head_file(text);
    if commit.is_none()
        && let Some(ref branch_name) = branch
    {
        let ref_path = git_dir.join("refs").join("heads").join(branch_name);
        if let Ok(ref_bytes) = read_bounded(&ref_path, MAX_GIT_CONFIG_BYTES)
            && let Ok(ref_text) = std::str::from_utf8(&ref_bytes)
        {
            let trimmed = ref_text.trim();
            if is_commit_hex(trimmed) {
                commit = Some(trimmed.to_owned());
            }
        }
    }
    Some((commit, branch))
}

/// Pure: extract deduplicated remote URLs from `git remote -v` output.
#[must_use]
pub fn parse_git_remote_v(output: &str) -> Vec<String> {
    let mut remotes = Vec::new();
    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let Some(_name) = parts.next() else {
            continue;
        };
        let Some(url) = parts.next() else {
            continue;
        };
        if url.is_empty() || url.chars().any(char::is_control) {
            continue;
        }
        if !remotes.iter().any(|existing| existing == url) {
            remotes.push(url.to_owned());
            if remotes.len() >= MAX_REMOTES_PER_COPY {
                break;
            }
        }
    }
    remotes
}

/// Pure: extract deduplicated remote URLs from `.git/config` INI text.
#[must_use]
pub fn parse_git_config_remotes(config_text: &str) -> Vec<String> {
    let mut in_remote_section = false;
    let mut remotes = Vec::new();
    for raw_line in config_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let inner = line
                .get(1..line.len().saturating_sub(1))
                .unwrap_or("")
                .trim();
            in_remote_section = inner
                .get(..6)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("remote"))
                && inner
                    .as_bytes()
                    .get(6)
                    .is_some_and(|b| b.is_ascii_whitespace() || *b == b'"');
            continue;
        }
        if !in_remote_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("url") {
            let url = value.trim().trim_matches('"');
            if !url.is_empty()
                && !url.chars().any(char::is_control)
                && !remotes.iter().any(|existing| existing == url)
            {
                remotes.push(url.to_owned());
                if remotes.len() >= MAX_REMOTES_PER_COPY {
                    break;
                }
            }
        }
    }
    remotes
}

/// Pure: extract remotes from `.pijul/config` if configured.
#[must_use]
pub fn parse_pijul_config_remotes(config_text: &str) -> Vec<String> {
    let mut remotes = Vec::new();
    for raw_line in config_text.lines() {
        let line = raw_line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("default_remote") {
            let url = value.trim().trim_matches('"');
            if !url.is_empty()
                && !url.chars().any(char::is_control)
                && !remotes.iter().any(|existing| existing == url)
            {
                remotes.push(url.to_owned());
                if remotes.len() >= MAX_REMOTES_PER_COPY {
                    break;
                }
            }
        }
    }
    remotes
}

/// Pure: determine working tree dirty status from `git status --porcelain=v1`.
#[must_use]
pub fn parse_git_status_dirty(porcelain: &str) -> bool {
    porcelain.lines().any(|line| !line.is_empty())
}

/// Pure: parse `<behind>\t<ahead>` from `git rev-list --left-right --count @{upstream}...HEAD`.
#[must_use]
pub fn parse_git_ahead_behind(counts: &str) -> (Option<u32>, Option<u32>) {
    let mut parts = counts.split_whitespace();
    let (Some(behind), Some(ahead), None) = (parts.next(), parts.next(), parts.next()) else {
        return (None, None);
    };
    match (ahead.parse::<u32>(), behind.parse::<u32>()) {
        (Ok(a), Ok(b)) => (Some(a), Some(b)),
        _ => (None, None),
    }
}

/// Pure: parse `.git/HEAD` content into `(detached_commit, branch_name)`.
#[must_use]
pub fn parse_git_head_file(head_text: &str) -> (Option<String>, Option<String>) {
    let trimmed = head_text.trim();
    if let Some(target) = trimmed.strip_prefix("ref:") {
        let ref_name = target.trim();
        let branch = ref_name
            .strip_prefix("refs/heads/")
            .unwrap_or(ref_name)
            .to_owned();
        if branch.is_empty() {
            (None, None)
        } else {
            (None, Some(branch))
        }
    } else if is_commit_hex(trimmed) {
        (Some(trimmed.to_owned()), None)
    } else {
        (None, None)
    }
}

/// Pure: parse `jj log -r @` tab-delimited template output.
#[must_use]
pub fn parse_jj_log_line(output: &str) -> (Option<String>, Option<String>, Option<bool>) {
    let Some(line) = output.lines().find(|l| !l.trim().is_empty()) else {
        return (None, None, None);
    };
    let mut parts = line.split('\t');
    let commit = parts
        .next()
        .map(str::trim)
        .filter(|s| is_commit_hex(s))
        .map(str::to_owned);
    let dirty = match parts.next().map(str::trim) {
        Some("true") => Some(false),
        Some("false") => Some(true),
        _ => None,
    };
    let direct_bookmarks = parts.next().unwrap_or_default();
    let parent_bookmarks = parts.next().unwrap_or_default();
    let bookmark = first_nonempty_csv(direct_bookmarks)
        .or_else(|| first_nonempty_csv(parent_bookmarks))
        .map(str::to_owned);
    (commit, bookmark, dirty)
}

fn first_nonempty_csv(raw: &str) -> Option<&str> {
    raw.split(',')
        .map(str::trim)
        .find(|token| !token.is_empty())
}

/// Pure: parse the active `* <channel>` line from `pijul channel`.
#[must_use]
pub fn parse_pijul_channel(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("* ")
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    })
}

/// Pure: parse the newest change hash from `pijul log --limit 1`.
#[must_use]
pub fn parse_pijul_log_hash(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let trimmed = line.trim();
        let rest = trimmed
            .strip_prefix("Change:")
            .or_else(|| trimmed.strip_prefix("change:"))?;
        let token = rest.trim();
        (!token.is_empty()).then(|| token.to_owned())
    })
}

/// Pure: lowercase SHA-1 (40) or SHA-256 (64) hex object identifier.
#[must_use]
pub fn is_commit_hex(value: &str) -> bool {
    COMMIT_HEX_LENGTHS.contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sanitize_error(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    truncate_utf8(cleaned.trim(), MAX_ERROR_BYTES)
}

fn truncate_utf8(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_owned();
    }
    let mut end = 0usize;
    for (idx, ch) in input.char_indices() {
        let next = idx + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    input.get(..end).unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_adapter_herdr::HerdrMachine;

    #[test]
    fn parses_git_remotes_status_ahead_behind_and_head() {
        let remote_v = "\
origin\thttps://github.com/OnixResearch/kiln.git (fetch)\n\
origin\thttps://github.com/OnixResearch/kiln.git (push)\n\
rad\trad://z2wsvXm5S2sJGvuV1k5JHiwi1PbKE (fetch)\n";
        assert_eq!(
            parse_git_remote_v(remote_v),
            vec![
                "https://github.com/OnixResearch/kiln.git",
                "rad://z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
            ]
        );

        let git_config = "\
[core]\n\trepositoryformatversion = 0\n\
[remote \"origin\"]\n\turl = git@github.com:OnixResearch/kiln.git\n\
[remote \"mirror\"]\n\turl = https://git.blr.dev/OnixResearch/kiln.git\n";
        assert_eq!(
            parse_git_config_remotes(git_config),
            vec![
                "git@github.com:OnixResearch/kiln.git",
                "https://git.blr.dev/OnixResearch/kiln.git",
            ]
        );

        assert!(!parse_git_status_dirty(""));
        assert!(!parse_git_status_dirty("\n"));
        assert!(parse_git_status_dirty(" M crates/kiln-fleet/src/api.rs\n"));
        assert!(parse_git_status_dirty("?? untracked.txt\n"));

        assert_eq!(parse_git_ahead_behind("2\t5"), (Some(5), Some(2)));
        assert_eq!(parse_git_ahead_behind("invalid"), (None, None));

        assert_eq!(
            parse_git_head_file("ref: refs/heads/main\n"),
            (None, Some("main".to_owned()))
        );
        let sha = "a".repeat(40);
        assert_eq!(parse_git_head_file(&sha), (Some(sha), None));
    }

    #[test]
    fn parses_jj_and_pijul_outputs() {
        let sha = "b".repeat(40);
        let line = format!("{sha}\tfalse\t\tmain,dev");
        assert_eq!(
            parse_jj_log_line(&line),
            (Some(sha.clone()), Some("main".to_owned()), Some(true))
        );
        let clean_line = format!("{sha}\ttrue\tfeature\t");
        assert_eq!(
            parse_jj_log_line(&clean_line),
            (Some(sha), Some("feature".to_owned()), Some(false))
        );

        assert_eq!(
            parse_pijul_channel("  other\n* main\n"),
            Some("main".to_owned())
        );
        assert_eq!(
            parse_pijul_log_hash("Change: ABCD1234\nAuthor: test\n"),
            Some("ABCD1234".to_owned())
        );
    }

    #[test]
    // r[verify kiln.fleet.local_copy_census.scenario.offline_host]
    fn assemble_census_keeps_offline_hosts_explicit_and_enforces_bounds() {
        let remote_machine = HerdrMachine {
            label: "aspen3".to_owned(),
            herdr_machine: Some("aspen3".to_owned()),
            ssh_target: Some("brittonr@aspen3".to_owned()),
            worktree_root: "/home/brittonr/worktrees".to_owned(),
            repositories: BTreeMap::from([(
                "OnixResearch/kiln".to_owned(),
                "/home/brittonr/git/kiln".to_owned(),
            )]),
            agents: Vec::new(),
        };
        let (err, remote_copies, remote_trunc) =
            project_remote_host("aspen3", Some(&remote_machine), None, 1_700_000_000_000);
        assert!(err.contains("aspen3"));
        assert!(!remote_trunc);
        assert_eq!(remote_copies.len(), 1);
        let first_remote = remote_copies.first().unwrap_or_else(|| unreachable!());
        assert_eq!(first_remote.health, CopyHealth::Unavailable);
        assert_eq!(first_remote.dirty, None);

        let local_copy = FleetLocalCopy {
            machine: "britton-desktop".to_owned(),
            path: "/home/brittonr/git/kiln".to_owned(),
            vcs: Some(LocalVcs::Jj),
            head_commit: Some("c".repeat(40)),
            branch_or_bookmark: Some("main".to_owned()),
            dirty: Some(false),
            ahead: None,
            behind: None,
            remotes: vec!["https://github.com/OnixResearch/kiln.git".to_owned()],
            cairn_count: Some(2),
            observed_at_ms: Some(1_700_000_000_000),
            health: CopyHealth::Available,
            error: None,
        };

        let census = assemble_census(
            "britton-desktop",
            BTreeSet::from(["britton-desktop".to_owned(), "aspen3".to_owned()]),
            vec![local_copy],
            None,
            false,
            vec![("aspen3".to_owned(), err, remote_copies, false)],
            1_700_000_000_000,
        );
        assert_eq!(census.schema, REPOSITORIES_SCHEMA);
        assert!(census.bounded);
        assert!(!census.truncated);
        assert_eq!(census.hosts.len(), 2);
        let aspen = census
            .hosts
            .iter()
            .find(|h| h.machine == "aspen3")
            .unwrap_or_else(|| unreachable!());
        assert_eq!(aspen.health, HostHealth::Unavailable);
        assert!(aspen.error.is_some());
        assert_eq!(aspen.copy_count, 1);
    }
}
