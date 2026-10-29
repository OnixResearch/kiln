use super::model::DurableState;
use crate::profile::{ServiceBounds, ServiceProfile};
use fs2::FileExt;
use rusqlite::{Connection, params};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::Path;

pub struct Store {
    connection: Connection,
}

/// Release ownership when the shell closes, not when the last inherited file
/// description closes. CLOEXEC alone leaves a fork-to-exec retention window.
pub(super) struct StateLock(File);

impl StateLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        let file = owned_file(path)?;
        file.try_lock_exclusive()
            .map_err(|_| "service state already has an owner")?;
        Ok(Self(file))
    }
}

impl Drop for StateLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

pub fn private_directory(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if !meta.is_dir()
                || meta.uid() != nix::unistd::geteuid().as_raw()
                || meta.mode() & 0o777 != 0o700
            {
                return Err("state root must be an owned private directory".into());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(path)
                .map_err(|_| "private state directory cannot be created")?;
        }
        Err(_) => return Err("private state metadata unavailable".into()),
    }
    // Sticky shared ancestors such as /tmp cannot rename another owner's entries.
    for ancestor in path.ancestors() {
        let metadata =
            std::fs::symlink_metadata(ancestor).map_err(|_| "state ancestor unavailable")?;
        if metadata.file_type().is_symlink()
            || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
        {
            return Err(
                "state ancestors may not permit untrusted replacement or symlink traversal".into(),
            );
        }
    }
    Ok(())
}
pub fn owned_file(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "private state file cannot be opened")?;
    let meta = file
        .metadata()
        .map_err(|_| "state file metadata unavailable")?;
    if !meta.is_file()
        || meta.uid() != nix::unistd::geteuid().as_raw()
        || meta.mode() & 0o777 != 0o600
        || meta.nlink() != 1
    {
        return Err("state file must be owned private regular file".into());
    }
    Ok(file)
}
/// Publish one immutable owned blob without replacing an existing identity.
pub(super) fn publish_retained(directory: &Path, id: &str, bytes: &[u8]) -> Result<(), String> {
    if !hex_identity(id) {
        return Err("retained identity invalid".into());
    }
    private_directory(directory)?;
    let temporary = directory.join(format!("{id}.tmp"));
    let path = directory.join(id);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temporary)
        .map_err(|_| "retained publication reserved or unavailable")?;
    let result = (|| -> Result<(), std::io::Error> {
        file.write_all(bytes)?;
        file.sync_all()?;
        // Linking is atomic and, unlike rename, cannot replace an admitted blob.
        std::fs::hard_link(&temporary, &path)?;
        std::fs::remove_file(&temporary)?;
        File::open(directory)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err("retained publication failed; restart requires recovery".into());
    }
    Ok(())
}
pub(super) fn read_retained(
    directory: &Path,
    id: &str,
    length: usize,
    digest: &str,
) -> Result<Vec<u8>, String> {
    if !hex_identity(id) || !hex_identity(digest) {
        return Err("retained metadata invalid".into());
    }
    private_directory(directory)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(directory.join(id))
        .map_err(|_| "retained bytes unavailable")?;
    let meta = file
        .metadata()
        .map_err(|_| "retained metadata unavailable")?;
    if !meta.is_file()
        || meta.uid() != nix::unistd::geteuid().as_raw()
        || meta.nlink() != 1
        || meta.mode() & 0o777 != 0o600
        || meta.len() != length as u64
    {
        return Err("retained ownership or size conflict".into());
    }
    let limit = length.checked_add(1).ok_or("retained size overflow")?;
    let mut bytes = Vec::with_capacity(length);
    file.take(limit as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "retained read failed")?;
    if bytes.len() != length || blake3::hash(&bytes).to_hex().as_str() != digest {
        return Err("retained content conflict".into());
    }
    Ok(bytes)
}
impl Store {
    pub fn open(profile: &ServiceProfile) -> Result<(Self, DurableState, StateLock), String> {
        Self::open_bound(profile, true)
    }
    pub(super) fn open_existing(
        profile: &ServiceProfile,
    ) -> Result<(Self, DurableState, StateLock), String> {
        Self::open_bound(profile, false)
    }
    // r[impl kiln.web.lifecycle]
    fn open_bound(
        profile: &ServiceProfile,
        initialize: bool,
    ) -> Result<(Self, DurableState, StateLock), String> {
        if !initialize
            && std::fs::symlink_metadata(profile.state_directory.join("service.sqlite")).is_err()
        {
            return Err("project extension requires an existing service database".into());
        }
        private_directory(&profile.state_directory)?;
        let lock = StateLock::acquire(&profile.state_directory.join("service.lock"))?;
        private_directory(&profile.state_directory.join("artifacts"))?;
        private_directory(&profile.state_directory.join("logs"))?;
        let path = profile.state_directory.join("service.sqlite");
        let _file = owned_file(&path)?;
        for suffix in ["service.sqlite-wal", "service.sqlite-shm"] {
            let sidecar = profile.state_directory.join(suffix);
            if std::fs::symlink_metadata(&sidecar).is_ok() {
                let _ = owned_file(&sidecar)?;
            }
        }
        let mut connection =
            Connection::open(&path).map_err(|_| "state database cannot be opened")?;
        connection
            .busy_timeout(std::time::Duration::from_millis(500))
            .map_err(|_| "database timeout admission failed")?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|_| "database WAL admission failed")?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|_| "database FULL durability admission failed")?;
        let journal: String = connection
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .map_err(|_| "database journal mode unavailable")?;
        let synchronous: i64 = connection
            .pragma_query_value(None, "synchronous", |r| r.get(0))
            .map_err(|_| "database synchronization mode unavailable")?;
        if journal != "wal" || synchronous != 2 {
            return Err("database did not admit WAL/FULL durability".into());
        }
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|_| "database version unavailable")?;
        let profile_id = super::identity(profile)?;
        if version == 0 {
            if !initialize {
                return Err("project extension requires versioned service state".into());
            }
            let count:i64=connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0)).map_err(|_| "database schema unavailable")?;
            if count != 0 {
                return Err("unversioned database cannot be adopted".into());
            }
            let tx = connection
                .transaction()
                .map_err(|_| "database transaction unavailable")?;
            tx.execute_batch("CREATE TABLE service_state (id INTEGER PRIMARY KEY CHECK(id=1),profile TEXT NOT NULL,state BLOB NOT NULL); PRAGMA user_version=1;").map_err(|_| "database schema initialization failed")?;
            tx.execute(
                "INSERT INTO service_state VALUES(1,?1,?2)",
                params![
                    profile_id.as_str(),
                    serde_json::to_vec(&DurableState::default())
                        .map_err(|_| "state encoding failed")?
                ],
            )
            .map_err(|_| "database profile binding failed")?;
            tx.commit()
                .map_err(|_| "database initialization commit failed")?;
        } else if version != 1 {
            return Err("database schema version unsupported".into());
        }
        let (binding, bytes): (String, Vec<u8>) = connection
            .query_row(
                "SELECT profile,state FROM service_state WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| "durable state unavailable")?;
        if binding != profile_id.as_str() || bytes.len() > profile.bounds.max_state_bytes {
            return Err("durable profile binding or size conflict".into());
        }
        let mut state: DurableState =
            serde_json::from_slice(&bytes).map_err(|_| "durable state is malformed")?;
        validate(&state, &profile.bounds)?;
        if state
            .enrolled
            .iter()
            .any(|id| !profile.projects.iter().any(|p| &p.id == id))
        {
            return Err("stored enrollment is not allowlisted".into());
        }
        for (id, run) in &mut state.runs {
            run.restore()?;
            let project = profile
                .projects
                .iter()
                .find(|p| p.id == run.project_id)
                .ok_or("stored project missing")?;
            let source = kiln_core::workflow::WorkflowSourceContext {
                provider: kiln_core::types::Alias::new("git").map_err(|_| "git alias invalid")?,
                repository: run.source.repository.clone(),
                revision: kiln_core::types::ExternalRef::new(
                    "git",
                    "revision",
                    run.source.exact_revision.native_id.as_str(),
                )
                .map_err(|_| "stored native revision invalid")?,
            };
            if id != &run.id
                || run.id != format!("web-run-{}", run.plan.idempotency_key.as_str())
                || run.pipeline_identity != super::identity(&project.workflow)?
                || run.plan
                    != super::model::selected_plan(
                        super::model::workflow_plan(
                            project,
                            &run.plan.idempotency_key,
                            &run.pipeline_identity,
                            Some(source),
                        )?,
                        &run.selected_nodes,
                    )?
                || (run.phase == "terminal") != run.outcome().is_terminal()
            {
                return Err("stored execution linkage conflicts with reviewed profile".into());
            }
        }
        for run in state.runs.values() {
            if !matches!(
                run.phase.as_str(),
                "queued"
                    | "dispatch_reserved"
                    | "control_reserved"
                    | "active"
                    | "unknown"
                    | "terminal"
            ) || run.phase == "queued"
                && (run.observation.is_some()
                    || run.outcome() != kiln_core::domain::Outcome::Pending
                    || !run.artifacts.is_empty()
                    || !run.logs.is_empty()
                    || !run.reports.is_empty())
            {
                return Err("stored queue phase conflicts with execution".into());
            }
            if let Some(parent_id) = &run.parent_run_id {
                let parent = state
                    .runs
                    .get(parent_id)
                    .ok_or("stored retry parent missing")?;
                if parent.ordinal >= run.ordinal
                    || parent.project_id != run.project_id
                    || !parent.outcome().is_terminal()
                {
                    return Err("stored retry parent conflict".into());
                }
                if !run.selected_nodes.is_empty() {
                    let observation = parent
                        .observation
                        .as_ref()
                        .ok_or("stored failed retry observation missing")?;
                    let mut selected: std::collections::BTreeSet<String> = observation
                        .nodes
                        .iter()
                        .filter(|n| n.state == kiln_core::workflow::WorkflowNodeState::Failed)
                        .map(|n| n.node_id.as_str().into())
                        .collect();
                    if selected.is_empty()
                        || observation
                            .nodes
                            .iter()
                            .any(|n| n.state == kiln_core::workflow::WorkflowNodeState::Unknown)
                        || run.source != parent.source
                    {
                        return Err("stored failed retry source or outcome conflict".into());
                    }
                    let project = profile
                        .projects
                        .iter()
                        .find(|p| p.id == run.project_id)
                        .ok_or("stored retry project missing")?;
                    loop {
                        let before = selected.len();
                        for edge in &project.workflow.edges {
                            if selected.contains(&edge.source) {
                                selected.insert(edge.target.clone());
                            }
                        }
                        if selected.len() == before {
                            break;
                        }
                    }
                    if selected.into_iter().collect::<Vec<_>>() != run.selected_nodes {
                        return Err("stored retry selection conflict".into());
                    }
                }
            } else if !run.selected_nodes.is_empty() {
                return Err("stored retry selection has no parent".into());
            }
        }
        reconcile_retained_storage(profile, &state, "artifacts")?;
        reconcile_retained_storage(profile, &state, "logs")?;
        for run in state.runs.values() {
            for artifact in &run.artifacts {
                read_retained(
                    &profile.state_directory.join("artifacts"),
                    &artifact.id,
                    artifact.bytes,
                    &artifact.blake3,
                )?;
            }
            for chunk in &run.logs {
                read_retained(
                    &profile.state_directory.join("logs"),
                    &chunk.id,
                    chunk.bytes,
                    &chunk.blake3,
                )?;
            }
            super::reports::verify_retained(run, &profile.state_directory.join("artifacts"))?;
        }
        Ok((Self { connection }, state, lock))
    }
    pub(super) fn extend_binding(&mut self, previous: &str, next: &str) -> Result<(), String> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| "project extension transaction unavailable")?;
        let changed = tx
            .execute(
                "UPDATE service_state SET profile=?1 WHERE id=1 AND profile=?2",
                params![next, previous],
            )
            .map_err(|_| "project extension binding write failed")?;
        if changed != 1 {
            return Err("project extension previous binding changed".into());
        }
        tx.commit()
            .map_err(|_| "project extension commit failed".to_owned())
    }
    pub fn save(&mut self, state: &DurableState, bounds: &ServiceBounds) -> Result<(), String> {
        validate(state, bounds)?;
        let bytes = serde_json::to_vec(state).map_err(|_| "durable state encoding failed")?;
        if bytes.len() > bounds.max_state_bytes {
            return Err("durable state retention exhausted".into());
        }
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| "state transaction unavailable")?;
        tx.execute(
            "UPDATE service_state SET state=?1 WHERE id=1",
            params![bytes],
        )
        .map_err(|_| "durable state write failed")?;
        tx.commit()
            .map_err(|_| "durable state commit failed".to_owned())
    }
}
fn validate(state: &DurableState, b: &ServiceBounds) -> Result<(), String> {
    super::snapshots::validate(state, b.max_runs)?;
    if state.runs.len() > b.max_runs
        || state.enrolled.len() > b.max_projects
        || state.requests.len() > b.max_requests
        || state.audit.len() > b.max_requests.saturating_mul(4).max(64)
        || state
            .runs
            .values()
            .any(|r| r.events.len() > b.max_events_per_run || r.artifacts.len() > b.max_artifacts)
    {
        return Err("durable retention bound exceeded".into());
    }
    let mut total = 0usize;
    if state
        .access_policy_identity
        .as_deref()
        .is_some_and(|id| !hex_identity(id))
    {
        return Err("stored access policy identity rejected".into());
    }
    for run in state.runs.values() {
        super::logs::validate(run)?;
        super::reports::validate(run)?;
        if run.event_recorded_at_ms.len() > run.events.len()
            || run
                .node_timing
                .keys()
                .any(|id| !run.plan.nodes.iter().any(|n| n.id.as_str() == id))
        {
            return Err("stored observed timing linkage rejected".into());
        }
        let mut run_bytes = 0usize;
        let mut names = std::collections::BTreeSet::new();
        for artifact in &run.artifacts {
            if !hex_identity(&artifact.id)
                || !hex_identity(&artifact.blake3)
                || !crate::profile::identifier(&artifact.name)
                || matches!(artifact.name.as_str(), "." | "..")
                || artifact.attempt_id != run.plan.attempt_id
                || !run
                    .plan
                    .nodes
                    .iter()
                    .any(|n| n.id.as_str() == artifact.node_id)
                || !names.insert((&artifact.node_id, &artifact.name))
                || artifact.bytes > b.max_artifact_bytes
                || artifact.download_path
                    != format!("/api/runs/{}/artifacts/{}", run.id, artifact.id)
            {
                return Err("durable artifact identity rejected".into());
            }
            let observation = run
                .observation
                .as_ref()
                .ok_or("stored artifact has no execution linkage")?;
            let expected = super::identity(&(
                "kiln.web-artifact.v1",
                &run.id,
                &artifact.attempt_id,
                &observation.execution,
                &artifact.node_id,
                &artifact.name,
                &artifact.blake3,
            ))?;
            if expected.as_str() != artifact.id {
                return Err(
                    "stored artifact content identity conflicts with execution linkage".into(),
                );
            }
            run_bytes = run_bytes
                .checked_add(artifact.bytes)
                .ok_or("artifact counter overflow")?;
        }
        if run_bytes > b.max_run_artifact_bytes {
            return Err("run artifact retention exceeded".into());
        }
        total = total
            .checked_add(run_bytes)
            .ok_or("artifact total counter overflow")?;
    }
    if total > b.max_total_artifact_bytes {
        return Err("artifact total retention exceeded".into());
    }
    Ok(())
}
pub(super) fn hex_identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn reconcile_retained_storage(
    profile: &ServiceProfile,
    state: &DurableState,
    kind: &str,
) -> Result<(), String> {
    let mut retained = std::collections::BTreeSet::new();
    for run in state.runs.values() {
        if kind == "logs" {
            retained.extend(run.logs.iter().map(|chunk| chunk.id.as_str()));
        } else {
            retained.extend(run.artifacts.iter().map(|artifact| artifact.id.as_str()));
        }
    }
    let directory = profile.state_directory.join(kind);
    // A crash between link and unlink can leave the temporary and final names.
    // Accept only that exact owned pair, then restore the single-link invariant.
    for entry in std::fs::read_dir(&directory).map_err(|_| "retained directory unavailable")? {
        let entry = entry.map_err(|_| "retained entry unavailable")?;
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".tmp")) else {
            continue;
        };
        if !hex_identity(id) {
            return Err("unexpected retained temporary name".into());
        }
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|_| "retained temporary unavailable")?;
        if metadata.nlink() == 2 {
            let final_meta = std::fs::symlink_metadata(directory.join(id))
                .map_err(|_| "retained publication pair missing")?;
            if !metadata.is_file()
                || metadata.uid() != nix::unistd::geteuid().as_raw()
                || metadata.mode() & 0o777 != 0o600
                || !final_meta.is_file()
                || final_meta.dev() != metadata.dev()
                || final_meta.ino() != metadata.ino()
                || final_meta.nlink() != 2
            {
                return Err("retained publication pair conflict".into());
            }
            std::fs::remove_file(entry.path()).map_err(|_| "retained temporary cleanup failed")?;
            File::open(&directory)
                .and_then(|file| file.sync_all())
                .map_err(|_| "retained pair synchronization failed")?;
        }
    }
    let mut removed = false;
    for entry in std::fs::read_dir(&directory).map_err(|_| "artifact directory unavailable")? {
        let entry = entry.map_err(|_| "artifact directory entry unavailable")?;
        let name = entry.file_name();
        let name = name.to_str().ok_or("artifact filename is not UTF-8")?;
        if !hex_identity(name) && !name.strip_suffix(".tmp").is_some_and(hex_identity) {
            return Err("unexpected artifact storage entry".into());
        }
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|_| "artifact recovery metadata unavailable")?;
        if !metadata.is_file()
            || metadata.uid() != nix::unistd::geteuid().as_raw()
            || metadata.mode() & 0o777 != 0o600
            || metadata.nlink() != 1
        {
            return Err("artifact recovery ownership rejected".into());
        }
        if !retained.contains(name) {
            std::fs::remove_file(entry.path())
                .map_err(|_| "unadmitted artifact recovery cleanup failed")?;
            removed = true;
        }
    }
    if removed {
        File::open(directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| "artifact recovery synchronization failed")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_ownership_releases_before_inherited_description_closes() {
        let checked = |result: Result<File, String>| {
            result.unwrap_or_else(|error| unreachable!("fixture failed: {error}"))
        };
        let root =
            tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error}"));
        let path = root.path().join("service.lock");
        let owner = StateLock::acquire(&path)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error}"));
        // dup and fork retain the same open file description. Keep that
        // description alive without racing an actual child process.
        let inherited = owner
            .0
            .try_clone()
            .unwrap_or_else(|error| unreachable!("fixture failed: {error}"));
        let contender = checked(owned_file(&path));
        assert_eq!(
            contender
                .try_lock_exclusive()
                .err()
                .map(|error| error.kind()),
            Some(std::io::ErrorKind::WouldBlock)
        );
        drop(owner);
        contender
            .try_lock_exclusive()
            .unwrap_or_else(|error| unreachable!("ownership was not released: {error}"));
        drop(inherited);
        let third = checked(owned_file(&path));
        assert_eq!(
            third.try_lock_exclusive().err().map(|error| error.kind()),
            Some(std::io::ErrorKind::WouldBlock)
        );
    }
}
