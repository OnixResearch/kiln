//! Hosted execution: explicit reviewed backends, durable pre-effect admission,
//! external resource enforcement and host-side observations. Namespaces are for
//! trusted jobs only. A supplied VM image does not itself qualify hostile use.
//! Firecracker runners put a separate guest kernel behind KVM per job; that is
//! an isolation upgrade over namespaces, not a hostile-isolation qualification.
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::fleet::Executable;
use bounded_exec::{
    CommandSpec, Completion, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroize;

use kiln_microvm::backend::{
    Capabilities, CloudHypervisor, ConfinementKinds, Firecracker, NetworkCapability, VmmBackend,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub path: PathBuf,
    pub blake3: String,
}
impl Image {
    pub(crate) fn verify(&self) -> Result<PathBuf, String> {
        // Images and executables share the immutable content identity contract.
        Executable {
            path: self.path.clone(),
            blake3: self.blake3.clone(),
        }
        .verify()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Backend {
    Namespaces {
        unshare: Executable,
        python: Executable,
        mount: Executable,
        shell: Executable,
        runtime_roots: Vec<PathBuf>,
        /// Optional reviewed guest-network request; only `none` is admissible.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        network: Option<String>,
    },
    Qemu {
        qemu: Executable,
        kernel: Image,
        initramfs: Image,
        root_image: Option<Image>,
        guest_shell: String,
        acceleration: String,
        qualification_identity: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        network: Option<String>,
    },
    /// One Firecracker microVM per job: kernel and initramfs from the reviewed
    /// `microvm-guest` flake output, the source snapshot as a read-only
    /// virtio-blk image, vsock for the guest protocol and no network device.
    Firecracker {
        firecracker: Executable,
        kernel: Image,
        initrd: Image,
        guest_shell: String,
        vcpus: u8,
        memory_mib: u32,
        boot_timeout_ms: u64,
        source_image_mib: u32,
        warm_pool: Option<crate::microvm::WarmPool>,
        /// Opt-in debug hold of failed jobs; absent means disabled and keeps
        /// the configuration identity of profiles without the option.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        debug_hold: Option<kiln_microvm::debug_hold::DebugHoldPolicy>,
        qualification_identity: Option<String>,
        /// Operator declaration that this per-job microVM runner may execute
        /// approved untrusted pull-request runs (never with credentials or
        /// cache). Absent means not untrusted-capable and keeps identity.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        untrusted_capable: bool,
        /// Explicit operator override for unsafe host preflight facts on an
        /// untrusted-capable runner; recorded by the receipt.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host_preflight_override: Option<String>,
        /// Reviewed trust-domain label. Runners sharing a domain are treated as
        /// one placement pool; untrusted and workload-bearing pools must differ.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trust_domain: Option<String>,
        /// Reviewed physical-host identity used for static co-residency checks.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host_identity: Option<String>,
        /// Reviewed cache-partition label. This is a placement declaration; it
        /// does not program resctrl or prove LLC separation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_partition: Option<String>,
        /// Reviewed CPU list for this runner's VMM, e.g. `0-3,8,10-11`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cpu_affinity: Option<String>,
        /// Reviewed VMM version; recorded in receipts and snapshot identity v2.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        network: Option<String>,
    },
    /// One Cloud Hypervisor microVM per job. Cold boot only in phase 1; the
    /// same guest agent, read-only source drive and vsock protocol apply.
    #[serde(rename = "cloud-hypervisor")]
    CloudHypervisor {
        cloud_hypervisor: Executable,
        kernel: Image,
        initrd: Image,
        guest_shell: String,
        vcpus: u8,
        memory_mib: u32,
        boot_timeout_ms: u64,
        source_image_mib: u32,
        qualification_identity: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        warm_pool: Option<crate::microvm::WarmPool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        network: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cpu_affinity: Option<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretEnvironment {
    pub environment: String,
    pub name: String,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkloadPolicy {
    pub organization: String,
    pub project: String,
    pub runner: String,
    pub audience: Option<String>,
    pub secrets: Vec<SecretEnvironment>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedProject {
    pub organization: String,
    pub project: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingPolicy {
    pub organization: String,
    pub runner: String,
    pub price_id: String,
    pub class: String,
    pub max_units: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub systemd_run: Executable,
    pub systemctl: Executable,
    /// Absolute taskset path; required only when a Firecracker runner pins CPUs.
    #[serde(default)]
    pub taskset: Option<Executable>,
    pub manager_runtime_directory: PathBuf,
    #[serde(default)]
    pub workload: Vec<WorkloadPolicy>,
    #[serde(default)]
    pub trusted_projects: Vec<TrustedProject>,
    #[serde(default)]
    pub pricing: Vec<PricingPolicy>,
    #[serde(default)]
    pub cache: Option<crate::cache::HostedCacheConfig>,
    pub env: Executable,
    pub work_root: PathBuf,
    pub runners: BTreeMap<String, Backend>,
    pub maximum_concurrent: u32,
    pub memory_bytes: u64,
    pub processes: u32,
    pub cpu_percent: u32,
    pub maximum_timeout_ms: u64,
    pub maximum_output_bytes: usize,
}
fn valid_cpu_affinity(value: &str) -> bool {
    if value.is_empty() || value.len() > 256 || value.contains('\0') {
        return false;
    }
    let mut seen = std::collections::BTreeSet::new();
    for item in value.split(',') {
        let mut range = item.split('-');
        let Some(start) = range.next().and_then(|v| v.parse::<u32>().ok()) else {
            return false;
        };
        let end = match range.next() {
            Some(end) => match end.parse::<u32>() {
                Ok(end) => end,
                Err(_) => return false,
            },
            None => start,
        };
        if range.next().is_some() || start > end || end > 4095 {
            return false;
        }
        for cpu in start..=end {
            if !seen.insert(cpu) {
                return false;
            }
        }
    }
    true
}

fn validate_trust_domains(
    runners: &std::collections::BTreeMap<String, Backend>,
    workload: &[WorkloadPolicy],
) -> Result<(), String> {
    let mut domains: std::collections::BTreeMap<&str, (bool, bool)> =
        std::collections::BTreeMap::new();
    let mut hosts: std::collections::BTreeMap<&str, (bool, bool)> =
        std::collections::BTreeMap::new();
    let mut partitions: std::collections::BTreeMap<&str, (bool, bool)> =
        std::collections::BTreeMap::new();
    for (label, backend) in runners {
        if let Backend::Firecracker {
            trust_domain,
            host_identity,
            cache_partition,
            cpu_affinity,
            untrusted_capable,
            ..
        } = backend
        {
            if *untrusted_capable
                && trust_domain.as_ref().is_none_or(|domain| {
                    domain.trim().is_empty() || domain.len() > 128 || domain.contains('\0')
                })
            {
                return Err("untrusted-capable Firecracker runner needs a trust domain".into());
            }
            if let Some(affinity) = cpu_affinity
                && !valid_cpu_affinity(affinity)
            {
                return Err("invalid Firecracker CPU affinity".into());
            }
            if *untrusted_capable
                && host_identity.as_ref().is_none_or(|host| {
                    host.trim().is_empty() || host.len() > 128 || host.contains('\0')
                })
            {
                return Err("untrusted-capable Firecracker runner needs a host identity".into());
            }
            let domain = trust_domain.as_deref().unwrap_or(label.as_str());
            if domain.trim().is_empty() || domain.len() > 128 || domain.contains('\0') {
                return Err("invalid Firecracker trust domain".into());
            }
            let workload = workload.iter().any(|policy| policy.runner == *label);
            let entry = domains.entry(domain).or_default();
            entry.0 |= *untrusted_capable;
            entry.1 |= workload;
            let host = host_identity.as_deref().unwrap_or(label.as_str());
            if host.trim().is_empty() || host.len() > 128 || host.contains('\0') {
                return Err("invalid Firecracker host identity".into());
            }
            let host_entry = hosts.entry(host).or_default();
            host_entry.0 |= *untrusted_capable;
            host_entry.1 |= workload;
            if let Some(partition) = cache_partition {
                if partition.trim().is_empty() || partition.len() > 128 || partition.contains('\0')
                {
                    return Err("invalid cache partition label".into());
                }
                let partition_entry = partitions.entry(partition).or_default();
                partition_entry.0 |= *untrusted_capable;
                partition_entry.1 |= workload;
            }
        }
    }
    if domains
        .values()
        .any(|(untrusted, workload)| *untrusted && *workload)
    {
        return Err("untrusted and workload-bearing runners cannot share a trust domain".into());
    }
    if hosts
        .values()
        .any(|(untrusted, workload)| *untrusted && *workload)
    {
        return Err("untrusted and workload-bearing runners cannot share a host".into());
    }
    if partitions
        .values()
        .any(|(untrusted, workload)| *untrusted && *workload)
    {
        return Err("untrusted and workload-bearing runners cannot share a cache partition".into());
    }
    Ok(())
}

impl Config {
    fn validate(&self) -> Result<(), String> {
        if self.maximum_concurrent == 0
            || self.maximum_concurrent > 128
            || !(67_108_864..=68_719_476_736).contains(&self.memory_bytes)
            || !(8..=1024).contains(&self.processes)
            || !(1..=800).contains(&self.cpu_percent)
            || !(100..=3_600_000).contains(&self.maximum_timeout_ms)
            || !(1024..=1_048_576).contains(&self.maximum_output_bytes)
            || self.runners.is_empty()
            || self.runners.len() > 32
            || !clean(&self.work_root)
        {
            return Err("execution policy outside reviewed finite bounds".into());
        }
        self.systemd_run.verify()?;
        self.systemctl.verify()?;
        self.env.verify()?;
        if self.runners.values().any(|backend| {
            matches!(
                backend,
                Backend::Firecracker {
                    cpu_affinity: Some(_),
                    ..
                }
            )
        }) {
            self.taskset
                .as_ref()
                .ok_or("CPU affinity requires a configured taskset")?
                .verify()?;
        }
        for backend in self.runners.values() {
            if backend_declared_network(backend).is_some_and(|network| network != "none") {
                return Err("backend requests a guest network device".into());
            }
        }
        if let Some(cache) = &self.cache {
            cache.validate(&self.work_root)?;
            if cache
                .policies
                .iter()
                .any(|p| !self.runners.contains_key(&p.runner))
            {
                return Err("cache references an unreviewed runner".into());
            }
        }
        if !clean(&self.manager_runtime_directory)
            || self.manager_runtime_directory.parent() != Some(Path::new("/run/user"))
            || self
                .manager_runtime_directory
                .file_name()
                .and_then(|v| v.to_str())
                .is_none_or(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err("user manager runtime directory invalid".into());
        }
        if self.workload.len() > 128 {
            return Err("workload policy bound exceeded".into());
        }
        if self.trusted_projects.len() > 128
            || self
                .trusted_projects
                .iter()
                .any(|p| !crate::identifier(&p.organization) || !crate::identifier(&p.project))
        {
            return Err("invalid operator trusted-project allowlist".into());
        }
        if self.pricing.len() > 128 {
            return Err("execution pricing policy bound exceeded".into());
        }
        let mut prices = std::collections::BTreeSet::new();
        for price in &self.pricing {
            if !crate::identifier(&price.organization)
                || !self.runners.contains_key(&price.runner)
                || !crate::identifier(&price.price_id)
                || !crate::identifier(&price.class)
                || price.max_units == 0
                || price.max_units > 1_000_000_000
                || !prices.insert((
                    &price.organization,
                    &price.runner,
                    &price.price_id,
                    &price.class,
                ))
            {
                return Err("invalid exact operator execution price".into());
            }
        }
        validate_trust_domains(&self.runners, &self.workload)?;
        let mut grants = std::collections::BTreeSet::new();
        for policy in &self.workload {
            if !crate::identifier(&policy.organization)
                || !crate::identifier(&policy.project)
                || !self.runners.contains_key(&policy.runner)
                || policy.secrets.len() > 32
                || !grants.insert((&policy.organization, &policy.project, &policy.runner))
                || policy
                    .audience
                    .as_ref()
                    .is_some_and(|v| v.is_empty() || v.len() > 256)
            {
                return Err("invalid exact workload policy".into());
            }
            let mut names = std::collections::BTreeSet::new();
            for secret in &policy.secrets {
                if !crate::identifier(&secret.name)
                    || secret.revision == 0
                    || !environment_name(&secret.environment)
                    || !names.insert(&secret.environment)
                {
                    return Err("invalid secret environment grant".into());
                }
            }
        }
        let root = self
            .work_root
            .canonicalize()
            .map_err(|_| "execution workspace root unavailable")?;
        if root == Path::new("/") || root.starts_with("/nix/store") {
            return Err("invalid private customer workspace root".into());
        }
        for (label, backend) in &self.runners {
            if !crate::identifier(label) {
                return Err("invalid runner label".into());
            }
            match backend {
                Backend::Namespaces {
                    unshare,
                    python,
                    mount,
                    shell,
                    runtime_roots,
                    network: _,
                } => {
                    unshare.verify()?;
                    python.verify()?;
                    mount.verify()?;
                    shell.verify()?;
                    if runtime_roots.is_empty()
                        || runtime_roots.len() > 6
                        || runtime_roots.iter().any(|p| {
                            !["/nix/store", "/usr", "/bin", "/lib", "/lib64"]
                                .iter()
                                .any(|r| p == Path::new(r))
                        })
                    {
                        return Err(
                            "only read-only runtime roots may enter customer namespace".into()
                        );
                    }
                }
                Backend::Qemu {
                    qemu,
                    kernel,
                    initramfs,
                    root_image,
                    guest_shell,
                    acceleration,
                    qualification_identity,
                    network: _,
                } => {
                    qemu.verify()?;
                    kernel.verify()?;
                    initramfs.verify()?;
                    if let Some(image) = root_image {
                        image.verify()?;
                    }
                    if !["tcg", "kvm"].contains(&acceleration.as_str())
                        || !guest_shell.starts_with('/')
                        || guest_shell.len() > 4096
                        || guest_shell.contains('\0')
                    {
                        return Err("invalid VM backend".into());
                    }
                    qualification_reference(qualification_identity)?;
                }
                Backend::CloudHypervisor {
                    cloud_hypervisor,
                    kernel,
                    initrd,
                    ..
                } => {
                    admit_cloud_hypervisor(self, label, backend)?;
                    cloud_hypervisor.verify()?;
                    kernel.verify()?;
                    initrd.verify()?;
                }
                Backend::Firecracker {
                    firecracker,
                    kernel,
                    initrd,
                    warm_pool,
                    ..
                } => {
                    admit_firecracker(self, label, backend)?;
                    firecracker.verify()?;
                    kernel.verify()?;
                    initrd.verify()?;
                    if let Some(pool) = warm_pool {
                        crate::microvm::verify_pool_directory(&pool.directory)?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn qualification_reference(identity: &Option<String>) -> Result<(), String> {
    match identity {
        Some(identity)
            if identity.len() != 64 || !identity.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Err("invalid independent VM qualification identity".into())
        }
        _ => Ok(()),
    }
}

// r[impl kiln.hosted.microvm.admission]
// r[impl kiln.hosted.microvm.admission.scenario.bounds]
/// Pure Firecracker runner admission against this execution policy. Image and
/// pool filesystem checks follow in the shell.
fn backend_declared_network(backend: &Backend) -> Option<&str> {
    match backend {
        Backend::Namespaces { network, .. }
        | Backend::Qemu { network, .. }
        | Backend::Firecracker { network, .. }
        | Backend::CloudHypervisor { network, .. } => network.as_deref(),
    }
}

fn backend_capabilities(backend: &Backend) -> Capabilities {
    match backend {
        Backend::Firecracker { .. } => Firecracker.capabilities(),
        Backend::CloudHypervisor { .. } => CloudHypervisor.capabilities(),
        Backend::Qemu { .. } | Backend::Namespaces { .. } => Capabilities {
            network: NetworkCapability::NoDevice,
            snapshot: false,
            arch: &["x86_64", "aarch64"],
            confinement: ConfinementKinds {
                seccomp: false,
                landlock: false,
                no_new_privs: false,
                cap_eff: false,
            },
            per_job: false,
        },
    }
}

fn cache_partition_admitted(partition: Option<&str>, resctrl_mounted: bool) -> bool {
    partition.is_none() || resctrl_mounted
}

fn admit_cache_partition(
    backend: &Backend,
    host: &kiln_microvm::host::HostFacts,
) -> Result<(), String> {
    let partition = match backend {
        Backend::Firecracker {
            cache_partition: partition,
            ..
        } => partition.as_deref(),
        _ => None,
    };
    if cache_partition_admitted(partition, host.resctrl_mounted) {
        Ok(())
    } else {
        Err("cache partition labels require an observed resctrl mount".into())
    }
}

fn admit_snapshot(capabilities: &Capabilities, requested: bool) -> Result<(), String> {
    if requested && !capabilities.snapshot {
        return Err("warm pool requires a snapshot-capable backend".into());
    }
    Ok(())
}

fn admit_cloud_hypervisor(config: &Config, label: &str, backend: &Backend) -> Result<(), String> {
    let Backend::CloudHypervisor {
        kernel,
        initrd,
        qualification_identity,
        warm_pool,
        version,
        cpu_affinity,
        ..
    } = backend
    else {
        return Err("not a Cloud Hypervisor runner".into());
    };
    let capabilities = backend_capabilities(backend);
    if !capabilities.admits_arch(std::env::consts::ARCH) {
        return Err("Cloud Hypervisor runner arch is not in its reviewed capability set".into());
    }
    if backend_declared_network(backend).is_some_and(|network| network != "none") {
        return Err("backend requests a guest network device".into());
    }
    if version.as_ref().is_some_and(|version| {
        version.trim().is_empty() || version.len() > 128 || version.contains('\0')
    }) {
        return Err("invalid Cloud Hypervisor version".into());
    }
    if cpu_affinity
        .as_ref()
        .is_some_and(|affinity| !valid_cpu_affinity(affinity))
    {
        return Err("invalid Cloud Hypervisor CPU affinity".into());
    }
    kernel.verify()?;
    initrd.verify()?;
    qualification_reference(qualification_identity)?;
    let policy = crate::microvm::machine_policy(backend).ok_or("not a Cloud Hypervisor runner")?;
    kiln_microvm::admission::admit_machine(
        &policy,
        &kiln_microvm::admission::HostBounds {
            unit_memory_bytes: config.memory_bytes,
            unit_tasks: config.processes,
            maximum_concurrent: config.maximum_concurrent,
            maximum_timeout_ms: config.maximum_timeout_ms,
        },
    )?;
    if let Some(pool) = warm_pool {
        crate::microvm::admit_pool_path(&pool.directory, &config.work_root)?;
    }
    if config
        .cache
        .as_ref()
        .is_some_and(|cache| cache.policies.iter().any(|p| p.runner == label))
    {
        return Err("cache policies cannot name a Cloud Hypervisor runner".into());
    }
    Ok(())
}

fn admit_firecracker(config: &Config, _label: &str, backend: &Backend) -> Result<(), String> {
    let Backend::Firecracker {
        qualification_identity,
        host_preflight_override,
        warm_pool,
        debug_hold,
        version,
        ..
    } = backend
    else {
        return Err("not a Firecracker runner".into());
    };
    if version.as_ref().is_some_and(|version| {
        version.trim().is_empty() || version.len() > 128 || version.contains('\0')
    }) {
        return Err("invalid Firecracker version".into());
    }
    let capabilities = backend_capabilities(backend);
    if !capabilities.admits_arch(std::env::consts::ARCH) {
        return Err("Firecracker runner arch is not in its reviewed capability set".into());
    }
    if host_preflight_override.as_ref().is_some_and(|reason| {
        reason.trim().is_empty() || reason.len() > 1024 || reason.contains('\0')
    }) {
        return Err("invalid host preflight override".into());
    }
    let policy = crate::microvm::machine_policy(backend).ok_or("not a Firecracker runner")?;
    kiln_microvm::admission::admit_machine(
        &policy,
        &kiln_microvm::admission::HostBounds {
            unit_memory_bytes: config.memory_bytes,
            unit_tasks: config.processes,
            maximum_concurrent: config.maximum_concurrent,
            maximum_timeout_ms: config.maximum_timeout_ms,
        },
    )?;
    qualification_reference(qualification_identity)?;
    admit_snapshot(&capabilities, warm_pool.is_some())?;
    if let Some(pool) = warm_pool {
        crate::microvm::admit_pool_path(&pool.directory, &config.work_root)?;
    }
    // r[impl kiln.hosted.microvm.debug_hold.policy]
    // r[impl kiln.hosted.microvm.debug_hold.policy.scenario.bounds]
    if let Some(policy) = debug_hold {
        kiln_microvm::debug_hold::admit(policy, config.maximum_concurrent)?;
    }
    Ok(())
}
fn clean(p: &Path) -> bool {
    p.is_absolute()
        && p.components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
}

pub fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_execution_config(singleton INTEGER PRIMARY KEY CHECK(singleton=1),identity TEXT NOT NULL,config TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS hosted_execution(org TEXT NOT NULL,project TEXT NOT NULL,attempt TEXT NOT NULL,pipeline TEXT NOT NULL,request_identity TEXT NOT NULL,config_identity TEXT NOT NULL,unit TEXT NOT NULL,state TEXT NOT NULL,reserved_ms INTEGER NOT NULL,result TEXT,PRIMARY KEY(org,attempt));
      CREATE TABLE IF NOT EXISTS hosted_microvm_job(org TEXT NOT NULL,attempt TEXT NOT NULL,job TEXT NOT NULL,lifecycle TEXT NOT NULL,result TEXT,PRIMARY KEY(org,attempt,job));
      CREATE TABLE IF NOT EXISTS hosted_microvm_placement(org TEXT NOT NULL,attempt TEXT NOT NULL,job TEXT NOT NULL,host TEXT NOT NULL,trust_domain TEXT NOT NULL,class TEXT NOT NULL CHECK(class IN('untrusted','workload','trusted')),started_ms INTEGER NOT NULL,PRIMARY KEY(org,attempt,job));
      CREATE INDEX IF NOT EXISTS hosted_microvm_placement_host ON hosted_microvm_placement(host);") .map_err(|e|e.to_string())?;
    crate::debug_hold::init(db)
}
/// Trusted startup only. Existing in-flight/unknown effects pin their configuration.
pub fn configure(db: &mut Connection, config: &Config) -> Result<(), String> {
    config.validate()?;
    let identity = crate::identity(config)?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let old: Option<String> = tx
        .query_row(
            "SELECT identity FROM hosted_execution_config WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if old.as_ref().is_some_and(|v| v != &identity) {
        let active:u64=tx.query_row("SELECT count(*) FROM hosted_execution WHERE state IN ('reserved','running','unknown')",[],|r|crate::sqlite_u64(r,0)).map_err(|e|e.to_string())?;
        if active > 0 {
            return Err("execution configuration pinned by uncertain effects".into());
        }
    }
    tx.execute("INSERT INTO hosted_execution_config VALUES(1,?1,?2) ON CONFLICT(singleton) DO UPDATE SET identity=excluded.identity,config=excluded.config",params![identity,serde_json::to_string(config).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}
fn configuration(db: &Connection) -> Result<(String, Config), String> {
    let (id, raw): (String, String) = db
        .query_row(
            "SELECT identity,config FROM hosted_execution_config WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "execution backend not configured")?;
    let config: Config = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    config.validate()?;
    Ok((id, config))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineRequest {
    pub project: String,
    pub pipeline_id: String,
    pub attempt_id: String,
    pub price_id: String,
    pub class: String,
    /// Relative to work_root/<organization>/<project>; only customer source here.
    pub workspace: String,
    pub trust: String,
    /// Source event to evaluate against the plan's trigger filters; omitted
    /// requests run unconditionally, exactly as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<crate::pipelines::SourceEvent>,
    /// Approved untrusted pull-request run (or a retry of one): only
    /// untrusted-capable microVM runners, no secrets, no workload token and no
    /// cache. Omitted for trusted requests, keeping their request identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untrusted: Option<kiln_core::untrusted::UntrustedProvenance>,
}

fn workspace(
    config: &Config,
    organization: &str,
    request: &PipelineRequest,
) -> Result<PathBuf, String> {
    let relative = Path::new(&request.workspace);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("workspace must be a customer-root relative directory".into());
    }
    let base = config
        .work_root
        .canonicalize()
        .map_err(|_| "workspace root unavailable")?;
    let scoped = base.join(organization).join(&request.project);
    let root = scoped
        .canonicalize()
        .map_err(|_| "tenant workspace root unavailable")?;
    if root != scoped {
        return Err("tenant workspace root must not traverse symlinks".into());
    }
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|_| "customer source workspace unavailable")?;
    if !path.starts_with(&root) || path == root || !path.is_dir() {
        return Err("workspace escapes customer root".into());
    }
    Ok(path)
}
// r[impl kiln.hosted.execution]
fn admit_backend(backend: &Backend, trust: &str) -> Result<(), String> {
    if backend_capabilities(backend).network != NetworkCapability::NoDevice
        || backend_declared_network(backend).is_some_and(|network| network != "none")
    {
        return Err("backend requests a guest network device".into());
    }
    match (trust,backend) {
        ("trusted",_)=>Ok(()),
        ("hostile",Backend::Qemu{qualification_identity:Some(_),..}|Backend::Firecracker{qualification_identity:Some(_),..})=>Ok(()),
        ("hostile",_)=>Err("hostile execution blocked: independently qualified VM backend and immutable images required; namespaces are trusted-only".into()),
        _=>Err("trust must be trusted or hostile".into()),
    }
}

/// The exact attempt a trigger decision is recorded against.
struct Attempt<'a> {
    organization: &'a str,
    request: &'a PipelineRequest,
    identity: &'a str,
    config_identity: &'a str,
    now_ms: u64,
}

// r[impl kiln.triggers.record]
/// Evaluate a request's source event against the plan's trigger filters.
/// `Ok(Some(..))` is the matched trigger and its reasons for the run record;
/// `Ok(None)` means no event (unchanged behaviour). An event no trigger admits
/// is recorded durably as a `skipped` attempt naming each excluding filter,
/// with no finance reservation and no process, and returned as `Err(record)`.
fn trigger_gate(
    db: &Connection,
    attempt: &Attempt<'_>,
    plan: &crate::pipelines::CompiledPipeline,
    workspace: &Path,
) -> Result<Result<Option<Value>, Value>, String> {
    let Some(event) = &attempt.request.event else {
        return Ok(Ok(None));
    };
    match crate::pipelines::select_trigger(plan, event, workspace)? {
        kiln_core::trigger_filter::Selection::Matched {
            index,
            event,
            reasons,
        } => Ok(Ok(Some(
            json!({"index":index,"event":event,"reasons":reasons}),
        ))),
        kiln_core::trigger_filter::Selection::Excluded { exclusions } => {
            let request = attempt.request;
            let record = json!({"attempt_id":request.attempt_id,"pipeline":request.pipeline_id,"state":"skipped","excluded":exclusions});
            db.execute(
                "INSERT INTO hosted_execution VALUES(?1,?2,?3,?4,?5,?6,'','skipped',?7,?8)",
                params![
                    attempt.organization,
                    request.project,
                    request.attempt_id,
                    request.pipeline_id,
                    attempt.identity,
                    attempt.config_identity,
                    crate::sqlite_integer(attempt.now_ms)?,
                    record.to_string()
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(Err(record))
        }
    }
}

// r[impl kiln.hosted.execution]
/// Real pipeline entry point. Compiled commands come only from the reviewed plan,
/// finance reserves before the first child can start, and retries never rerun an
/// interrupted attempt. Parallel-ready jobs are executed sequentially.
pub fn run_pipeline(
    db: &mut Connection,
    actor: &crate::Actor,
    request: PipelineRequest,
    now_ms: u64,
    cancellation: &AtomicBool,
) -> Result<Value, String> {
    let actor = crate::accounts::require_project(
        db,
        actor,
        &request.project,
        &["owner", "admin", "member"],
    )?;
    if !crate::identifier(&request.attempt_id)
        || !crate::identifier(&request.price_id)
        || !crate::identifier(&request.class)
        || now_ms > i64::MAX as u64
    {
        return Err("invalid execution admission identity".into());
    }
    let plan = crate::pipelines::reviewed(db, &actor, &request.project, &request.pipeline_id)?;
    let (config_id, config) = configuration(db)?;
    let work = workspace(&config, &actor.organization, &request)?;
    let identity = crate::identity(
        &json!({"organization":actor.organization,"request":request,"configuration":config_id,"plan":plan.identity}),
    )?;
    let prior:Option<(String,String,Option<String>)>=db.query_row("SELECT request_identity,state,result FROM hosted_execution WHERE org=?1 AND attempt=?2",params![actor.organization,request.attempt_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|e|e.to_string())?;
    if let Some((old, state, result)) = prior {
        if old != identity {
            return Err("execution attempt identity conflict".into());
        }
        return result.map(|r|serde_json::from_str(&r).map_err(|e|e.to_string())).unwrap_or_else(||Ok(json!({"attempt_id":request.attempt_id,"state":state,"uncertain":true,"rerun":false})));
    }
    let attempt = Attempt {
        organization: &actor.organization,
        request: &request,
        identity: &identity,
        config_identity: &config_id,
        now_ms,
    };
    let trigger = match trigger_gate(db, &attempt, &plan, &work)? {
        Ok(trigger) => trigger,
        Err(skipped) => return Ok(skipped),
    };
    if plan.jobs.is_empty() || plan.jobs.len() > 128 {
        return Err("pipeline execution size rejected".into());
    }
    let mut maximum_ms = 0_u64;
    let mut step_count = 0_usize;
    let mut operator_maximum = u64::MAX;
    let mut hold_reserve_ms = 0_u64;
    let host_facts = crate::microvm::read_host_facts();
    for job in &plan.jobs {
        let backend = config
            .runners
            .get(&job.runner)
            .ok_or_else(|| format!("runner '{}' has no reviewed backend mapping", job.runner))?;
        admit_backend(backend, &request.trust)?;
        admit_untrusted_runner(backend, request.untrusted.as_ref())?;
        if request.untrusted.is_some() {
            admit_untrusted_host(backend, &host_facts)?;
        }
        admit_cache_partition(backend, &host_facts)?;
        if request.trust == "trusted"
            && !config
                .trusted_projects
                .iter()
                .any(|p| p.organization == actor.organization && p.project == request.project)
        {
            return Err("trusted execution requires operator-admitted organization/project, not a customer trust assertion".into());
        }
        let price = config
            .pricing
            .iter()
            .find(|p| {
                p.organization == actor.organization
                    && p.runner == job.runner
                    && p.price_id == request.price_id
                    && p.class == request.class
            })
            .ok_or("organization/runner has no matching operator-approved execution price/class")?;
        operator_maximum = operator_maximum.min(price.max_units);
        if job.timeout_ms == 0
            || job.timeout_ms > config.maximum_timeout_ms
            || job.steps.is_empty()
            || job.steps.len() > 128
        {
            return Err("job resource/deadline policy denied".into());
        }
        step_count = step_count
            .checked_add(job.steps.len())
            .ok_or("step count overflow")?;
        if step_count > 128 {
            return Err("pipeline step limit exceeded".into());
        }
        maximum_ms = maximum_ms
            .checked_add(job.timeout_ms)
            .and_then(|v| v.checked_add(20_000))
            .ok_or("pipeline deadline overflow")?;
        // At most one job per attempt fails (later jobs are skipped), so one
        // grace covers every hold; zero unless a hold is possible at all.
        hold_reserve_ms = hold_reserve_ms.max(kiln_microvm::debug_hold::reserve_ms(
            debug_hold_policy(backend),
            hold_grants(&config, &actor.organization, &request, &job.runner),
        ));
    }
    maximum_ms = maximum_ms
        .checked_add(hold_reserve_ms)
        .ok_or("pipeline deadline overflow")?;
    if maximum_ms > 3_600_000 {
        return Err("pipeline admission exceeds one-hour capability boundary".into());
    }
    let price = crate::finance::trusted_price(db, &actor.organization, &request.price_id)?;
    if price.class != request.class {
        return Err("operator price class mismatch".into());
    }
    let maximum_units = reservation_units(
        maximum_ms,
        price.unit_ms,
        operator_maximum.min(price.max_units),
    )?;
    let unit = format!(
        "kiln-hosted-{}.service",
        identity
            .get(..32)
            .ok_or("execution identity length invalid")?
    );
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if let Err(error) = check_capacity(&tx, config.maximum_concurrent) {
        drop(tx);
        // r[impl kiln.hosted.microvm.debug_hold.end]
        // A denied admission is capacity pressure; held VMs give way.
        if config
            .runners
            .values()
            .any(|backend| debug_hold_policy(backend).is_some_and(|policy| policy.enabled))
        {
            crate::debug_hold::record_capacity_pressure(db)?;
        }
        return Err(error);
    }
    tx.execute(
        "INSERT INTO hosted_execution VALUES(?1,?2,?3,?4,?5,?6,?7,'reserved',?8,NULL)",
        params![
            actor.organization,
            request.project,
            request.attempt_id,
            request.pipeline_id,
            identity,
            config_id,
            unit,
            crate::sqlite_integer(now_ms)?
        ],
    )
    .map_err(|e| e.to_string())?;
    reserve_placements(
        &tx,
        &actor.organization,
        &request.attempt_id,
        &plan,
        &request.project,
        &config,
        untrusted_run(&request),
        now_ms,
    )?;
    tx.commit().map_err(|e| e.to_string())?;
    let financial_exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM finance_reservations WHERE org=?1 AND attempt=?2)",
            params![actor.organization, request.attempt_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if financial_exists {
        db.execute(
            "UPDATE hosted_execution SET state='not_dispatched' WHERE org=?1 AND attempt=?2",
            params![actor.organization, request.attempt_id],
        )
        .map_err(|e| e.to_string())?;
        return Err(
            "preexisting financial attempt cannot authorize a new execution dispatch".into(),
        );
    }
    let financial = crate::finance::reserve(
        db,
        &actor,
        json!({"attempt_id":request.attempt_id,"price_id":request.price_id,"class":request.class,"max_units":maximum_units,"binding":{"project":request.project,"pipeline":request.pipeline_id,"job":identity}}),
        now_ms,
    );
    if let Err(error) = financial {
        db.execute("UPDATE hosted_execution SET state='not_dispatched',result=?3 WHERE org=?1 AND attempt=?2",params![actor.organization,request.attempt_id,json!({"state":"not_dispatched","reason":error}).to_string()]).map_err(|e|e.to_string())?;
        return Err(error);
    }
    db.execute(
        "UPDATE hosted_execution SET state='running' WHERE org=?1 AND attempt=?2",
        params![actor.organization, request.attempt_id],
    )
    .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut jobs = Vec::new();
    let mut successful = std::collections::BTreeSet::new();
    let mut state = "succeeded";
    let mut cleanup = true;
    let mut output_budget = config.maximum_output_bytes;
    for job in &plan.jobs {
        if !job.needs.iter().all(|need| successful.contains(need)) {
            jobs.push(json!({"job":job.id,"state":"skipped_dependency"}));
            continue;
        }
        if state != "succeeded" {
            jobs.push(json!({"job":job.id,"state":"skipped"}));
            continue;
        }
        let queued_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let job_started = Instant::now();
        let mut steps = Vec::new();
        let mut microvm: Option<Value> = None;
        let bound_job = crate::identity(
            &json!({"attempt":request.attempt_id,"job":job.id,"organization":actor.organization,"project":request.project}),
        )?;
        // r[impl kiln.untrusted.execution]
        let grants = job_grants(
            &config,
            &actor.organization,
            &request.project,
            &job.runner,
            request.untrusted.is_some(),
        );
        let policy = grants.workload;
        let firecracker = matches!(
            config.runners.get(&job.runner),
            Some(Backend::Firecracker { .. })
        );
        let cache_workspace = firecracker
            .then(tempfile::tempdir)
            .transpose()
            .map_err(|e| e.to_string())?;
        let cache_image_dir = firecracker
            .then(tempfile::tempdir)
            .transpose()
            .map_err(|e| e.to_string())?;
        let job_identity = crate::identity(job)?;
        let cache_now_ms = now_ms
            .checked_add(
                u64::try_from(started.elapsed().as_millis()).map_err(|_| "clock overflow")?,
            )
            .ok_or("clock overflow")?;
        let mut cache_context = crate::cache::Context {
            organization: &actor.organization,
            project: &request.project,
            runner: &job.runner,
            plan_identity: &plan.identity,
            job_identity: &job_identity,
            configuration_identity: &config_id,
            workspace: cache_workspace.as_ref().map_or(&work, |d| d.path()),
            now_ms: cache_now_ms,
        };
        let mut cache_key = None;
        let mut cache_image = None;
        let mut cache_ms = None;
        let mut cache_outcome = "disabled_unconfigured";
        let mut cache_observation = json!({"outcome":cache_outcome,"duration_ms":null});
        let credential_grants =
            policy.is_some_and(|p| p.audience.is_some() || !p.secrets.is_empty());
        if !grants.cache {
            cache_outcome = "disabled_untrusted";
            cache_observation = json!({"outcome":cache_outcome,"duration_ms":null});
        } else if credential_grants {
            cache_outcome = "disabled_secret_grants";
            cache_observation = json!({"outcome":cache_outcome,"duration_ms":null});
        } else if let Some(cache) = &config.cache {
            if cache.policy(&cache_context).is_some() {
                let cache_started = Instant::now();
                match crate::cache::restore(cache, &cache_context) {
                    Ok((key, observation)) => {
                        cache_outcome = if observation.outcome == "hit" {
                            "hit"
                        } else {
                            "miss"
                        };
                        cache_observation = json!({"restore":observation});
                        cache_key = Some(key);
                        if let (true, Some(workspace)) = (firecracker, cache_workspace.as_ref()) {
                            let policy = config
                                .runners
                                .get(&job.runner)
                                .and_then(crate::microvm::machine_policy)
                                .ok_or("cache runner has no machine policy")?;
                            let directory = cache_image_dir
                                .as_ref()
                                .ok_or("cache image directory unavailable")?;
                            let image = directory.path().join("cache.img");
                            crate::microvm::source_image(
                                workspace.path(),
                                &image,
                                u64::from(policy.source_image_mib) * 1_048_576,
                            )?;
                            cache_image = Some(image);
                        }
                    }
                    Err(error) => {
                        cache_outcome = "rejected";
                        state = "failed";
                        cache_observation = json!({"outcome":"rejected","reason":error});
                    }
                }
                cache_ms =
                    Some(u64::try_from(cache_started.elapsed().as_millis()).unwrap_or(u64::MAX));
            } else {
                cache_outcome = "disabled_policy";
                cache_observation = json!({"outcome":cache_outcome,"duration_ms":null});
            }
        }
        let mut secret_environment = BTreeMap::new();
        let mut redactions = Vec::new();
        let mut capability_admitted = false;
        let credential_result = (|| -> Result<(), String> {
            if state != "succeeded" {
                return Err("cache admission rejected before customer dispatch".into());
            }
            let Some(policy) = policy else {
                return Ok(());
            };
            let current_ms = now_ms
                .checked_add(
                    u64::try_from(started.elapsed().as_millis()).map_err(|_| "clock overflow")?,
                )
                .ok_or("clock overflow")?;
            let binding = crate::secrets::JobBinding {
                organization: actor.organization.clone(),
                project: request.project.clone(),
                job: bound_job.clone(),
                admission_identity: identity.clone(),
                expires_ms: now_ms
                    .checked_add(maximum_ms)
                    .ok_or("capability expiry overflow")?,
                audiences: policy.audience.iter().cloned().collect(),
                secrets: policy
                    .secrets
                    .iter()
                    .map(|s| crate::secrets::SecretGrant {
                        name: s.name.clone(),
                        revision: s.revision,
                    })
                    .collect(),
            };
            let capability = crate::secrets::begin_job(db, &binding, current_ms)?;
            capability_admitted = true;
            for grant in &policy.secrets {
                let delivered = crate::secrets::resolve_for_job(
                    db,
                    capability.as_str(),
                    &grant.name,
                    current_ms,
                )?;
                let value = delivered
                    .value
                    .as_str()
                    .map_err(|_| "delivered secret is not valid UTF-8")?
                    .to_owned();
                redactions.push(value.clone());
                secret_environment.insert(grant.environment.clone(), value);
            }
            if let Some(audience) = &policy.audience {
                let token =
                    crate::secrets::exchange(db, capability.as_str(), audience, current_ms)?;
                let value = token.as_str().to_owned();
                redactions.push(value.clone());
                secret_environment.insert("KILN_WORKLOAD_TOKEN".into(), value);
            }
            Ok(())
        })();
        if let Err(error) = credential_result {
            state = "failed";
            steps.push(json!({"state":"not_dispatched","reason":error}));
        } else if let Some(runner) = config.runners.get(&job.runner).and_then(|backend| {
            crate::microvm::Runner::from_backend(backend)
                .or_else(|| crate::microvm::Runner::from_cloud_hypervisor(backend))
        }) {
            let elapsed = u64::try_from(job_started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let remaining = job.timeout_ms.saturating_sub(elapsed);
            if remaining == 0 {
                state = "failed";
                steps.push(json!({"state":"timed_out"}));
            } else if output_budget < 1024 {
                state = "failed";
                steps.push(json!({"state":"output_budget_exhausted"}));
            } else {
                let output_limit = (output_budget / 12).max(1);
                let mut guest_steps = Vec::with_capacity(job.steps.len());
                let mut rejected = None;
                for step in &job.steps {
                    let mut environment = step.env.clone();
                    for (name, value) in &secret_environment {
                        environment.insert(name.clone(), value.clone());
                    }
                    match kiln_microvm::protocol::guest_step(
                        runner.policy.guest_shell,
                        matches!(step.shell, crate::pipelines::Shell::Bash),
                        &step.run,
                        &step.working_directory,
                        &environment,
                        output_limit,
                    ) {
                        Ok(guest) => guest_steps.push((step.name.clone(), guest)),
                        Err(error) => rejected = Some(error),
                    }
                    for value in environment.values_mut() {
                        value.zeroize();
                    }
                }
                if let Some(error) = rejected {
                    state = "failed";
                    steps.push(json!({"state":"not_dispatched","reason":error}));
                } else {
                    let mut grants =
                        hold_grants(&config, &actor.organization, &request, &job.runner);
                    // Any delivered credential rules a hold out, whatever its source.
                    grants.secrets |= !secret_environment.is_empty();
                    let host_preflight_override =
                        config
                            .runners
                            .get(&job.runner)
                            .and_then(|backend| match backend {
                                Backend::Firecracker {
                                    host_preflight_override,
                                    ..
                                } => host_preflight_override.as_deref(),
                                _ => None,
                            });
                    let host_of = |runner: &str| -> String {
                        config
                            .runners
                            .get(runner)
                            .and_then(|backend| match backend {
                                Backend::Firecracker { host_identity, .. } => {
                                    host_identity.as_deref()
                                }
                                _ => None,
                            })
                            .unwrap_or(runner)
                            .to_string()
                    };
                    let current_host = host_of(&job.runner);
                    let attempt_co_residency = plan
                        .jobs
                        .iter()
                        .filter(|other| {
                            other.id != job.id && host_of(&other.runner) == current_host
                        })
                        .map(|other| (request.attempt_id.clone(), other.id.clone()))
                        .collect::<Vec<_>>();
                    let outcome = crate::microvm::run_job(
                        db,
                        &config,
                        &runner,
                        &crate::microvm::Job {
                            organization: &actor.organization,
                            project: &request.project,
                            attempt: &request.attempt_id,
                            job: &job.id,
                            runner: &job.runner,
                            workspace: &work,
                            steps: &guest_steps,
                            unit: &unit,
                            timeout_ms: remaining,
                            cache_image: cache_image.as_deref(),
                            grants,
                            attempt_co_residency: &attempt_co_residency,
                        },
                        &host_facts,
                        host_preflight_override,
                        cancellation,
                    );
                    for (_, guest) in &mut guest_steps {
                        for value in guest.environment.values_mut() {
                            value.zeroize();
                        }
                    }
                    for observation in outcome.steps {
                        if !absorb(
                            Ok(observation),
                            &mut state,
                            &mut cleanup,
                            &mut output_budget,
                            &redactions,
                            &mut steps,
                        ) {
                            break;
                        }
                    }
                    let mut vm = outcome.vm;
                    redact_value(&mut vm, &redactions);
                    microvm = Some(vm);
                }
            }
        } else {
            for step in &job.steps {
                let elapsed = u64::try_from(job_started.elapsed().as_millis()).unwrap_or(u64::MAX);
                let remaining = job.timeout_ms.saturating_sub(elapsed);
                if remaining == 0 {
                    state = "failed";
                    steps.push(json!({"state":"timed_out"}));
                    break;
                }
                let backend = config
                    .runners
                    .get(&job.runner)
                    .ok_or("admitted runner mapping absent")?;
                if output_budget < 1024 {
                    state = "failed";
                    steps.push(json!({"state":"output_budget_exhausted"}));
                    break;
                }
                let mut environment = step.env.clone();
                for (name, value) in &secret_environment {
                    environment.insert(name.clone(), value.clone());
                }
                let observation = run_step(
                    &config,
                    StepRequest {
                        backend,
                        workspace: &work,
                        step,
                        environment: &environment,
                        unit: &unit,
                        timeout_ms: remaining,
                        output_limit: (output_budget / 12).max(1),
                    },
                    cancellation,
                );
                for name in secret_environment.keys() {
                    if let Some(value) = environment.get_mut(name) {
                        value.zeroize();
                    }
                }
                if !absorb(
                    observation,
                    &mut state,
                    &mut cleanup,
                    &mut output_budget,
                    &redactions,
                    &mut steps,
                ) {
                    break;
                }
            }
        }
        if capability_admitted
            && crate::secrets::finish_job(
                db,
                &actor.organization,
                &request.project,
                &bound_job,
                &identity,
            )
            .is_err()
        {
            state = "unknown";
        }
        for value in secret_environment.values_mut() {
            value.zeroize();
        }
        for value in &mut redactions {
            value.zeroize();
        }
        if state == "succeeded" && cleanup && !firecracker {
            if let (Some(cache), Some(key)) = (&config.cache, &cache_key) {
                let cache_started = Instant::now();
                cache_context.now_ms = now_ms
                    .checked_add(
                        u64::try_from(started.elapsed().as_millis())
                            .map_err(|_| "clock overflow")?,
                    )
                    .ok_or("clock overflow")?;
                let published = crate::cache::publish(cache, &cache_context, key);
                let duration =
                    u64::try_from(cache_started.elapsed().as_millis()).unwrap_or(u64::MAX);
                cache_ms = Some(
                    cache_ms
                        .unwrap_or(0)
                        .checked_add(duration)
                        .ok_or("cache duration overflow")?,
                );
                let fields = cache_observation
                    .as_object_mut()
                    .ok_or("cache observation is not an object")?;
                match published {
                    Ok(observation) => {
                        fields.insert("publish".into(), json!(observation));
                    }
                    Err(error) => {
                        state = "failed";
                        cache_outcome = "rejected";
                        fields.insert(
                            "publish".into(),
                            json!({"outcome":"rejected","reason":error}),
                        );
                    }
                }
            }
        }
        if state == "succeeded" {
            successful.insert(job.id.clone());
        }
        let mut job_record =
            json!({"job":job.id,"state":state,"steps":steps,"cache":cache_observation});
        if let (Some(fields), Some(vm)) = (job_record.as_object_mut(), microvm) {
            fields.insert("microvm".into(), vm);
        }
        jobs.push(job_record);
        crate::fleet::record_timing(
            db,
            &actor.organization,
            &bound_job,
            queued_ms,
            cache_ms,
            now_ms,
            cache_outcome,
        )?;
    }
    let elapsed_ms =
        u64::try_from(started.elapsed().as_millis()).map_err(|_| "host clock duration overflow")?;
    let evidence = crate::identity(
        &json!({"attempt":request.attempt_id,"elapsed_ms":elapsed_ms,"jobs":jobs,"cleaned":cleanup}),
    )?;
    let qualification_references = plan
        .jobs
        .iter()
        .filter_map(|job| match config.runners.get(&job.runner) {
            Some(
                Backend::Qemu {
                    qualification_identity: Some(reference),
                    ..
                }
                | Backend::Firecracker {
                    qualification_identity: Some(reference),
                    ..
                },
            ) => Some(reference.clone()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut result = json!({"attempt_id":request.attempt_id,"pipeline":request.pipeline_id,"state":state,"elapsed_ms":elapsed_ms,"cleaned":cleanup,"evidence":evidence,"jobs":jobs,"requested_trust":request.trust,"hostile_isolation_claim":false,"operator_qualification_references":qualification_references,"qualification_evidence":"external independent evidence required"});
    if let (Some(trigger), Some(fields)) = (trigger, result.as_object_mut()) {
        fields.insert("trigger".into(), trigger);
    }
    let outcome = canonical_outcome(&result)?;
    if let Some(fields) = result.as_object_mut() {
        fields.insert("canonical_outcome".into(), json!(outcome));
    }
    let settlement = if state == "unknown" || !cleanup {
        crate::finance::observe_unknown_trusted(
            db,
            &actor.organization,
            &request.attempt_id,
            &evidence,
            &evidence,
            now_ms,
        )
    } else {
        crate::finance::settle_elapsed_trusted(
            db,
            &actor.organization,
            &request.attempt_id,
            &crate::finance::ElapsedMeasurement {
                receipt_id: &evidence,
                elapsed_ms,
                execution_outcome: state,
                evidence: &evidence,
            },
            now_ms,
        )
    };
    if let Err(error) = settlement {
        let fields = result
            .as_object_mut()
            .ok_or("execution receipt is not an object")?;
        fields.insert("financial_obligation".into(), json!("reserved"));
        fields.insert("settlement_error".into(), json!(error));
    }
    db.execute(
        "UPDATE hosted_execution SET state=?3,result=?4 WHERE org=?1 AND attempt=?2",
        params![
            actor.organization,
            request.attempt_id,
            if cleanup { state } else { "unknown" },
            result.to_string()
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(result)
}
fn reservation_units(maximum_ms: u64, unit_ms: u64, maximum_units: u64) -> Result<u64, String> {
    if unit_ms == 0 {
        return Err("zero price duration".into());
    }
    let units = (maximum_ms / unit_ms)
        .checked_add(u64::from(maximum_ms % unit_ms != 0))
        .ok_or("duration rounding overflow")?;
    if units == 0 || units > maximum_units {
        Err("bounded execution exceeds operator or exact price maximum units".into())
    } else {
        Ok(units)
    }
}
fn debug_hold_policy(backend: &Backend) -> Option<&kiln_microvm::debug_hold::DebugHoldPolicy> {
    match backend {
        Backend::Firecracker { debug_hold, .. } => debug_hold.as_ref(),
        _ => None,
    }
}
/// Hostile trust and approved untrusted pull-request runs are untrusted runs.
fn untrusted_run(request: &PipelineRequest) -> bool {
    request.trust != "trusted" || request.untrusted.is_some()
}
// r[impl kiln.untrusted.execution]
/// Untrusted runs dispatch only to declared untrusted-capable microVM runners,
/// checked for every job before any reservation or effect.
fn admit_untrusted_runner(
    backend: &Backend,
    untrusted: Option<&kiln_core::untrusted::UntrustedProvenance>,
) -> Result<(), String> {
    let Some(provenance) = untrusted else {
        return Ok(());
    };
    if !provenance.admit() {
        return Err("untrusted run provenance invalid".into());
    }
    match backend {
        Backend::Firecracker {
            untrusted_capable: true,
            ..
        } => Ok(()),
        _ => Err("untrusted run requires an untrusted-capable microVM runner".into()),
    }
}
fn admit_untrusted_host(
    backend: &Backend,
    host: &kiln_microvm::host::HostFacts,
) -> Result<(), String> {
    let Backend::Firecracker {
        host_preflight_override,
        ..
    } = backend
    else {
        return Ok(());
    };
    host.admit_untrusted(host_preflight_override.as_deref())
        .map_err(|reason| format!("untrusted host preflight refused: {reason}"))
}

/// Credentials and cache one job may receive.
struct JobGrants<'a> {
    /// Workload policy delivering secrets and `KILN_WORKLOAD_TOKEN`.
    workload: Option<&'a WorkloadPolicy>,
    /// Cache restore and publication.
    cache: bool,
}
// r[impl kiln.untrusted.execution]
/// Untrusted runs, and every retry attempt of one, get no workload policy (so
/// no secret environment and no workload token) and no cache; there is no
/// per-run override. Trusted jobs keep the reviewed workload lookup.
fn job_grants<'a>(
    config: &'a Config,
    organization: &str,
    project: &str,
    runner: &str,
    untrusted: bool,
) -> JobGrants<'a> {
    let grants = kiln_core::untrusted::execution_grants(untrusted);
    let policy = config
        .workload
        .iter()
        .find(|p| p.organization == organization && p.project == project && p.runner == runner);
    JobGrants {
        workload: policy.filter(|_| grants.secrets && grants.workload_token),
        cache: grants.cache_restore && grants.cache_publish,
    }
}
// r[impl kiln.hosted.microvm.debug_hold.lifecycle]
/// Credential and trust facts that rule out a debug hold for one job; the
/// same workload policy lookup that later delivers its credentials.
fn hold_grants(
    config: &Config,
    organization: &str,
    request: &PipelineRequest,
    runner: &str,
) -> kiln_microvm::debug_hold::Grants {
    let policy = config.workload.iter().find(|p| {
        p.organization == organization && p.project == request.project && p.runner == runner
    });
    kiln_microvm::debug_hold::Grants {
        secrets: policy.is_some_and(|p| !p.secrets.is_empty()),
        workload_token: policy.is_some_and(|p| p.audience.is_some()),
        untrusted: untrusted_run(request),
    }
}
/// Static host and trust facts one job contributes to the placement ledger.
fn placement_facts(
    config: &Config,
    organization: &str,
    project: &str,
    job: &crate::pipelines::CompiledJob,
    untrusted: bool,
) -> (String, String, &'static str) {
    let (host, domain) = config
        .runners
        .get(&job.runner)
        .and_then(|backend| match backend {
            Backend::Firecracker {
                host_identity,
                trust_domain,
                ..
            } => Some((
                host_identity.clone().unwrap_or_else(|| job.runner.clone()),
                trust_domain.clone().unwrap_or_else(|| job.runner.clone()),
            )),
            _ => None,
        })
        .unwrap_or_else(|| (job.runner.clone(), job.runner.clone()));
    let class = if untrusted {
        "untrusted"
    } else if config.workload.iter().any(|policy| {
        policy.organization == organization
            && policy.project == project
            && policy.runner == job.runner
    }) {
        "workload"
    } else {
        "trusted"
    };
    (host, domain, class)
}

/// Reserve this attempt's declared host placements before any financial or
/// process effect. A stale row is ignored unless its attempt is active.
fn reserve_placements(
    db: &Connection,
    organization: &str,
    attempt: &str,
    plan: &crate::pipelines::CompiledPipeline,
    project: &str,
    config: &Config,
    untrusted: bool,
    now_ms: u64,
) -> Result<(), String> {
    db.execute(
        "DELETE FROM hosted_microvm_placement WHERE attempt NOT IN (SELECT attempt FROM hosted_execution WHERE state IN ('reserved','running','unknown'))",
        [],
    )
    .map_err(|e| e.to_string())?;
    for job in &plan.jobs {
        let (host, _, class) = placement_facts(config, organization, project, job, untrusted);
        let opposite = if class == "untrusted" {
            "workload"
        } else if class == "workload" {
            "untrusted"
        } else {
            ""
        };
        if !opposite.is_empty() {
            let conflict: i64 = db
                .query_row(
                    "SELECT count(*) FROM hosted_microvm_placement WHERE host=?1 AND class=?2",
                    params![host, opposite],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if conflict > 0 {
                return Err(
                    "host placement conflict: untrusted and workload-bearing jobs overlap".into(),
                );
            }
        }
    }
    for job in &plan.jobs {
        let (host, domain, class) = placement_facts(config, organization, project, job, untrusted);
        db.execute(
            "INSERT INTO hosted_microvm_placement VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                organization,
                attempt,
                job.id,
                host,
                domain,
                class,
                crate::sqlite_integer(now_ms)?
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn check_capacity(db: &Connection, maximum: u32) -> Result<(), String> {
    let occupied: u32 = db
        .query_row(
            "SELECT count(*) FROM hosted_execution WHERE state IN ('reserved','running','unknown')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if occupied >= maximum {
        Err("execution capacity exhausted; unknown effects retain slots".into())
    } else {
        Ok(())
    }
}
fn environment_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit())
}
fn redact_value(value: &mut Value, secrets: &[String]) {
    match value {
        Value::String(text) => {
            for secret in secrets {
                if !secret.is_empty() {
                    *text = text.replace(secret, "[REDACTED]");
                }
            }
        }
        Value::Array(rows) => {
            for row in rows {
                redact_value(row, secrets);
            }
        }
        Value::Object(fields) => {
            for field in fields.values_mut() {
                redact_value(field, secrets);
            }
        }
        _ => {}
    }
}

pub(crate) fn infrastructure_failure_state(
    cancel_requested: bool,
    cleaned: bool,
) -> (&'static str, bool) {
    if !cleaned {
        ("unknown", false)
    } else if cancel_requested {
        ("cancelled", true)
    } else {
        ("failed", false)
    }
}
pub(crate) struct StepObservation {
    pub(crate) success: bool,
    pub(crate) cancelled: bool,
    pub(crate) cleaned: bool,
    pub(crate) value: Value,
}
/// Backend-independent facts about one finished step process.
pub(crate) struct ProcessFacts<'a> {
    pub(crate) name: &'a str,
    pub(crate) exited: bool,
    pub(crate) timed_out: bool,
    pub(crate) cancelled: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) signal: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) stdout_truncated: bool,
    pub(crate) stderr_truncated: bool,
    pub(crate) elapsed_ms: u64,
    pub(crate) cleaned: bool,
}
// r[impl kiln.hosted.microvm.equivalence]
/// The one step-receipt mapping shared by every backend. `extra_success` lets
/// a backend add its own completion evidence (the QEMU guest result marker).
pub(crate) fn step_observation(
    facts: ProcessFacts<'_>,
    extra_success: bool,
    guest: Option<Value>,
) -> StepObservation {
    let success = facts.exited
        && facts.exit_code == Some(0)
        && !facts.stdout_truncated
        && !facts.stderr_truncated
        && extra_success;
    let state = if !facts.cleaned {
        "unknown"
    } else if facts.cancelled {
        "cancelled"
    } else if facts.timed_out {
        "timed_out"
    } else if success {
        "succeeded"
    } else {
        "failed"
    };
    StepObservation {
        success,
        cancelled: facts.cancelled,
        cleaned: facts.cleaned,
        value: json!({"name":facts.name,"state":state,"exit_code":facts.exit_code,"signal":facts.signal,"elapsed_ms":facts.elapsed_ms,"stdout":facts.stdout,"stderr":facts.stderr,"stdout_truncated":facts.stdout_truncated,"stderr_truncated":facts.stderr_truncated,"cleaned":facts.cleaned,"guest":guest}),
    }
}
/// Fold one step observation into the job state. Returns false to stop the job.
fn absorb(
    observation: Result<StepObservation, String>,
    state: &mut &'static str,
    cleanup: &mut bool,
    output_budget: &mut usize,
    redactions: &[String],
    steps: &mut Vec<Value>,
) -> bool {
    match observation {
        Ok(mut observation) => {
            *cleanup &= observation.cleaned;
            if !observation.cleaned {
                *state = "unknown";
            } else if observation.cancelled {
                *state = "cancelled";
            } else if !observation.success {
                *state = "failed";
            }
            redact_value(&mut observation.value, redactions);
            *output_budget = output_budget.saturating_sub(observation.value.to_string().len());
            steps.push(observation.value);
        }
        Err(error) => {
            *state = "unknown";
            *cleanup = false;
            steps.push(json!({"state":"unknown","reason":error}));
        }
    }
    *state == "succeeded"
}
// r[impl kiln.hosted.microvm.equivalence]
// r[impl kiln.hosted.microvm.equivalence.scenario.host]
/// BLAKE3 identity of the backend-independent outcome: pipeline, overall and
/// per-job states, and each step's name, state, exit code and stdout. Timing,
/// cleanup, stderr (the namespace launcher writes its own diagnostics there),
/// signals and backend observations are excluded.
pub(crate) fn canonical_outcome(receipt: &Value) -> Result<String, String> {
    let field = |value: &Value, key: &str| value.get(key).cloned().unwrap_or(Value::Null);
    let jobs: Vec<Value> = receipt
        .get("jobs")
        .and_then(Value::as_array)
        .ok_or("execution receipt has no jobs")?
        .iter()
        .map(|job| {
            let steps: Vec<Value> = job
                .get("steps")
                .and_then(Value::as_array)
                .map(|steps| {
                    steps
                        .iter()
                        .map(|step| {
                            json!({"name":field(step,"name"),"state":field(step,"state"),"exit_code":field(step,"exit_code"),"stdout":field(step,"stdout"),"stdout_truncated":field(step,"stdout_truncated")})
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({"job":field(job,"job"),"state":field(job,"state"),"steps":steps})
        })
        .collect();
    crate::identity(
        &json!({"schema":"kiln.hosted.canonical-outcome.v1","pipeline":field(receipt,"pipeline"),"state":field(receipt,"state"),"jobs":jobs}),
    )
}
struct StepRequest<'a> {
    backend: &'a Backend,
    workspace: &'a Path,
    step: &'a crate::pipelines::CompiledStep,
    environment: &'a BTreeMap<String, String>,
    unit: &'a str,
    timeout_ms: u64,
    output_limit: usize,
}
fn request(
    program: PathBuf,
    args: Vec<OsString>,
    timeout_ms: u64,
    output: usize,
    input: Input,
) -> Result<RunRequest, String> {
    Ok(RunRequest {
        command: CommandSpec {
            program,
            args,
            current_dir: PathBuf::from("/"),
            environment_mode: EnvironmentMode::Clear,
            environment: vec![],
            input,
        },
        limits: ExecutionLimits {
            timeout_ms,
            stdin_max_bytes: 1_048_576,
            stdout_max_bytes: output,
            stderr_max_bytes: output,
            poll_interval_ms: 10,
            teardown_timeout_ms: 2000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true).map_err(|e| format!("{e:?}"))?,
    })
}
pub(crate) fn manager_request(
    config: &Config,
    program: PathBuf,
    args: Vec<OsString>,
    timeout_ms: u64,
    output: usize,
    input: Input,
) -> Result<RunRequest, String> {
    let mut run = request(program, args, timeout_ms, output, input)?;
    run.command.environment.push((
        "XDG_RUNTIME_DIR".into(),
        config.manager_runtime_directory.as_os_str().to_owned(),
    ));
    Ok(run)
}
/// Reviewed systemd unit properties. `collect` marks the VMM unit, which also
/// receives `NoNewPrivileges=yes`; customer step units do not.
fn unit_properties(config: &Config, timeout_ms: u64, collect: bool) -> Vec<String> {
    let mut properties = vec![
        format!("MemoryMax={}", config.memory_bytes),
        "MemorySwapMax=0".into(),
        format!("TasksMax={}", config.processes),
        format!("CPUQuota={}%", config.cpu_percent),
        format!("RuntimeMaxSec={}ms", timeout_ms.saturating_add(1000)),
        "KillMode=control-group".into(),
        "SendSIGKILL=yes".into(),
        "TimeoutStopSec=2s".into(),
    ];
    if collect {
        properties.push("NoNewPrivileges=yes".into());
    }
    properties
}

/// `systemd-run` arguments placing one process tree in the attempt unit with the
/// reviewed cgroup limits, followed by `env -i`. `collect` unloads the unit even
/// after failure so a later process in the same attempt can reuse its name.
pub(crate) fn unit_command(
    config: &Config,
    unit: &str,
    timeout_ms: u64,
    collect: bool,
) -> Result<Vec<OsString>, String> {
    let mut service = vec![
        OsString::from("--user"),
        OsString::from("--wait"),
        OsString::from("--pipe"),
        OsString::from(format!("--unit={unit}")),
    ];
    if collect {
        service.push(OsString::from("--collect"));
    }
    for property in unit_properties(config, timeout_ms, collect) {
        service.push(OsString::from(format!("--property={property}")));
    }
    service.push(config.env.verify()?.into_os_string());
    service.push(OsString::from("-i"));
    Ok(service)
}
pub(crate) fn systemctl(
    config: &Config,
    args: &[&str],
) -> Result<bounded_exec::ExecutionOutput, String> {
    bounded_exec::run(manager_request(
        config,
        config.systemctl.verify()?,
        std::iter::once(OsString::from("--user"))
            .chain(args.iter().map(OsString::from))
            .collect(),
        5000,
        16384,
        Input::Null,
    )?)
    .map_err(|e| e.to_string())
}
/// Stop is owned and bounded. Missing, inactive, or failed units with no cgroup
/// are authoritative systemd cleanup observations; any ambiguity retains a slot.
pub(crate) fn cleanup_unit(config: &Config, unit: &str) -> bool {
    let stopped = systemctl(config, &["stop", unit]);
    if stopped.as_ref().is_err() {
        return false;
    }
    let Ok(output) = systemctl(
        config,
        &[
            "show",
            unit,
            "--property=LoadState",
            "--property=ActiveState",
            "--property=ControlGroup",
        ],
    ) else {
        return false;
    };
    if output.completion != Completion::Exited || output.stdout.truncated {
        return false;
    }
    let text = String::from_utf8_lossy(&output.stdout.bytes);
    let properties = text
        .lines()
        .filter_map(|l| l.split_once('='))
        .collect::<BTreeMap<_, _>>();
    properties.get("ControlGroup") == Some(&"")
        && (matches!(properties.get("ActiveState"), Some(&"inactive" | &"failed"))
            || properties.get("LoadState") == Some(&"not-found"))
}
fn run_step(
    config: &Config,
    request: StepRequest<'_>,
    cancellation: &AtomicBool,
) -> Result<StepObservation, String> {
    let StepRequest {
        backend,
        workspace,
        step,
        environment,
        unit,
        timeout_ms,
        output_limit,
    } = request;
    let root = tempfile::Builder::new()
        .prefix("kiln-hosted-root-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let mut service = unit_command(config, unit, timeout_ms, false)?;
    let input = match backend {
        Backend::Namespaces {
            unshare,
            python,
            mount,
            shell,
            runtime_roots,
            network: _,
        } => {
            let shell = shell.verify()?;
            let data = json!({"root":root.path(),"workspace":workspace,"mount":mount.verify()?,"runtime_roots":runtime_roots,"shell":shell,"run":step.run,"env":environment,"cwd":step.working_directory,"bash":matches!(step.shell,crate::pipelines::Shell::Bash)});
            let payload = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
            if payload.len() > 1_048_576 {
                return Err("namespace request exceeds protocol bound".into());
            }
            let payload_path = root.path().join("namespace-request.json");
            std::fs::write(&payload_path, payload).map_err(|e| e.to_string())?;
            service.extend([
                unshare.verify()?.into_os_string(),
                "--user".into(),
                "--map-root-user".into(),
                "--mount".into(),
                "--pid".into(),
                "--fork".into(),
                "--kill-child=KILL".into(),
                "--net".into(),
                "--ipc".into(),
                "--uts".into(),
                python.verify()?.into_os_string(),
                "-I".into(),
                "-S".into(),
                "-c".into(),
                NAMESPACE_INIT.into(),
                payload_path.into_os_string(),
            ]);
            Input::Null
        }
        Backend::Qemu {
            qemu,
            kernel,
            initramfs,
            root_image,
            guest_shell,
            acceleration,
            ..
        } => {
            service.push(qemu.verify()?.into_os_string());
            let mut args = vec![
                "-nodefaults".to_string(),
                "-no-user-config".into(),
                "-nographic".into(),
                "-no-reboot".into(),
                "-monitor".into(),
                "none".into(),
                "-serial".into(),
                "stdio".into(),
                "-m".into(),
                (config.memory_bytes / 1_048_576)
                    .saturating_sub(32)
                    .max(32)
                    .to_string(),
                "-smp".into(),
                "1".into(),
                "-accel".into(),
                acceleration.clone(),
                "-sandbox".into(),
                "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny".into(),
                "-kernel".into(),
                kernel.verify()?.to_string_lossy().into_owned(),
                "-initrd".into(),
                initramfs.verify()?.to_string_lossy().into_owned(),
                "-append".into(),
                "console=ttyS0 panic=-1 rdinit=/init quiet".into(),
                "-virtfs".into(),
                format!(
                    "local,path={},mount_tag=kilnsource,security_model=mapped-xattr",
                    workspace
                        .to_str()
                        .filter(|p| !p.contains(','))
                        .ok_or("VM workspace path encoding denied")?
                ),
            ];
            if let Some(image) = root_image {
                let path = image.verify()?;
                if path.to_string_lossy().contains(',') {
                    return Err("VM image path invalid".into());
                }
                args.extend([
                    "-drive".into(),
                    format!("file={},format=raw,if=virtio,readonly=on", path.display()),
                ]);
            }
            let payload=serde_json::to_vec(&json!({"schema":"kiln.hosted.guest.v1","shell":guest_shell,"run":step.run,"env":environment,"cwd":step.working_directory,"bash":matches!(step.shell,crate::pipelines::Shell::Bash)})).map_err(|e|e.to_string())?;
            if payload.len() > 1_048_576 {
                return Err("VM request exceeds protocol bound".into());
            }
            let payload_path = root.path().join("guest-request.json");
            std::fs::write(&payload_path, payload).map_err(|e| e.to_string())?;
            args.extend([
                "-fw_cfg".into(),
                format!("name=opt/kiln/request,file={}", payload_path.display()),
            ]);
            service.extend(args.into_iter().map(OsString::from));
            Input::Null
        }
        Backend::Firecracker { .. } | Backend::CloudHypervisor { .. } => {
            return Err("microVM runners execute whole jobs, not single steps".into());
        }
    };
    let started = Instant::now();
    let observed = bounded_exec::run_with_cancellation(
        manager_request(
            config,
            config.systemd_run.verify()?,
            service,
            timeout_ms,
            output_limit,
            input,
        )?,
        cancellation,
    );
    let cleaned = cleanup_unit(config, unit);
    // The private root is safe to remove only after the owning unit is gone.
    if !cleaned {
        let _retained = root.keep();
    }
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    match observed {
        Ok(output) => {
            let cancelled = output.completion == Completion::Cancelled;
            let stdout = String::from_utf8_lossy(&output.stdout.bytes).into_owned();
            let mut guest = None;
            let mut qemu_success = true;
            if matches!(backend, Backend::Qemu { .. }) {
                guest = stdout
                    .lines()
                    .filter_map(|line| line.strip_prefix("KILN_RESULT:"))
                    .next_back()
                    .and_then(|line| serde_json::from_str::<Value>(line).ok());
                qemu_success = guest.as_ref().is_some_and(|r| {
                    r.get("schema") == Some(&json!("kiln.hosted.guest.v1"))
                        && r.get("exit_code") == Some(&json!(0))
                });
            }
            Ok(step_observation(
                ProcessFacts {
                    name: &step.name,
                    exited: output.completion == Completion::Exited,
                    timed_out: output.completion == Completion::TimedOut,
                    cancelled,
                    exit_code: output.exit_code,
                    signal: output.signal,
                    stdout,
                    stderr: String::from_utf8_lossy(&output.stderr.bytes).into_owned(),
                    stdout_truncated: output.stdout.truncated,
                    stderr_truncated: output.stderr.truncated,
                    elapsed_ms,
                    cleaned,
                },
                qemu_success,
                guest,
            ))
        }
        Err(error) => {
            let (state, cancelled) =
                infrastructure_failure_state(cancellation.load(Ordering::Acquire), cleaned);
            Ok(StepObservation {
                success: false,
                cancelled,
                cleaned,
                value: json!({"name":step.name,"state":state,"reason":error.to_string(),"cleaned":cleaned,"elapsed_ms":elapsed_ms}),
            })
        }
    }
}

/// Embedded trusted namespace bootstrap. It executes before chroot, mounts only
/// reviewed runtime roots and customer source, clears all env, then drops every
/// capability before exec. The isolated PID-namespace init dies with its parent.
const NAMESPACE_INIT: &str = r#"
import os,sys,json,subprocess,ctypes
p=json.load(open(sys.argv[1])); root=p['root']; mount=p['mount']
def m(*args): subprocess.run([mount,*args],check=True,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL)
m('--make-rprivate','/')
m('-t','tmpfs','-o','mode=0700,size=32m','tmpfs',root)
for source in p['runtime_roots']:
 target=root+source; os.makedirs(target,exist_ok=True);m('--rbind',source,target);m('-o','remount,bind,ro',target)
for target in ['/work','/proc','/dev','/tmp','/bin']:os.makedirs(root+target,exist_ok=True)
m('--bind',p['workspace'],root+'/work')
m('-t','proc','-o','nosuid,nodev,noexec','proc',root+'/proc')
for name in ['null','zero','random','urandom']:
 open(root+'/dev/'+name,'w').close();m('--bind','/dev/'+name,root+'/dev/'+name)
if not os.path.lexists(root+'/bin/sh'):os.symlink(p['shell'],root+'/bin/sh')
libc=ctypes.CDLL(None,use_errno=True)
if libc.prctl(38,1,0,0,0)!=0:raise OSError('no_new_privs failed')
for cap in range(41):libc.prctl(24,cap,0,0,0)
class Header(ctypes.Structure):_fields_=[('version',ctypes.c_uint32),('pid',ctypes.c_int)]
class Caps(ctypes.Structure):_fields_=[('effective',ctypes.c_uint32),('permitted',ctypes.c_uint32),('inheritable',ctypes.c_uint32)]
os.chroot(root)
cwd=os.path.normpath('/work/'+p['cwd']);
if cwd!='/work' and not cwd.startswith('/work/'):raise ValueError('working directory escapes source')
os.chdir(cwd)
header=Header(0x20080522,0);caps=(Caps*2)()
if libc.capset(ctypes.byref(header),ctypes.byref(caps))!=0:raise OSError('capability drop failed')
args=[p['shell']]+(['--noprofile','--norc','-e','-o','pipefail'] if p['bash'] else ['-e'])+['-c',p['run']]
env={'PATH':os.path.dirname(p['shell'])+':/bin','HOME':'/tmp','TMPDIR':'/tmp','LANG':'C.UTF-8'}
env.update(p['env']);os.execve(p['shell'],args,env)
"#;

/// Reference guest /init payload for an operator-built initramfs. Requires
/// Python 3 as /bin/python3, mount, stty and poweroff in that image, plus kernel
/// virtio-9p/virtio-pci/serial/fw_cfg-sysfs support. Customer workspace uses 9p
/// mapped-xattr so successful writes persist across steps; no other host path
/// enters the guest. Optional raw filesystem root image is mounted read-only
/// at /runtime. The image is separately hash admitted.
/// Guest logs/results are observations, never trusted billing/cleanup evidence.
pub const QEMU_GUEST_INIT: &str = r#"#!/bin/python3
import os,sys,json,subprocess,shutil
os.makedirs('/proc',exist_ok=True);os.makedirs('/sys',exist_ok=True);os.makedirs('/dev',exist_ok=True);os.makedirs('/work',exist_ok=True);os.makedirs('/runtime',exist_ok=True);os.makedirs('/tmp',exist_ok=True)
subprocess.run(['/bin/mount','-t','proc','proc','/proc'],check=True)
subprocess.run(['/bin/mount','-t','devtmpfs','devtmpfs','/dev'],check=True)
serial=os.open('/dev/ttyS0',os.O_RDWR)
for fd in [0,1,2]:os.dup2(serial,fd)
subprocess.run(['/bin/stty','-F','/dev/ttyS0','raw','-echo'],check=True)
subprocess.run(['/bin/mount','-t','sysfs','sysfs','/sys'],check=True)
subprocess.run(['/bin/mount','-t','9p','-o','trans=virtio,version=9p2000.L','kilnsource','/work'],check=True)
if os.path.exists('/dev/vda'):subprocess.run(['/bin/mount','-o','ro','/dev/vda','/runtime'],check=True)
with open('/sys/firmware/qemu_fw_cfg/by_name/opt/kiln/request/raw','rb') as request:line=request.read(1048577)
try:
 if len(line)>1048576:raise ValueError('oversized request')
 p=json.loads(line)
 if p['schema']!='kiln.hosted.guest.v1':raise ValueError('protocol mismatch')
 cwd=os.path.normpath('/work/'+p['cwd'])
 if cwd!='/work' and not cwd.startswith('/work/'):raise ValueError('cwd escapes source')
 args=[p['shell']]+(['--noprofile','--norc','-e','-o','pipefail'] if p['bash'] else ['-e'])+['-c',p['run']]
 result=subprocess.run(args,cwd=cwd,env=dict({'PATH':'/bin:/usr/bin','HOME':'/tmp','TMPDIR':'/tmp'},**p['env']))
 code=result.returncode
except Exception as error:
 print('guest admission failed: '+str(error),flush=True);code=125
print('\nKILN_RESULT:'+json.dumps({'schema':'kiln.hosted.guest.v1','exit_code':code}),flush=True)
subprocess.run(['/bin/poweroff','-f'])
"#;

pub fn handle(
    db: &mut Connection,
    actor: &crate::Actor,
    action: &str,
    input: Value,
    now_ms: u64,
) -> Result<Value, String> {
    match action {
        "list" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct List {
                project: String,
                #[serde(default)]
                after: String,
            }
            let input: List =
                serde_json::from_value(input).map_err(|_| "execution list request rejected")?;
            crate::accounts::require_project(
                db,
                actor,
                &input.project,
                &["owner", "admin", "member", "viewer", "billing"],
            )?;
            if input.after.len() > 128 {
                return Err("execution cursor exceeds bound".into());
            }
            let mut statement=db.prepare("SELECT attempt,pipeline,state,reserved_ms FROM hosted_execution WHERE org=?1 AND project=?2 AND attempt>?3 ORDER BY attempt LIMIT 101").map_err(|_|"execution list unavailable")?;
            let mut rows=statement.query_map(params![actor.organization,input.project,input.after],|row|Ok(json!({"attempt_id":row.get::<_,String>(0)?,"pipeline":row.get::<_,String>(1)?,"state":row.get::<_,String>(2)?,"reserved_ms":crate::sqlite_u64(row,3)?}))).map_err(|_|"execution list unavailable")?.collect::<Result<Vec<_>,_>>().map_err(|_|"execution list unavailable")?;
            let has_more = rows.len() > 100;
            rows.truncate(100);
            let next = if has_more {
                rows.last().and_then(|row| row.get("attempt_id")).cloned()
            } else {
                None
            };
            Ok(json!({"attempts":rows,"next_after":next}))
        }
        "debug_issue" | "debug_exec" | "debug_release" | "debug_list" => {
            crate::debug_hold::handle(db, actor, action, input)
        }
        "run" => run_pipeline(
            db,
            actor,
            serde_json::from_value(input).map_err(|e| e.to_string())?,
            now_ms,
            &AtomicBool::new(false),
        ),
        "get" | "recover" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Lookup {
                project: String,
                attempt_id: String,
            }
            let lookup: Lookup = serde_json::from_value(input).map_err(|e| e.to_string())?;
            let roles: &[&str] = if action == "recover" {
                &["owner", "admin"]
            } else {
                &["owner", "admin", "member", "viewer", "billing"]
            };
            crate::accounts::require_project(db, actor, &lookup.project, roles)?;
            let (state,unit,result):(String,String,Option<String>)=db.query_row("SELECT state,unit,result FROM hosted_execution WHERE org=?1 AND project=?2 AND attempt=?3",params![actor.organization,lookup.project,lookup.attempt_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|"execution attempt absent")?;
            if action == "get" {
                let mut got = json!({"attempt_id":lookup.attempt_id,"state":state,"result":result.and_then(|r|serde_json::from_str::<Value>(&r).ok())});
                // r[impl kiln.hosted.microvm.debug_hold.sessions]
                // While a job is held, the attempt's log ends with how to connect.
                let holds = crate::debug_hold::active(db, &actor.organization, &lookup.attempt_id)?;
                if let (false, Some(fields)) = (holds.is_empty(), got.as_object_mut()) {
                    fields.insert("debug_holds".into(), json!(holds));
                }
                return Ok(got);
            }
            if !["reserved", "running", "unknown"].contains(&state.as_str()) {
                return Err("no uncertain execution to recover".into());
            }
            let (_, config) = configuration(db)?;
            let cleaned = cleanup_unit(&config, &unit);
            if cleaned {
                db.execute("UPDATE hosted_execution SET state='reaped_unknown' WHERE org=?1 AND project=?2 AND attempt=?3",params![actor.organization,lookup.project,lookup.attempt_id]).map_err(|e|e.to_string())?;
                // The owned unit held every VM of this attempt; its empty cgroup
                // retires any VM whose destruction was not yet observed.
                db.execute("UPDATE hosted_microvm_job SET lifecycle='reaped' WHERE org=?1 AND attempt=?2 AND lifecycle<>'destroyed'",params![actor.organization,lookup.attempt_id]).map_err(|e|e.to_string())?;
                crate::debug_hold::end_attempt(db, &actor.organization, &lookup.attempt_id)?;
            }
            Ok(
                json!({"attempt_id":lookup.attempt_id,"cleaned":cleaned,"state":if cleaned {"reaped_unknown"} else {"unknown"},"financial_obligation":"reserved","rerun":false}),
            )
        }
        _ => Err("unknown execution action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // r[verify kiln.hosted.execution.scenario.teardown]
    #[test]
    fn cancellation_does_not_turn_uncertain_teardown_into_safe_completion() {
        assert_eq!(
            infrastructure_failure_state(true, true),
            ("cancelled", true)
        );
        assert_eq!(infrastructure_failure_state(false, true), ("failed", false));
        assert_eq!(
            infrastructure_failure_state(true, false),
            ("unknown", false)
        );
        assert_eq!(
            infrastructure_failure_state(false, false),
            ("unknown", false)
        );
    }
    #[test]
    fn financial_admission_rounds_up_and_enforces_operator_maximum() {
        assert_eq!(reservation_units(1000, 1000, 2), Ok(1));
        assert_eq!(reservation_units(1001, 1000, 2), Ok(2));
        assert!(reservation_units(2001, 1000, 2).is_err());
        assert!(reservation_units(1, 0, 2).is_err());
        assert_eq!(reservation_units(u64::MAX, 2, u64::MAX), Ok(1_u64 << 63));
    }
    // r[verify kiln.hosted.execution.scenario.teardown r[verify kiln.hosted.qualification.scenario.nonclaims]]
    #[test]
    fn namespaces_never_admit_hostile_jobs() {
        let e = Executable {
            path: "/missing".into(),
            blake3: "0".repeat(64),
        };
        let backend = Backend::Namespaces {
            unshare: e.clone(),
            python: e.clone(),
            mount: e.clone(),
            shell: e,
            runtime_roots: vec!["/nix/store".into()],
            network: None,
        };
        assert!(admit_backend(&backend, "trusted").is_ok());
        assert!(
            admit_backend(&backend, "hostile")
                .err()
                .is_some_and(|error| error.contains("qualified VM"))
        );
        assert!(admit_backend(&backend, "unknown").is_err());
    }
    // r[verify kiln.hosted.execution.scenario.teardown]
    #[test]
    fn uncertain_states_retain_capacity_until_owned_reaping() {
        let db = Connection::open_in_memory()
            .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        init(&db).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        for (attempt, state) in [
            ("a", "unknown"),
            ("b", "running"),
            ("c", "succeeded"),
            ("d", "reaped_unknown"),
        ] {
            db.execute("INSERT INTO hosted_execution VALUES('org','project',?1,'pipeline','id','config','unit',?2,0,NULL)",params![attempt,state]).unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        }
        assert!(check_capacity(&db, 2).is_err());
        assert!(check_capacity(&db, 3).is_ok());
        db.execute(
            "UPDATE hosted_execution SET state='reaped_unknown' WHERE attempt='a'",
            [],
        )
        .unwrap_or_else(|error| unreachable!("fixture operation succeeds: {error:?}"));
        assert!(check_capacity(&db, 2).is_ok());
    }

    fn executable() -> Value {
        json!({"path": "/nix/store/missing/bin/tool", "blake3": "0".repeat(64)})
    }

    fn firecracker_backend(extra: Value) -> Value {
        let mut backend = json!({
            "kind": "firecracker", "firecracker": executable(), "kernel": executable(),
            "initrd": executable(), "guest_shell": "/bin/bash", "vcpus": 1, "memory_mib": 256,
            "boot_timeout_ms": 10_000, "source_image_mib": 64, "warm_pool": null,
            "qualification_identity": null, "trust_domain": "trusted",
            "host_identity": "host-trusted",
        });
        if let (Some(fields), Some(extra)) = (backend.as_object_mut(), extra.as_object()) {
            fields.extend(extra.clone());
        }
        backend
    }

    fn firecracker_config(backend: Value, cache_runner: Option<&str>) -> Config {
        let cache = cache_runner.map(|runner| json!({
            "directory": "/srv/kiln/cache", "maximum_entry_bytes": 1024, "maximum_total_bytes": 4096,
            "maximum_entries": 4, "maximum_age_seconds": 60, "maximum_files": 4,
            "policies": [{"organization": "acme", "project": "app", "runner": runner, "worker": {
                "producer": "hosted", "toolchain": "t", "platform": "linux", "inputs": ["Cargo.lock"],
                "paths": ["target"], "maximum_entry_bytes": 1024, "maximum_files": 4}}],
        }));
        serde_json::from_value(json!({
            "systemd_run": executable(), "systemctl": executable(),
            "manager_runtime_directory": "/run/user/1000", "env": executable(),
            "work_root": "/srv/kiln/work", "cache": cache,
            "runners": {"vm": backend},
            "maximum_concurrent": 2, "memory_bytes": 512 * 1_048_576_u64, "processes": 16,
            "cpu_percent": 100, "maximum_timeout_ms": 60_000, "maximum_output_bytes": 65_536,
        }))
        .unwrap_or_else(|error| unreachable!("fixture configuration parses: {error}"))
    }

    fn admit(config: &Config) -> Result<(), String> {
        let backend = config.runners.get("vm").unwrap_or_else(|| unreachable!());
        admit_firecracker(config, "vm", backend)
    }

    // r[verify kiln.microvm.confinement.observation]
    #[test]
    fn vmm_unit_sets_no_new_privileges() {
        let config = firecracker_config(firecracker_backend(json!({})), None);
        assert!(unit_properties(&config, 1_000, true).contains(&"NoNewPrivileges=yes".into()));
        assert!(!unit_properties(&config, 1_000, false).contains(&"NoNewPrivileges=yes".into()));
    }

    // r[verify kiln.hosted.microvm.admission]
    // r[verify kiln.hosted.microvm.admission.scenario.bounds]
    #[test]
    fn firecracker_runner_admission_is_bounded_by_unit_limits_cache_and_pool() {
        assert_eq!(
            admit(&firecracker_config(firecracker_backend(json!({})), None)),
            Ok(())
        );
        let pooled = json!({"warm_pool": {"directory": "/srv/kiln/pool", "slots": 2}});
        assert_eq!(
            admit(&firecracker_config(firecracker_backend(pooled), None)),
            Ok(())
        );
        for rejected in [
            json!({"memory_mib": 512}),
            json!({"vcpus": 13}),
            json!({"boot_timeout_ms": 60_000}),
            json!({"warm_pool": {"directory": "/srv/kiln/pool", "slots": 3}}),
            json!({"warm_pool": {"directory": "/srv/kiln/work/pool", "slots": 1}}),
            json!({"qualification_identity": "not-a-blake3"}),
        ] {
            let config = firecracker_config(firecracker_backend(rejected.clone()), None);
            assert!(admit(&config).is_err(), "{rejected}");
        }
        let cached = firecracker_config(firecracker_backend(json!({})), Some("vm"));
        assert_eq!(admit(&cached), Ok(()));
        let unknown = serde_json::from_value::<Backend>(firecracker_backend(
            json!({"network_interfaces": []}),
        ));
        assert!(unknown.is_err(), "unknown backend fields are rejected");
    }

    // r[verify kiln.hosted.microvm.debug_hold.policy]
    // r[verify kiln.hosted.microvm.debug_hold.policy.scenario.bounds]
    // r[verify kiln.hosted.microvm.debug_hold.policy.scenario.disabled]
    #[test]
    fn debug_hold_is_bounded_and_absent_keeps_the_configuration_identity() {
        let plain = firecracker_config(firecracker_backend(json!({})), None);
        let reserialized: Config = serde_json::from_value(
            serde_json::to_value(&plain).unwrap_or_else(|error| unreachable!("{error}")),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        let identity = crate::identity(&plain).unwrap_or_default();
        assert_eq!(crate::identity(&reserialized).unwrap_or_default(), identity);
        assert!(
            !serde_json::to_string(&plain)
                .unwrap_or_default()
                .contains("debug_hold"),
            "an absent option is never serialized"
        );
        let enabled =
            json!({"debug_hold": {"enabled": true, "grace_seconds": 300, "max_concurrent": 2}});
        let held = firecracker_config(firecracker_backend(enabled), None);
        assert_eq!(admit(&held), Ok(()));
        assert_ne!(crate::identity(&held).unwrap_or_default(), identity);
        for rejected in [
            json!({"debug_hold": {"enabled": true, "grace_seconds": 0, "max_concurrent": 1}}),
            json!({"debug_hold": {"enabled": true, "grace_seconds": 901, "max_concurrent": 1}}),
            json!({"debug_hold": {"enabled": true, "grace_seconds": 60, "max_concurrent": 0}}),
            json!({"debug_hold": {"enabled": true, "grace_seconds": 60, "max_concurrent": 3}}),
            json!({"debug_hold": {"enabled": false, "grace_seconds": 901, "max_concurrent": 1}}),
        ] {
            let config = firecracker_config(firecracker_backend(rejected.clone()), None);
            assert!(admit(&config).is_err(), "{rejected}");
        }
        let override_field = serde_json::from_value::<Backend>(firecracker_backend(json!({
            "debug_hold": {"enabled": true, "grace_seconds": 60, "max_concurrent": 1, "allow_secrets": true}
        })));
        assert!(override_field.is_err(), "no override field exists");
        // Hold time is reserved only when a hold is possible at all.
        let policy = debug_hold_policy(held.runners.get("vm").unwrap_or_else(|| unreachable!()));
        let request: PipelineRequest = serde_json::from_value(json!({
            "project": "app", "pipeline_id": "p", "attempt_id": "a", "price_id": "x",
            "class": "c", "workspace": "w", "trust": "trusted",
        }))
        .unwrap_or_else(|error| unreachable!("{error}"));
        let grants = hold_grants(&held, "acme", &request, "vm");
        assert_eq!(
            kiln_microvm::debug_hold::reserve_ms(policy, grants),
            300_000
        );
        let hostile = PipelineRequest {
            trust: "hostile".into(),
            ..request
        };
        let grants = hold_grants(&held, "acme", &hostile, "vm");
        assert!(grants.untrusted);
        assert_eq!(kiln_microvm::debug_hold::reserve_ms(policy, grants), 0);
    }

    // r[verify kiln.hosted.qualification.scenario.nonclaims]
    #[test]
    fn firecracker_admits_hostile_only_with_an_operator_qualification_reference() {
        let unqualified: Backend = serde_json::from_value(firecracker_backend(json!({})))
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(admit_backend(&unqualified, "trusted").is_ok());
        assert!(admit_backend(&unqualified, "hostile").is_err());
        let qualified: Backend = serde_json::from_value(firecracker_backend(
            json!({"qualification_identity": "a".repeat(64)}),
        ))
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(admit_backend(&qualified, "hostile").is_ok());
    }

    fn untrusted_request(attempt: &str) -> PipelineRequest {
        serde_json::from_value(json!({
            "project": "app", "pipeline_id": "ci", "attempt_id": attempt, "price_id": "standard",
            "class": "small", "workspace": "pr-7", "trust": "hostile",
            "untrusted": {"reason": "fork", "source_commit": "1".repeat(40), "approver_id": "local:operator"},
        }))
        .unwrap_or_else(|error| unreachable!("fixture request parses: {error}"))
    }

    // r[verify kiln.untrusted.execution]
    // r[verify kiln.untrusted.execution.scenario.retry]
    #[test]
    fn untrusted_attempts_and_retries_get_no_secrets_token_or_cache() {
        let mut config = firecracker_config(
            firecracker_backend(json!({"untrusted_capable": true})),
            Some("vm"),
        );
        config.workload = vec![WorkloadPolicy {
            organization: "acme".into(),
            project: "app".into(),
            runner: "vm".into(),
            audience: Some("https://registry.example".into()),
            secrets: vec![SecretEnvironment {
                environment: "DEPLOY_TOKEN".into(),
                name: "deploy".into(),
                revision: 1,
            }],
        }];
        // Trusted jobs keep the reviewed workload grant (secrets, audience).
        let trusted = job_grants(&config, "acme", "app", "vm", false);
        assert!(
            trusted
                .workload
                .is_some_and(|p| p.audience.is_some() && !p.secrets.is_empty())
        );
        assert!(trusted.cache);
        // The approved run and its retry attempt: no workload policy means the
        // credential step resolves no secret and mints no KILN_WORKLOAD_TOKEN,
        // and no cache restore runs, so no publication receipt can exist.
        for attempt in ["approved-1", "approved-1-retry-1"] {
            let request = untrusted_request(attempt);
            assert!(untrusted_run(&request));
            let grants = job_grants(&config, "acme", "app", "vm", request.untrusted.is_some());
            assert!(grants.workload.is_none(), "{attempt}");
            assert!(!grants.cache, "{attempt}");
            let hold = hold_grants(&config, "acme", &request, "vm");
            assert!(hold.untrusted);
        }
    }

    // r[verify kiln.untrusted.execution]
    #[test]
    fn untrusted_runs_need_a_declared_untrusted_capable_microvm_runner() {
        let request = untrusted_request("approved-1");
        let provenance = request.untrusted.as_ref();
        let declared: Backend =
            serde_json::from_value(firecracker_backend(json!({"untrusted_capable": true})))
                .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(admit_untrusted_runner(&declared, provenance).is_ok());
        let undeclared: Backend = serde_json::from_value(firecracker_backend(json!({})))
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(admit_untrusted_runner(&undeclared, provenance).is_err());
        // Trusted requests are unaffected by the declaration.
        assert!(admit_untrusted_runner(&undeclared, None).is_ok());
        let safe = kiln_microvm::host::HostFacts::observed(
            "6.18.56",
            "",
            &std::collections::BTreeMap::new(),
            false,
            false,
            Some(true),
            Some(false),
            Some(0),
            true,
        );
        assert!(admit_untrusted_host(&declared, &safe).is_ok());
        let mut unsafe_host = safe.clone();
        unsafe_host.smt_active = true;
        assert!(admit_untrusted_host(&declared, &unsafe_host).is_err());
        let overridden: Backend = serde_json::from_value(firecracker_backend(json!({
            "untrusted_capable": true,
            "host_preflight_override": "operator accepted test host"
        })))
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(admit_untrusted_host(&overridden, &unsafe_host).is_ok());
        let e = Executable {
            path: "/missing".into(),
            blake3: "0".repeat(64),
        };
        let namespaces = Backend::Namespaces {
            unshare: e.clone(),
            python: e.clone(),
            mount: e.clone(),
            shell: e,
            runtime_roots: vec!["/nix/store".into()],
            network: None,
        };
        assert!(admit_untrusted_runner(&namespaces, provenance).is_err());
        let mut forged = untrusted_request("approved-1");
        if let Some(untrusted) = forged.untrusted.as_mut() {
            untrusted.source_commit = "HEAD".into();
        }
        assert!(admit_untrusted_runner(&declared, forged.untrusted.as_ref()).is_err());
        // The declaration is omitted from configuration bytes when absent.
        let encoded = serde_json::to_value(&undeclared).unwrap_or_default();
        assert!(encoded.get("untrusted_capable").is_none());
        let trusted: PipelineRequest = serde_json::from_value(json!({
            "project": "app", "pipeline_id": "ci", "attempt_id": "a", "price_id": "standard",
            "class": "small", "workspace": "w", "trust": "trusted",
        }))
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(!untrusted_run(&trusted));
        assert_eq!(
            serde_json::to_string(&trusted).unwrap_or_default(),
            r#"{"project":"app","pipeline_id":"ci","attempt_id":"a","price_id":"standard","class":"small","workspace":"w","trust":"trusted"}"#
        );
    }

    fn namespace_step(name: &str, exit_code: i32, stdout: &str, stderr: &str) -> StepObservation {
        // The facts run_step derives from a finished `systemd-run` process.
        step_observation(
            ProcessFacts {
                name,
                exited: true,
                timed_out: false,
                cancelled: false,
                exit_code: Some(exit_code),
                signal: None,
                stdout: stdout.into(),
                stderr: stderr.into(),
                stdout_truncated: false,
                stderr_truncated: false,
                elapsed_ms: 41,
                cleaned: true,
            },
            true,
            None,
        )
    }

    fn receipt(steps: Vec<StepObservation>, state: &str, extra: Value) -> Value {
        let steps: Vec<Value> = steps.into_iter().map(|step| step.value).collect();
        json!({"attempt_id": "a", "pipeline": "p", "state": state, "elapsed_ms": 999,
            "jobs": [{"job": "build", "state": state, "steps": steps, "cache": {"outcome": "disabled_unconfigured"}, "microvm": extra},
                     {"job": "after", "state": "skipped_dependency"}]})
    }

    // r[verify kiln.hosted.microvm.equivalence]
    #[test]
    fn namespace_and_microvm_observations_share_one_canonical_outcome() {
        use kiln_microvm::protocol::StepReport;
        let namespace = receipt(
            vec![
                namespace_step(
                    "read",
                    0,
                    "readme=hello\n",
                    "Running as unit: kiln-hosted-1.service\n",
                ),
                namespace_step("fail", 3, "before-failure\n", ""),
            ],
            "failed",
            Value::Null,
        );
        let reports = [
            StepReport {
                index: 0,
                exit_code: Some(0),
                signal: None,
                timed_out: false,
                stdout: "readme=hello\n".into(),
                stderr: String::new(),
                stdout_truncated: false,
                stderr_truncated: false,
                elapsed_ms: 7,
            },
            StepReport {
                index: 1,
                exit_code: Some(3),
                signal: None,
                timed_out: false,
                stdout: "before-failure\n".into(),
                stderr: String::new(),
                stdout_truncated: false,
                stderr_truncated: false,
                elapsed_ms: 2,
            },
        ];
        let guest_steps: Vec<(String, kiln_microvm::protocol::GuestStep)> = ["read", "fail"]
            .iter()
            .map(|name| {
                let step = kiln_microvm::protocol::guest_step(
                    "/bin/bash",
                    true,
                    "true",
                    ".",
                    &BTreeMap::new(),
                    1024,
                )
                .unwrap_or_else(|error| unreachable!("{error}"));
                ((*name).to_owned(), step)
            })
            .collect();
        let microvm_steps =
            crate::microvm::observations_for_reports(&guest_steps, &reports, None, true, false);
        let microvm = receipt(
            microvm_steps,
            "failed",
            json!({"boot": "restore", "ready_ms": 56}),
        );
        let identity =
            canonical_outcome(&namespace).unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(canonical_outcome(&microvm), Ok(identity.clone()));

        let changed = receipt(
            vec![
                namespace_step("read", 0, "readme=other\n", ""),
                namespace_step("fail", 3, "before-failure\n", ""),
            ],
            "failed",
            Value::Null,
        );
        assert_ne!(canonical_outcome(&changed), Ok(identity.clone()));
        let exit = receipt(
            vec![
                namespace_step("read", 0, "readme=hello\n", ""),
                namespace_step("fail", 4, "before-failure\n", ""),
            ],
            "failed",
            Value::Null,
        );
        assert_ne!(canonical_outcome(&exit), Ok(identity.clone()));
        let interrupted = crate::microvm::observations_for_reports(
            &guest_steps,
            reports.get(..1).unwrap_or_default(),
            Some("guest boot deadline elapsed"),
            true,
            false,
        );
        assert_ne!(
            canonical_outcome(&receipt(interrupted, "failed", Value::Null)),
            Ok(identity)
        );
    }
    #[test]
    fn untrusted_and_workload_runners_cannot_share_a_trust_domain() {
        let untrusted: Backend = serde_json::from_value(firecracker_backend(json!({
            "untrusted_capable": true,
            "trust_domain": "shared",
            "host_identity": "host-shared"
        })))
        .unwrap_or_else(|error| unreachable!("{error}"));
        let trusted: Backend = serde_json::from_value(firecracker_backend(json!({
            "trust_domain": "shared",
            "host_identity": "host-shared"
        })))
        .unwrap_or_else(|error| unreachable!("{error}"));
        let runners = std::collections::BTreeMap::from([
            ("untrusted".to_string(), untrusted.clone()),
            ("trusted".to_string(), trusted.clone()),
        ]);
        let workload = vec![WorkloadPolicy {
            organization: "acme".into(),
            project: "app".into(),
            runner: "trusted".into(),
            audience: None,
            secrets: Vec::new(),
        }];
        assert!(validate_trust_domains(&runners, &workload).is_err());
        let mut separated = runners.clone();
        separated.insert(
            "trusted".into(),
            serde_json::from_value(firecracker_backend(json!({
                "trust_domain": "trusted",
                "host_identity": "host-trusted"
            })))
            .unwrap_or_else(|error| unreachable!("{error}")),
        );
        assert!(validate_trust_domains(&separated, &workload).is_ok());
        let mut same_host = separated.clone();
        same_host.insert(
            "trusted".into(),
            serde_json::from_value(firecracker_backend(json!({
                "trust_domain": "trusted",
                "host_identity": "host-shared"
            })))
            .unwrap_or_else(|error| unreachable!("{error}")),
        );
        assert!(validate_trust_domains(&same_host, &workload).is_err());
        let missing: Backend = serde_json::from_value(firecracker_backend(json!({
            "untrusted_capable": true,
            "trust_domain": null,
            "host_identity": "host-u"
        })))
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(
            validate_trust_domains(
                &std::collections::BTreeMap::from([("untrusted".into(), missing)]),
                &workload
            )
            .is_err()
        );
    }
    #[test]
    fn placement_ledger_rejects_cross_attempt_trust_overlap() {
        let config = firecracker_config(firecracker_backend(json!({})), None);
        let db =
            rusqlite::Connection::open_in_memory().unwrap_or_else(|error| unreachable!("{error}"));
        db.execute_batch(
            "CREATE TABLE hosted_execution(org TEXT,attempt TEXT,state TEXT);             CREATE TABLE hosted_microvm_placement(org TEXT,attempt TEXT,job TEXT,host TEXT,trust_domain TEXT,class TEXT,started_ms INTEGER);",
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        db.execute(
            "INSERT INTO hosted_execution VALUES('acme','prior','running')",
            [],
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        db.execute(
            "INSERT INTO hosted_microvm_placement VALUES('acme','prior','job','host-trusted','trusted','workload',1)",
            [],
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        let plan = crate::pipelines::CompiledPipeline {
            schema: "kiln.pipeline.v1".into(),
            identity: "0".repeat(64),
            name: "app".into(),
            triggers: Vec::new(),
            schedules: std::collections::BTreeMap::new(),
            jobs: vec![crate::pipelines::CompiledJob {
                id: "job".into(),
                source_job: "job".into(),
                needs: Vec::new(),
                runner: "vm".into(),
                matrix: std::collections::BTreeMap::new(),
                steps: Vec::new(),
                timeout_ms: 1000,
            }],
        };
        assert!(reserve_placements(&db, "acme", "next", &plan, "app", &config, true, 2).is_err());
    }
    #[test]
    fn cpu_affinity_is_bounded_and_duplicate_free() {
        assert!(valid_cpu_affinity("0"));
        assert!(valid_cpu_affinity("0-3,8,10-11"));
        assert!(!valid_cpu_affinity(""));
        assert!(!valid_cpu_affinity("3-1"));
        assert!(!valid_cpu_affinity("0-3,3"));
        assert!(!valid_cpu_affinity("4096"));
        assert!(!valid_cpu_affinity("a"));
        assert!(!valid_cpu_affinity("0,,1"));
    }
    #[test]
    fn backend_capabilities_drive_admission_declarations() {
        let backend: Backend = serde_json::from_value(firecracker_backend(json!({})))
            .unwrap_or_else(|error| unreachable!("{error}"));
        let capabilities = backend_capabilities(&backend);
        assert_eq!(capabilities.network, NetworkCapability::NoDevice);
        assert!(capabilities.snapshot);
        assert!(capabilities.per_job);
        assert!(capabilities.arch.contains(&"x86_64"));
    }
    #[test]
    fn explicit_guest_network_requests_are_refused() {
        let backend: Backend = serde_json::from_value(firecracker_backend(json!({
            "network": "tap"
        })))
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(backend_declared_network(&backend), Some("tap"));
        assert!(admit_backend(&backend, "trusted").is_err());
        let backend: Backend = serde_json::from_value(firecracker_backend(json!({
            "network": "none"
        })))
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert!(admit_backend(&backend, "trusted").is_ok());
    }
    #[test]
    fn snapshot_capability_refuses_warm_pools() {
        let backend: Backend = serde_json::from_value(firecracker_backend(json!({})))
            .unwrap_or_else(|error| unreachable!("{error}"));
        let capabilities = backend_capabilities(&backend);
        assert!(admit_snapshot(&capabilities, true).is_ok());
        let unsupported = Capabilities {
            snapshot: false,
            ..capabilities
        };
        assert!(admit_snapshot(&unsupported, true).is_err());
        assert!(admit_snapshot(&unsupported, false).is_ok());
    }
    #[test]
    fn cloud_hypervisor_runner_declares_phase_one_capabilities() {
        let executable = Executable {
            path: "/missing".into(),
            blake3: "0".repeat(64),
        };
        let image = Image {
            path: "/missing".into(),
            blake3: "0".repeat(64),
        };
        let backend = Backend::CloudHypervisor {
            cloud_hypervisor: executable,
            kernel: image.clone(),
            initrd: image,
            guest_shell: "/bin/bash".into(),
            vcpus: 1,
            memory_mib: 256,
            boot_timeout_ms: 10_000,
            source_image_mib: 64,
            qualification_identity: None,
            warm_pool: None,
            network: None,
            version: Some("53.0".into()),
            cpu_affinity: None,
        };
        let capabilities = backend_capabilities(&backend);
        assert_eq!(capabilities.network, NetworkCapability::NoDevice);
        assert!(capabilities.snapshot);
        assert!(capabilities.admits_arch("x86_64"));
        assert!(crate::microvm::Runner::from_cloud_hypervisor(&backend).is_some());
    }
    #[test]
    fn untrusted_and_workload_runners_cannot_share_a_cache_partition() {
        let untrusted: Backend = serde_json::from_value(firecracker_backend(json!({
            "untrusted_capable": true,
            "trust_domain": "untrusted",
            "host_identity": "host-u",
            "cache_partition": "shared"
        })))
        .unwrap_or_else(|error| unreachable!("{error}"));
        let trusted: Backend = serde_json::from_value(firecracker_backend(json!({
            "trust_domain": "trusted",
            "host_identity": "host-t",
            "cache_partition": "shared"
        })))
        .unwrap_or_else(|error| unreachable!("{error}"));
        let runners = std::collections::BTreeMap::from([
            ("untrusted".to_string(), untrusted),
            ("trusted".to_string(), trusted),
        ]);
        let workload = vec![WorkloadPolicy {
            organization: "acme".into(),
            project: "app".into(),
            runner: "trusted".into(),
            audience: None,
            secrets: Vec::new(),
        }];
        assert!(validate_trust_domains(&runners, &workload).is_err());
    }
    #[test]
    fn cache_partition_labels_require_resctrl() {
        assert!(cache_partition_admitted(None, false));
        assert!(cache_partition_admitted(Some("l3-a"), true));
        assert!(!cache_partition_admitted(Some("l3-a"), false));
    }
}
