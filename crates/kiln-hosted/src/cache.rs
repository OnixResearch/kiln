//! Tenant-scoped expendable CI cache, using the maintained bounded fd-relative
//! cache transfer and immutable content-verified store. Never a source store,
//! credential transport, execution authority, or financial/replay journal.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use kiln_web_service::cache::{
    CacheKey, CacheObservation, CacheProfile, CacheProject, CacheStore, WorkerCacheProfile,
};
use kiln_web_service::worker::cache::{
    collect_local, local_key, require_absent_outputs, restore_local,
};
use serde::{Deserialize, Serialize};

static OWNER: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub organization: String,
    pub project: String,
    pub runner: String,
    pub worker: WorkerCacheProfile,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedCacheConfig {
    pub directory: PathBuf,
    pub maximum_entry_bytes: usize,
    pub maximum_total_bytes: u64,
    pub maximum_entries: usize,
    pub maximum_age_seconds: u64,
    pub maximum_files: usize,
    pub policies: Vec<Policy>,
}

#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
    pub organization: &'a str,
    pub project: &'a str,
    pub runner: &'a str,
    pub plan_identity: &'a str,
    pub job_identity: &'a str,
    pub configuration_identity: &'a str,
    pub workspace: &'a Path,
    pub now_ms: u64,
}

fn elapsed(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
fn tenant(organization: &str, project: &str) -> Result<String, String> {
    crate::identity(&("kiln.hosted.cache.tenant.v1", organization, project))
}
impl HostedCacheConfig {
    pub fn validate(&self, work_root: &Path) -> Result<(), String> {
        let cache = self
            .directory
            .canonicalize()
            .map_err(|_| "private cache root unavailable")?;
        let work = work_root
            .canonicalize()
            .map_err(|_| "customer work root unavailable")?;
        if cache.starts_with(&work) || work.starts_with(&cache) {
            return Err("cache root must be separate from customer source/workspaces".into());
        }
        if self.policies.is_empty()
            || self.policies.len() > 128
            || self.maximum_entry_bytes > 1_048_576
            || self.maximum_total_bytes > 1_073_741_824
            || self.maximum_entries > 4096
            || self.maximum_files > 4096
        {
            return Err("hosted cache bounds exceeded".into());
        }
        let mut scopes = BTreeSet::new();
        for policy in &self.policies {
            if !crate::identifier(&policy.organization)
                || !crate::identifier(&policy.project)
                || !crate::identifier(&policy.runner)
                || !scopes.insert((&policy.organization, &policy.project, &policy.runner))
            {
                return Err(
                    "cache requires unique exact organization/project/runner policies".into(),
                );
            }
            policy.worker.admit()?;
            if policy.worker.maximum_entry_bytes > self.maximum_entry_bytes
                || policy.worker.maximum_files > self.maximum_files
                || policy.worker.inputs.iter().any(|input| {
                    policy
                        .worker
                        .paths
                        .iter()
                        .any(|output| input.starts_with(output) || output.starts_with(input))
                })
            {
                return Err(
                    "cache output overlaps source inputs or exceeds operator bounds".into(),
                );
            }
        }
        self.profile()?.preflight()
    }
    pub fn policy(&self, context: &Context<'_>) -> Option<&Policy> {
        self.policies.iter().find(|p| {
            p.organization == context.organization
                && p.project == context.project
                && p.runner == context.runner
        })
    }
    fn profile(&self) -> Result<CacheProfile, String> {
        let mut projects: BTreeMap<String, CacheProject> = BTreeMap::new();
        for policy in &self.policies {
            let id = tenant(&policy.organization, &policy.project)?;
            let project = projects.entry(id.clone()).or_insert_with(|| CacheProject {
                project_id: id,
                producers: vec![],
                toolchains: vec![],
                platforms: vec![],
                paths: vec![],
            });
            project.producers.push(policy.worker.producer.clone());
            project.toolchains.push(policy.worker.toolchain.clone());
            project.platforms.push(policy.worker.platform.clone());
            project.paths.extend(policy.worker.paths.iter().cloned());
        }
        for project in projects.values_mut() {
            project.producers.sort();
            project.producers.dedup();
            project.toolchains.sort();
            project.toolchains.dedup();
            project.platforms.sort();
            project.platforms.dedup();
            project.paths.sort();
            project.paths.dedup();
            let paths = std::mem::take(&mut project.paths);
            project.paths = paths
                .iter()
                .filter(|path| {
                    !paths
                        .iter()
                        .any(|other| other != *path && path.starts_with(other))
                })
                .cloned()
                .collect();
        }
        Ok(CacheProfile {
            schema: "kiln.web-cache.v1".into(),
            directory: self.directory.clone(),
            maximum_entry_bytes: self.maximum_entry_bytes,
            maximum_total_bytes: self.maximum_total_bytes,
            maximum_entries: self.maximum_entries,
            maximum_age_seconds: self.maximum_age_seconds,
            maximum_files: self.maximum_files,
            projects: projects.into_values().collect(),
        })
    }
}
fn key<'a>(
    config: &'a HostedCacheConfig,
    context: &Context<'_>,
) -> Result<(CacheKey, &'a WorkerCacheProfile), String> {
    let policy = config
        .policy(context)
        .ok_or("job has no exact operator cache policy")?;
    let namespace = tenant(context.organization, context.project)?;
    let node = crate::identity(&(
        "kiln.hosted.cache.node.v1",
        context.configuration_identity,
        context.plan_identity,
        context.job_identity,
        &policy.worker,
    ))?;
    let key = local_key(&namespace, &node, &policy.worker, context.workspace)?;
    Ok((key, &policy.worker))
}
fn store<T>(
    config: &HostedCacheConfig,
    operation: impl FnOnce(&mut CacheStore) -> Result<T, String>,
) -> Result<T, String> {
    let _owner = OWNER.lock().map_err(|_| "cache owner lock unavailable")?;
    let mut store = CacheStore::open(config.profile()?)?;
    operation(&mut store)
}

