use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use git2::{Oid, Repository, Tree};
use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter, resolve_and_acquire};
use kiln_core::source::SourceResolution;
use kiln_core::types::ProviderProfileId;

use super::files;
use super::profile::{WorkerProfile, hex};

const MAX_TREE_DEPTH: usize = 128;

#[derive(Clone, Debug)]
pub struct SourceContext {
    pub provider: String,
    pub repository: String,
    pub revision: String,
}

impl SourceContext {
    pub fn from_environment() -> Result<Self, String> {
        let read =
            |key| std::env::var(key).map_err(|_| "worker source context missing".to_string());
        Ok(Self {
            provider: read("LATTICE_SOURCE_PROVIDER")?,
            repository: read("LATTICE_SOURCE_REPOSITORY")?,
            revision: read("LATTICE_SOURCE_REVISION")?,
        })
    }
}

pub struct PreparedSource {
    // Owning the temporary directory removes the isolated checkout after the job.
    _root: tempfile::TempDir,
    pub checkout: PathBuf,
    pub resolution: SourceResolution,
}

pub const PROJECT_ROOT_MISSING: &str = "project_root_missing";
pub const PROJECT_ROOT_ESCAPES_CHECKOUT: &str = "project_root_escapes_checkout";

// r[impl kiln.monorepo.dispatch]
/// Working directory for a scoped project's jobs: `checkout/root`, resolved
/// through symlinks and required to stay inside the exact checkout. An empty
/// or absent root is the checkout itself.
pub fn project_directory(checkout: &Path, root: Option<&str>) -> Result<PathBuf, String> {
    let Some(root) = root.filter(|root| !root.is_empty()) else {
        return Ok(checkout.to_path_buf());
    };
    if !kiln_core::paths::canonical(root) {
        return Err(PROJECT_ROOT_ESCAPES_CHECKOUT.into());
    }
    let checkout = fs::canonicalize(checkout).map_err(|_| "worker checkout unavailable")?;
    let directory = match fs::canonicalize(checkout.join(root)) {
        Ok(directory) => directory,
        Err(_) => return Err(PROJECT_ROOT_MISSING.into()),
    };
    if !directory.starts_with(&checkout) {
        return Err(PROJECT_ROOT_ESCAPES_CHECKOUT.into());
    }
    if !directory.is_dir() {
        return Err(PROJECT_ROOT_MISSING.into());
    }
    Ok(directory)
}

