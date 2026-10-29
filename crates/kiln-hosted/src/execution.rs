//! Hosted execution: explicit reviewed backends, durable pre-effect admission,
//! external resource enforcement and host-side observations. Namespaces are for
//! trusted jobs only. A supplied VM image does not itself qualify hostile use.
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub path: PathBuf,
    pub blake3: String,
}
impl Image {
    fn verify(&self) -> Result<PathBuf, String> {
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
    },
    Qemu {
        qemu: Executable,
        kernel: Image,
        initramfs: Image,
        root_image: Option<Image>,
        guest_shell: String,
        acceleration: String,
        qualification_identity: Option<String>,
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
                    if let Some(identity) = qualification_identity {
                        if identity.len() != 64 || !identity.bytes().all(|b| b.is_ascii_hexdigit())
                        {
                            return Err("invalid independent VM qualification identity".into());
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
fn clean(p: &Path) -> bool {
    p.is_absolute()
        && p.components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
}

pub fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_execution_config(singleton INTEGER PRIMARY KEY CHECK(singleton=1),identity TEXT NOT NULL,config TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS hosted_execution(org TEXT NOT NULL,project TEXT NOT NULL,attempt TEXT NOT NULL,pipeline TEXT NOT NULL,request_identity TEXT NOT NULL,config_identity TEXT NOT NULL,unit TEXT NOT NULL,state TEXT NOT NULL,reserved_ms INTEGER NOT NULL,result TEXT,PRIMARY KEY(org,attempt));") .map_err(|e|e.to_string())
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
    match (trust,backend) {
        ("trusted",_)=>Ok(()),
        ("hostile",Backend::Qemu{qualification_identity:Some(_),..})=>Ok(()),
        ("hostile",_)=>Err("hostile execution blocked: independently qualified VM backend and immutable images required; namespaces are trusted-only".into()),
        _=>Err("trust must be trusted or hostile".into()),
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
    if plan.jobs.is_empty() || plan.jobs.len() > 128 {
        return Err("pipeline execution size rejected".into());
    }
    let mut maximum_ms = 0_u64;
    let mut step_count = 0_usize;
    let mut operator_maximum = u64::MAX;
    for job in &plan.jobs {
        let backend = config
            .runners
            .get(&job.runner)
            .ok_or_else(|| format!("runner '{}' has no reviewed backend mapping", job.runner))?;
        admit_backend(backend, &request.trust)?;
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
    }
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
    let tx = db.transaction().map_err(|e| e.to_string())?;
    check_capacity(&tx, config.maximum_concurrent)?;
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
        let bound_job = crate::identity(
            &json!({"attempt":request.attempt_id,"job":job.id,"organization":actor.organization,"project":request.project}),
        )?;
        let policy = config.workload.iter().find(|p| {
            p.organization == actor.organization
                && p.project == request.project
                && p.runner == job.runner
        });
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
            workspace: &work,
            now_ms: cache_now_ms,
        };
        let mut cache_key = None;
        let mut cache_ms = None;
        let mut cache_outcome = "disabled_unconfigured";
        let mut cache_observation = json!({"outcome":cache_outcome,"duration_ms":null});
        let credential_grants =
            policy.is_some_and(|p| p.audience.is_some() || !p.secrets.is_empty());
        if credential_grants {
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
                match observation {
                    Ok(mut observation) => {
                        cleanup &= observation.cleaned;
                        if !observation.cleaned {
                            state = "unknown";
                        } else if observation.cancelled {
                            state = "cancelled";
                        } else if !observation.success {
                            state = "failed";
                        }
                        redact_value(&mut observation.value, &redactions);
                        output_budget =
                            output_budget.saturating_sub(observation.value.to_string().len());
                        steps.push(observation.value);
                    }
                    Err(error) => {
                        state = "unknown";
                        cleanup = false;
                        steps.push(json!({"state":"unknown","reason":error}));
                    }
                }
                if state != "succeeded" {
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
        if state == "succeeded" && cleanup {
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
        jobs.push(json!({"job":job.id,"state":state,"steps":steps,"cache":cache_observation}));
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
            Some(Backend::Qemu {
                qualification_identity: Some(reference),
                ..
            }) => Some(reference.clone()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut result = json!({"attempt_id":request.attempt_id,"pipeline":request.pipeline_id,"state":state,"elapsed_ms":elapsed_ms,"cleaned":cleanup,"evidence":evidence,"jobs":jobs,"requested_trust":request.trust,"hostile_isolation_claim":false,"operator_qualification_references":qualification_references,"qualification_evidence":"external independent evidence required"});
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

fn infrastructure_failure_state(cancel_requested: bool, cleaned: bool) -> (&'static str, bool) {
    if !cleaned {
        ("unknown", false)
    } else if cancel_requested {
        ("cancelled", true)
    } else {
        ("failed", false)
    }
}
struct StepObservation {
    success: bool,
    cancelled: bool,
    cleaned: bool,
    value: Value,
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
fn manager_request(
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
fn systemctl(config: &Config, args: &[&str]) -> Result<bounded_exec::ExecutionOutput, String> {
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
fn cleanup_unit(config: &Config, unit: &str) -> bool {
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
    let mut service = vec![
        OsString::from("--user"),
        OsString::from("--wait"),
        OsString::from("--pipe"),
        OsString::from(format!("--unit={unit}")),
    ];
    for property in [
        format!("MemoryMax={}", config.memory_bytes),
        "MemorySwapMax=0".into(),
        format!("TasksMax={}", config.processes),
        format!("CPUQuota={}%", config.cpu_percent),
        format!("RuntimeMaxSec={}ms", timeout_ms.saturating_add(1000)),
        "KillMode=control-group".into(),
        "SendSIGKILL=yes".into(),
        "TimeoutStopSec=2s".into(),
    ] {
        service.push(OsString::from(format!("--property={property}")));
    }
    service.push(config.env.verify()?.into_os_string());
    service.push(OsString::from("-i"));
    let input = match backend {
        Backend::Namespaces {
            unshare,
            python,
            mount,
            shell,
            runtime_roots,
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
            let mut success = output.completion == Completion::Exited
                && output.exit_code == Some(0)
                && !output.stdout.truncated
                && !output.stderr.truncated;
            let stdout = String::from_utf8_lossy(&output.stdout.bytes).into_owned();
            let mut guest = None;
            if matches!(backend, Backend::Qemu { .. }) {
                guest = stdout
                    .lines()
                    .filter_map(|line| line.strip_prefix("KILN_RESULT:"))
                    .next_back()
                    .and_then(|line| serde_json::from_str::<Value>(line).ok());
                success &= guest.as_ref().is_some_and(|r| {
                    r.get("schema") == Some(&json!("kiln.hosted.guest.v1"))
                        && r.get("exit_code") == Some(&json!(0))
                });
            }
            Ok(StepObservation {
                success,
                cancelled,
                cleaned,
                value: json!({"name":step.name,"state":if !cleaned {"unknown"} else if cancelled {"cancelled"} else if output.completion==Completion::TimedOut {"timed_out"} else if success {"succeeded"} else {"failed"},"exit_code":output.exit_code,"signal":output.signal,"elapsed_ms":elapsed_ms,"stdout":stdout,"stderr":String::from_utf8_lossy(&output.stderr.bytes),"stdout_truncated":output.stdout.truncated,"stderr_truncated":output.stderr.truncated,"cleaned":cleaned,"guest":guest}),
            })
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
                return Ok(
                    json!({"attempt_id":lookup.attempt_id,"state":state,"result":result.and_then(|r|serde_json::from_str::<Value>(&r).ok())}),
                );
            }
            if !["reserved", "running", "unknown"].contains(&state.as_str()) {
                return Err("no uncertain execution to recover".into());
            }
            let (_, config) = configuration(db)?;
            let cleaned = cleanup_unit(&config, &unit);
            if cleaned {
                db.execute("UPDATE hosted_execution SET state='reaped_unknown' WHERE org=?1 AND project=?2 AND attempt=?3",params![actor.organization,lookup.project,lookup.attempt_id]).map_err(|e|e.to_string())?;
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
}