/// Source snapshots must not already contain any declared output root. This
/// prevents restoration/publication over repository source or credentials and
/// requires a fresh acquired workspace for each admitted attempt.
pub fn restore(
    config: &HostedCacheConfig,
    context: &Context<'_>,
) -> Result<(CacheKey, CacheObservation), String> {
    let started = Instant::now();
    let (key, policy) = key(config, context)?;
    require_absent_outputs(context.workspace, policy)?;
    let envelope = store(config, |store| store.restore(&key, context.now_ms / 1000))?;
    let (outcome, bytes) = if let Some(envelope) = envelope {
        if envelope.entry.key != key {
            return Err("tenant/source/config cache key mismatch".into());
        }
        let bytes = envelope.verify(policy.maximum_entry_bytes, policy.maximum_files)?;
        restore_local(context.workspace, &envelope, policy)?;
        (
            "hit",
            u64::try_from(bytes).map_err(|_| "cache size conversion failed")?,
        )
    } else {
        ("miss", 0)
    };
    let observation = CacheObservation {
        key: key.identity()?,
        outcome: outcome.into(),
        transferred_bytes: bytes,
        duration_ms: elapsed(started),
    };
    Ok((key, observation))
}

/// Call only after externally observed job success and confirmed owned cleanup.
/// Inputs are rehashed before publication: a job that changes its source cannot
/// publish derived outputs under a stale source identity.
pub fn publish(
    config: &HostedCacheConfig,
    context: &Context<'_>,
    original: &CacheKey,
) -> Result<CacheObservation, String> {
    let started = Instant::now();
    let (current, policy) = key(config, context)?;
    if current != *original {
        return Err("cache source changed during job".into());
    }
    let envelope = collect_local(context.workspace, original, policy)?;
    let bytes = envelope.verify(policy.maximum_entry_bytes, policy.maximum_files)?;
    let published = store(config, |store| {
        store.publish(&envelope, context.now_ms / 1000)
    })?;
    Ok(CacheObservation {
        key: original.identity()?,
        outcome: if published { "published" } else { "retained" }.into(),
        transferred_bytes: u64::try_from(bytes).map_err(|_| "cache size conversion failed")?,
        duration_ms: elapsed(started),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    fn private_root() -> tempfile::TempDir {
        let directory = ok(tempfile::tempdir());
        ok(std::fs::set_permissions(
            directory.path(),
            std::fs::Permissions::from_mode(0o700),
        ));
        directory
    }
    fn ok<T, E: std::fmt::Debug>(value: Result<T, E>) -> T {
        value.unwrap_or_else(|error| unreachable!("cache fixture succeeds: {error:?}"))
    }
    fn settings(root: &Path) -> HostedCacheConfig {
        let worker = WorkerCacheProfile {
            producer: "hosted".into(),
            toolchain: "compiler-v1".into(),
            platform: "linux".into(),
            inputs: vec!["input".into()],
            paths: vec!["build-cache".into()],
            maximum_entry_bytes: 4096,
            maximum_files: 16,
        };
        HostedCacheConfig {
            directory: root.into(),
            maximum_entry_bytes: 4096,
            maximum_total_bytes: 65536,
            maximum_entries: 16,
            maximum_age_seconds: 3600,
            maximum_files: 16,
            policies: vec![
                Policy {
                    organization: "org".into(),
                    project: "project".into(),
                    runner: "runner".into(),
                    worker: worker.clone(),
                },
                Policy {
                    organization: "other".into(),
                    project: "project".into(),
                    runner: "runner".into(),
                    worker,
                },
            ],
        }
    }
    fn context<'a>(root: &'a Path, organization: &'a str, configuration: &'a str) -> Context<'a> {
        Context {
            organization,
            project: "project",
            runner: "runner",
            plan_identity: "plan",
            job_identity: "job",
            configuration_identity: configuration,
            workspace: root,
            now_ms: 1_000,
        }
    }
    #[test]
    fn successful_output_is_reused_but_other_tenants_inputs_and_configs_do_not_hit() {
        let root = private_root();
        let work = private_root();
        let cache = settings(root.path());
        ok(cache.validate(work.path()));
        let first = work.path().join("first");
        let second = work.path().join("second");
        ok(std::fs::DirBuilder::new().mode(0o700).create(&first));
        ok(std::fs::DirBuilder::new().mode(0o700).create(&second));
        ok(std::fs::write(first.join("input"), b"source-v1"));
        ok(std::fs::write(second.join("input"), b"source-v1"));
        let (key, miss) = ok(restore(&cache, &context(&first, "org", "config-v1")));
        assert_eq!(miss.outcome, "miss");
        ok(std::fs::create_dir(first.join("build-cache")));
        ok(std::fs::write(
            first.join("build-cache/output"),
            b"compiled-product",
        ));
        assert_eq!(
            ok(publish(&cache, &context(&first, "org", "config-v1"), &key)).outcome,
            "published"
        );
        assert_eq!(
            ok(restore(&cache, &context(&second, "other", "config-v1")))
                .1
                .outcome,
            "miss"
        );
        assert!(restore(&cache, &context(&second, "unadmitted", "config-v1")).is_err());
        assert_eq!(
            ok(restore(&cache, &context(&second, "org", "config-v2")))
                .1
                .outcome,
            "miss"
        );
        ok(std::fs::write(second.join("input"), b"source-v2"));
        assert_eq!(
            ok(restore(&cache, &context(&second, "org", "config-v1")))
                .1
                .outcome,
            "miss"
        );
        ok(std::fs::write(second.join("input"), b"source-v1"));
        let (_, hit) = ok(restore(&cache, &context(&second, "org", "config-v1")));
        assert_eq!(hit.outcome, "hit");
        assert_eq!(
            ok(std::fs::read(second.join("build-cache/output"))),
            b"compiled-product"
        );
        assert_eq!(ok(std::fs::read(second.join("input"))), b"source-v1");
    }
    #[test]
    fn corruption_symlinks_and_source_overlap_are_rejected() {
        let root = private_root();
        let work = private_root();
        let cache = settings(root.path());
        ok(cache.validate(work.path()));
        ok(std::fs::write(work.path().join("input"), b"source"));
        let (key, _) = ok(restore(&cache, &context(work.path(), "org", "config")));
        ok(std::fs::create_dir(work.path().join("build-cache")));
        ok(std::os::unix::fs::symlink(
            "../input",
            work.path().join("build-cache/link"),
        ));
        assert!(publish(&cache, &context(work.path(), "org", "config"), &key).is_err());
        ok(std::fs::remove_file(work.path().join("build-cache/link")));
        ok(std::fs::write(
            work.path().join("build-cache/output"),
            b"product",
        ));
        ok(publish(
            &cache,
            &context(work.path(), "org", "config"),
            &key,
        ));
        ok(std::fs::remove_dir_all(work.path().join("build-cache")));
        ok(std::fs::write(
            root.path().join("kiln-cache-v1").join(ok(key.identity())),
            b"corrupt",
        ));
        assert!(restore(&cache, &context(work.path(), "org", "config")).is_err());
        assert!(!work.path().join("build-cache").exists());
        let mut overlap = cache;
        let policy = overlap
            .policies
            .first_mut()
            .unwrap_or_else(|| unreachable!("fixture policy exists"));
        policy.worker.paths = vec!["input".into()];
        assert!(overlap.validate(work.path()).is_err());
    }
}