// r[impl kiln.web.remote_workers.source]
// r[impl kiln.web.remote_workers.source.scenario.clone]
pub fn prepare(profile: &WorkerProfile, context: &SourceContext) -> Result<PreparedSource, String> {
    profile.admit()?;
    if context.provider != "git"
        || context.repository != profile.source_repository
        || !hex(&context.revision, 20)
        || context.revision.bytes().all(|b| b == b'0')
    {
        return Err("worker source context invalid".into());
    }
    // Reject aliased backing paths without equating their local identity to the controller.
    files::directory(&profile.repository_path)?;
    files::owned_directory(&profile.work_root)?;
    let local =
        Repository::open(&profile.repository_path).map_err(|_| "worker repository unavailable")?;
    let oid = Oid::from_str(&context.revision).map_err(|_| "worker commit invalid")?;
    let synthetic = if local.find_commit(oid).is_ok() {
        None
    } else {
        let service = super::client::ServiceClient::new(profile)?;
        let response = service
            .client
            .get(format!(
                "{}/api/snapshots/{}/{}",
                service.origin, profile.project_id, context.revision
            ))
            .header(
                reqwest::header::AUTHORIZATION,
                service.authorization.clone(),
            )
            .send()
            .map_err(|_| "worker snapshot download failed")?;
        let record: crate::snapshot::SnapshotRecord =
            serde_json::from_value(service.response(response)?)
                .map_err(|_| "worker snapshot response invalid")?;
        if record.manifest.project_id != profile.project_id
            || record.manifest.identity()? != record.provenance.manifest_id
            || record.manifest.base_revision != record.provenance.base_revision
            || record.provenance.synthetic_revision != context.revision
        {
            return Err("worker snapshot provenance mismatch".into());
        }
        let synthetic = crate::snapshot::reconstruct(
            &profile.repository_path,
            &record.manifest,
            &profile.snapshot_limits,
        )?;
        if synthetic.revision != context.revision {
            return Err("worker snapshot revision mismatch".into());
        }
        Some(synthetic)
    };
    let backing = synthetic
        .as_ref()
        .map(|s| s.repository_path.clone())
        .unwrap_or_else(|| profile.repository_path.clone());
    let provider =
        ProviderProfileId::new("web-worker-git-v1").map_err(|_| "worker source profile invalid")?;
    let adapter = LocalGitAdapter::new(
        backing.clone(),
        context.revision.clone(),
        GitSourceProfile::pinned_local(provider.clone(), profile.snapshot_limits.clone()),
    )
    .map_err(|_| "worker source admission failed")?;
    let resolution =
        resolve_and_acquire(&adapter, provider).map_err(|_| "worker snapshot admission failed")?;
    if resolution.exact_revision.native_id.as_str() != context.revision {
        return Err("worker source binding mismatch".into());
    }
    let repository = Repository::open(&backing).map_err(|_| "worker repository unavailable")?;
    let commit = repository
        .find_commit(Oid::from_str(&context.revision).map_err(|_| "worker commit invalid")?)
        .map_err(|_| "worker commit unavailable")?;
    let tree = commit
        .tree()
        .map_err(|_| "worker source tree unavailable")?;
    validate_tree(
        &repository,
        &tree,
        0,
        0,
        profile.snapshot_limits.max_path_bytes,
    )?;
    let root = tempfile::Builder::new()
        .prefix("kiln-web-job-")
        .tempdir_in(&profile.work_root)
        .map_err(|_| "worker isolation directory failed")?;
    let checkout = root.path().join("checkout");
    files::create_private(&checkout)?;
    materialize(&repository, &tree, &checkout)?;
    Ok(PreparedSource {
        _root: root,
        checkout,
        resolution,
    })
}

fn validate_tree(
    repository: &Repository,
    tree: &Tree<'_>,
    prefix: usize,
    depth: usize,
    maximum_path: usize,
) -> Result<(), String> {
    if depth > MAX_TREE_DEPTH {
        return Err("worker source tree depth exceeded".into());
    }
    for entry in tree {
        let name = entry.name().ok_or("worker source path encoding denied")?;
        let bytes = prefix
            .checked_add(name.len())
            .ok_or("worker source path overflow")?;
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.eq_ignore_ascii_case(".git")
            || name.contains(['/', '\\', '\0'])
            || bytes > maximum_path
        {
            return Err("worker source path denied".into());
        }
        match entry.filemode() {
            0o040_000 => validate_tree(
                repository,
                &repository
                    .find_tree(entry.id())
                    .map_err(|_| "worker source tree unavailable")?,
                bytes + 1,
                depth + 1,
                maximum_path,
            )?,
            0o100_644 | 0o100_755 => {}
            _ => return Err("worker source links and special files denied".into()),
        }
    }
    Ok(())
}

fn materialize(repository: &Repository, tree: &Tree<'_>, destination: &Path) -> Result<(), String> {
    for entry in tree {
        let path = destination.join(entry.name().ok_or("worker source path encoding denied")?);
        if entry.filemode() == 0o040_000 {
            files::create_private(&path)?;
            materialize(
                repository,
                &repository
                    .find_tree(entry.id())
                    .map_err(|_| "worker source tree unavailable")?,
                &path,
            )?;
        } else {
            let blob = repository
                .find_blob(entry.id())
                .map_err(|_| "worker source blob unavailable")?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|_| "worker source materialization failed")?;
            file.write_all(blob.content())
                .map_err(|_| "worker source materialization failed")?;
            file.set_permissions(fs::Permissions::from_mode(
                if entry.filemode() == 0o100_755 {
                    0o700
                } else {
                    0o600
                },
            ))
            .map_err(|_| "worker source permissions failed")?;
        }
    }
    Ok(())
}
