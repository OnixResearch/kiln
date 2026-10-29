use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const PIPELINE_SCHEMA: &str = "kiln.hosted.pipeline.v1";
pub const PLAN_SCHEMA: &str = "kiln.hosted.pipeline-plan.v1";
pub const MAX_CONFIG_BYTES: usize = 1_048_576;
pub const MAX_JOBS: usize = 64;
pub const MAX_TEMPLATES: usize = 64;
pub const MAX_EXPANDED_JOBS: usize = 256;
pub const MAX_STEPS_PER_JOB: usize = 64;
pub const MAX_TOTAL_STEPS: usize = 4096;
pub const MAX_MATRIX_AXES: usize = 8;
pub const MAX_MATRIX_VALUES: usize = 32;
pub const MAX_PARAMETERS: usize = 64;
pub const MAX_ENVIRONMENT: usize = 128;
pub const MAX_VALUE_BYTES: usize = 4096;
pub const MAX_COMMAND_BYTES: usize = 32_768;
pub const MAX_PLAN_BYTES: usize = 4_194_304;
pub const MAX_EDGES: usize = 16_384;
pub const MAX_TIMEOUT_MS: u64 = 86_400_000;
pub const DEFAULT_TIMEOUT_MS: u64 = 3_600_000;
pub const MAX_REGISTRY_ROWS: i64 = 1024;

type Strings = BTreeMap<String, String>;

pub const MILLISECONDS_PER_MINUTE: u64 = 60_000;
pub const ACTIONS_DEFAULT_TIMEOUT_MS: u64 = 360 * MILLISECONDS_PER_MINUTE;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    Manual,
    Push,
    PullRequest,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shell {
    #[default]
    Bash,
    Sh,
}

fn root_directory() -> String {
    ".".into()
}
fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_MS
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunStep {
    #[serde(default)]
    pub name: String,
    pub run: String,
    #[serde(default)]
    pub shell: Shell,
    #[serde(default)]
    pub env: Strings,
    #[serde(default = "root_directory")]
    pub working_directory: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Run(RunStep),
    Use {
        step: String,
        #[serde(default)]
        parameters: Strings,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepTemplate {
    #[serde(default)]
    pub parameters: Strings,
    pub step: RunStep,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobTemplate {
    #[serde(default)]
    pub parameters: Strings,
    pub runner: String,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub env: Strings,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub parameters: Strings,
    #[serde(default)]
    pub runner: Option<String>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub env: Strings,
    #[serde(default)]
    pub needs: Vec<String>,
    #[serde(default)]
    pub matrix: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pipeline {
    pub schema: String,
    pub name: String,
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub parameters: Strings,
    #[serde(default)]
    pub env: Strings,
    #[serde(default)]
    pub templates: BTreeMap<String, JobTemplate>,
    #[serde(default)]
    pub steps: BTreeMap<String, StepTemplate>,
    pub jobs: BTreeMap<String, Job>,
}

pub type CompiledStep = RunStep;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledJob {
    pub id: String,
    pub source_job: String,
    pub needs: Vec<String>,
    pub runner: String,
    pub matrix: Strings,
    pub steps: Vec<CompiledStep>,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledPipeline {
    pub schema: String,
    pub identity: String,
    pub name: String,
    pub triggers: Vec<Trigger>,
    /// Stable topological order. Dependencies name expanded job ids.
    pub jobs: Vec<CompiledJob>,
}

fn fail(path: &str, message: &str) -> String {
    format!("{path}: {message}")
}
fn bounded_text(value: &str, max: usize, path: &str) -> Result<(), String> {
    if value.len() > max || value.contains('\0') {
        return Err(fail(path, "text exceeds bound or contains NUL"));
    }
    Ok(())
}
fn name(value: &str, path: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(fail(
            path,
            "expected 1..64 ASCII letters, digits, '_' or '-'",
        ));
    }
    Ok(())
}
fn strings(values: &Strings, max: usize, path: &str, environment: bool) -> Result<(), String> {
    if values.len() > max {
        return Err(fail(path, "too many entries"));
    }
    for (key, value) in values {
        name(key, &format!("{path}.{key}"))?;
        if environment
            && (!key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                || key.as_bytes().first().is_some_and(u8::is_ascii_digit))
        {
            return Err(fail(
                &format!("{path}.{key}"),
                "invalid environment variable name",
            ));
        }
        bounded_text(value, MAX_VALUE_BYTES, &format!("{path}.{key}"))?;
    }
    Ok(())
}

/// Only literal parameter/matrix substitutions are supported. No evaluation,
/// shell quoting, credential interpolation, or implicit provider context occurs.
fn substitute(
    value: &str,
    parameters: &Strings,
    matrix: &Strings,
    path: &str,
    max: usize,
) -> Result<String, String> {
    let mut output = String::new();
    let mut rest = value;
    while let Some((prefix, expression)) = rest.split_once("${{") {
        output.push_str(prefix);
        let (key, remaining) = expression
            .split_once("}}")
            .ok_or_else(|| fail(path, "unterminated expression"))?;
        let key = key.trim();
        let replacement = if let Some(key) = key.strip_prefix("parameters.") {
            parameters.get(key)
        } else if let Some(key) = key.strip_prefix("matrix.") {
            matrix.get(key)
        } else {
            return Err(fail(
                path,
                "unsupported expression; only parameters.KEY and matrix.KEY are supported",
            ));
        };
        let replacement =
            replacement.ok_or_else(|| fail(path, &format!("unknown expression {key}")))?;
        // Values are literals, never recursively evaluated.
        if replacement.contains("${{") {
            return Err(fail(path, "parameter/matrix values must be literals"));
        }
        output.push_str(replacement);
        if output.len() > max {
            return Err(fail(path, "expanded text exceeds bound"));
        }
        rest = remaining;
    }
    output.push_str(rest);
    bounded_text(&output, max, path)?;
    Ok(output)
}
fn expand_env(
    values: &Strings,
    parameters: &Strings,
    matrix: &Strings,
    path: &str,
) -> Result<Strings, String> {
    strings(values, MAX_ENVIRONMENT, path, true)?;
    values
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                substitute(
                    value,
                    parameters,
                    matrix,
                    &format!("{path}.{key}"),
                    MAX_VALUE_BYTES,
                )?,
            ))
        })
        .collect()
}
fn run_step(
    step: &RunStep,
    parameters: &Strings,
    matrix: &Strings,
    environment: &Strings,
    path: &str,
) -> Result<RunStep, String> {
    let mut env = environment.clone();
    env.extend(expand_env(
        &step.env,
        parameters,
        matrix,
        &format!("{path}.env"),
    )?);
    strings(&env, MAX_ENVIRONMENT, &format!("{path}.env"), true)?;
    let run = substitute(
        &step.run,
        parameters,
        matrix,
        &format!("{path}.run"),
        MAX_COMMAND_BYTES,
    )?;
    if run.trim().is_empty() {
        return Err(fail(path, "run must not be empty"));
    }
    let working_directory = substitute(
        &step.working_directory,
        parameters,
        matrix,
        &format!("{path}.working_directory"),
        MAX_VALUE_BYTES,
    )?;
    if working_directory.is_empty()
        || working_directory.starts_with('/')
        || working_directory.contains('\\')
        || working_directory.split('/').any(|v| v == "..")
    {
        return Err(fail(
            path,
            "working_directory must remain inside the acquired repository",
        ));
    }
    Ok(RunStep {
        name: substitute(&step.name, parameters, matrix, path, MAX_VALUE_BYTES)?,
        run,
        shell: step.shell,
        env,
        working_directory,
    })
}
fn matrix_rows(matrix: &BTreeMap<String, Vec<String>>, path: &str) -> Result<Vec<Strings>, String> {
    if matrix.len() > MAX_MATRIX_AXES {
        return Err(fail(path, "too many matrix axes"));
    }
    let mut rows = vec![Strings::new()];
    for (key, values) in matrix {
        name(key, path)?;
        if values.is_empty() || values.len() > MAX_MATRIX_VALUES {
            return Err(fail(path, "matrix axis must have 1..32 literal values"));
        }
        let count = rows
            .len()
            .checked_mul(values.len())
            .ok_or_else(|| fail(path, "matrix expansion overflow"))?;
        if count > MAX_EXPANDED_JOBS {
            return Err(fail(path, "matrix expansion exceeds MAX_EXPANDED_JOBS=256"));
        }
        let mut distinct = BTreeSet::new();
        for value in values {
            bounded_text(value, MAX_VALUE_BYTES, path)?;
            if value.contains("${{") || !distinct.insert(value) {
                return Err(fail(path, "matrix values must be distinct literals"));
            }
        }
        let mut next = Vec::with_capacity(count);
        for row in rows {
            for value in values {
                let mut row = row.clone();
                row.insert(key.clone(), value.clone());
                next.push(row);
            }
        }
        rows = next;
    }
    Ok(rows)
}

// r[impl kiln.hosted.pipelines]
/// Pure bounded compiler. Identity is BLAKE3 over the exact compact JSON plan
/// with an empty identity field; schema supplies domain separation. It binds
/// commands, runner labels, dependency expansion, env, triggers and deadlines.
pub fn compile(pipeline: &Pipeline) -> Result<CompiledPipeline, String> {
    let bytes = serde_json::to_vec(pipeline).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(fail("$", "configuration exceeds MAX_CONFIG_BYTES"));
    }
    if pipeline.schema != PIPELINE_SCHEMA {
        return Err(fail("$.schema", "unsupported schema"));
    }
    bounded_text(&pipeline.name, MAX_VALUE_BYTES, "$.name")?;
    if pipeline.name.trim().is_empty()
        || pipeline.triggers.is_empty()
        || pipeline.triggers.len() > 3
    {
        return Err(fail("$", "name and 1..3 distinct triggers are required"));
    }
    let triggers: BTreeSet<_> = pipeline.triggers.iter().map(|t| format!("{t:?}")).collect();
    if triggers.len() != pipeline.triggers.len() {
        return Err(fail("$.triggers", "duplicate trigger"));
    }
    strings(&pipeline.parameters, MAX_PARAMETERS, "$.parameters", false)?;
    strings(&pipeline.env, MAX_ENVIRONMENT, "$.env", true)?;
    if pipeline.jobs.is_empty()
        || pipeline.jobs.len() > MAX_JOBS
        || pipeline.templates.len() > MAX_TEMPLATES
        || pipeline.steps.len() > MAX_TEMPLATES
    {
        return Err(fail("$", "job/template/step count exceeds named bounds"));
    }
    for (key, template) in &pipeline.templates {
        name(key, "$.templates")?;
        strings(
            &template.parameters,
            MAX_PARAMETERS,
            "$.templates.parameters",
            false,
        )?;
        strings(&template.env, MAX_ENVIRONMENT, "$.templates.env", true)?;
        if template.steps.is_empty()
            || template.steps.len() > MAX_STEPS_PER_JOB
            || template.timeout_ms == 0
            || template.timeout_ms > MAX_TIMEOUT_MS
        {
            return Err(fail("$.templates", "invalid step count or timeout"));
        }
    }
    for (key, template) in &pipeline.steps {
        name(key, "$.steps")?;
        strings(
            &template.parameters,
            MAX_PARAMETERS,
            "$.steps.parameters",
            false,
        )?;
    }
    let mut expanded = BTreeMap::<String, Vec<String>>::new();
    let mut jobs = BTreeMap::<String, CompiledJob>::new();
    let mut total_steps = 0usize;
    for (key, job) in &pipeline.jobs {
        let path = format!("$.jobs.{key}");
        name(key, &path)?;
        strings(
            &job.parameters,
            MAX_PARAMETERS,
            &format!("{path}.parameters"),
            false,
        )?;
        strings(&job.env, MAX_ENVIRONMENT, &format!("{path}.env"), true)?;
        if job.needs.len() > MAX_JOBS {
            return Err(fail(&path, "too many dependencies"));
        }
        let mut distinct_needs = BTreeSet::new();
        for dependency in &job.needs {
            if !pipeline.jobs.contains_key(dependency) || !distinct_needs.insert(dependency) {
                return Err(fail(
                    &format!("{path}.needs"),
                    "unknown or duplicate dependency",
                ));
            }
        }
        let template = job
            .template
            .as_ref()
            .map(|id| {
                pipeline.templates.get(id).ok_or_else(|| {
                    fail(
                        &format!("{path}.template"),
                        &format!("missing template {id}"),
                    )
                })
            })
            .transpose()?;
        if template.is_some()
            && (!job.steps.is_empty() || job.runner.is_some() || job.timeout_ms.is_some())
        {
            return Err(fail(
                &path,
                "template jobs may override only parameters, env, needs and matrix",
            ));
        }
        let steps = template.map_or(&job.steps, |v| &v.steps);
        if steps.is_empty() || steps.len() > MAX_STEPS_PER_JOB {
            return Err(fail(&format!("{path}.steps"), "expected 1..64 steps"));
        }
        let runner = template
            .map(|v| v.runner.as_str())
            .or(job.runner.as_deref())
            .ok_or_else(|| {
                fail(
                    &format!("{path}.runner"),
                    "explicit reviewed runner label required",
                )
            })?;
        let timeout_ms = template
            .map(|v| v.timeout_ms)
            .or(job.timeout_ms)
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        if timeout_ms == 0 || timeout_ms > MAX_TIMEOUT_MS {
            return Err(fail(
                &format!("{path}.timeout_ms"),
                "timeout outside 1..86400000",
            ));
        }
        let mut parameters = pipeline.parameters.clone();
        if let Some(template) = template {
            parameters.extend(template.parameters.clone());
        }
        // Overrides must refer to a declared global or template parameter.
        for key in job.parameters.keys() {
            if !parameters.contains_key(key) {
                return Err(fail(
                    &format!("{path}.parameters.{key}"),
                    "parameter is not declared",
                ));
            }
        }
        parameters.extend(job.parameters.clone());
        strings(
            &parameters,
            MAX_PARAMETERS,
            &format!("{path}.parameters"),
            false,
        )?;
        let rows = matrix_rows(&job.matrix, &format!("{path}.matrix"))?;
        if jobs
            .len()
            .checked_add(rows.len())
            .is_none_or(|v| v > MAX_EXPANDED_JOBS)
        {
            return Err(fail(&path, "total expansion exceeds MAX_EXPANDED_JOBS=256"));
        }
        for (index, matrix) in rows.into_iter().enumerate() {
            total_steps = total_steps
                .checked_add(steps.len())
                .ok_or_else(|| fail(&path, "step count overflow"))?;
            if total_steps > MAX_TOTAL_STEPS {
                return Err(fail(&path, "expanded steps exceed MAX_TOTAL_STEPS=4096"));
            }
            let id = format!("{key}[{index}]");
            let mut env = pipeline.env.clone();
            if let Some(template) = template {
                env.extend(template.env.clone());
            }
            env.extend(job.env.clone());
            let env = expand_env(&env, &parameters, &matrix, &format!("{path}.env"))?;
            let mut compiled_steps = Vec::with_capacity(steps.len());
            for (index, step) in steps.iter().enumerate() {
                let step_path = format!("{path}.steps[{index}]");
                let compiled = match step {
                    Step::Run(step) => run_step(step, &parameters, &matrix, &env, &step_path)?,
                    Step::Use {
                        step,
                        parameters: overrides,
                    } => {
                        let definition = pipeline.steps.get(step).ok_or_else(|| {
                            fail(&step_path, &format!("missing reusable step {step}"))
                        })?;
                        strings(overrides, MAX_PARAMETERS, &step_path, false)?;
                        let mut local = parameters.clone();
                        local.extend(definition.parameters.clone());
                        for key in overrides.keys() {
                            if !definition.parameters.contains_key(key) {
                                return Err(fail(
                                    &step_path,
                                    &format!("unknown step parameter {key}"),
                                ));
                            }
                        }
                        for (key, value) in overrides {
                            local.insert(
                                key.clone(),
                                substitute(
                                    value,
                                    &parameters,
                                    &matrix,
                                    &step_path,
                                    MAX_VALUE_BYTES,
                                )?,
                            );
                        }
                        strings(&local, MAX_PARAMETERS, &step_path, false)?;
                        run_step(&definition.step, &local, &matrix, &env, &step_path)?
                    }
                };
                compiled_steps.push(compiled);
            }
            let runner = substitute(
                runner,
                &parameters,
                &matrix,
                &format!("{path}.runner"),
                MAX_VALUE_BYTES,
            )?;
            if runner.is_empty() {
                return Err(fail(&path, "empty runner"));
            }
            expanded.entry(key.clone()).or_default().push(id.clone());
            jobs.insert(
                id.clone(),
                CompiledJob {
                    id,
                    source_job: key.clone(),
                    needs: vec![],
                    runner,
                    matrix,
                    steps: compiled_steps,
                    timeout_ms,
                },
            );
        }
    }
    let mut edges = 0usize;
    for job in jobs.values_mut() {
        let source = pipeline
            .jobs
            .get(&job.source_job)
            .ok_or_else(|| fail("$.jobs", "missing source job"))?;
        for dependency in &source.needs {
            let dependency_jobs = expanded
                .get(dependency)
                .ok_or_else(|| fail("$.jobs", "missing expanded dependency"))?;
            edges = edges
                .checked_add(dependency_jobs.len())
                .ok_or_else(|| fail("$.jobs", "edge overflow"))?;
            if edges > MAX_EDGES {
                return Err(fail(
                    "$.jobs",
                    "dependency expansion exceeds MAX_EDGES=16384",
                ));
            }
            job.needs.extend(dependency_jobs.iter().cloned());
        }
        job.needs.sort();
    }
    let mut ordered = Vec::with_capacity(jobs.len());
    let mut completed = BTreeSet::new();
    while !jobs.is_empty() {
        let next = jobs
            .iter()
            .find(|(_, job)| job.needs.iter().all(|id| completed.contains(id)))
            .map(|(id, _)| id.clone());
        let Some(id) = next else {
            return Err(fail(
                "$.jobs.*.needs",
                &format!(
                    "dependency cycle involving {}",
                    jobs.keys().cloned().collect::<Vec<_>>().join(", ")
                ),
            ));
        };
        completed.insert(id.clone());
        ordered.push(
            jobs.remove(&id)
                .ok_or_else(|| fail("$", "internal ordering failure"))?,
        );
    }
    let mut plan = CompiledPipeline {
        schema: PLAN_SCHEMA.into(),
        identity: String::new(),
        name: pipeline.name.clone(),
        triggers: pipeline.triggers.clone(),
        jobs: ordered,
    };
    let bytes = serde_json::to_vec(&plan).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_PLAN_BYTES {
        return Err(fail("$", "compiled plan exceeds MAX_PLAN_BYTES"));
    }
    plan.identity = blake3::hash(&bytes).to_hex().to_string();
    Ok(plan)
}

pub fn compile_value(input: Value) -> Result<CompiledPipeline, String> {
    if serde_json::to_vec(&input).map_err(|e| e.to_string())?.len() > MAX_CONFIG_BYTES {
        return Err(fail("$", "configuration exceeds MAX_CONFIG_BYTES"));
    }
    let pipeline: Pipeline =
        serde_json::from_value(input).map_err(|e| fail("$", &e.to_string()))?;
    compile(&pipeline)
}

#[derive(Debug, Serialize)]
pub struct Migration {
    pub pipeline: Pipeline,
    pub compiled: CompiledPipeline,
    pub diagnostics: Vec<String>,
}
fn object<'a>(value: &'a Value, path: &str) -> Result<&'a serde_json::Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| fail(path, "expected object"))
}
fn fields(value: &Value, allowed: &[&str], path: &str) -> Result<(), String> {
    for key in object(value, path)?.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(fail(
                &format!("{path}.{key}"),
                "unsupported Actions feature; remove it or implement an explicitly reviewed native equivalent",
            ));
        }
    }
    Ok(())
}
fn text(value: &Value, path: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| fail(path, "expected literal string"))
}
fn env_value(value: Option<&Value>, path: &str) -> Result<Strings, String> {
    value
        .map(|v| {
            serde_json::from_value(v.clone())
                .map_err(|_| fail(path, "environment must map names to literal strings"))
        })
        .unwrap_or_else(|| Ok(Strings::new()))
}
fn string_list(value: &Value, path: &str) -> Result<Vec<String>, String> {
    if let Some(value) = value.as_str() {
        return Ok(vec![value.into()]);
    }
    value
        .as_array()
        .ok_or_else(|| fail(path, "expected string or array of strings"))?
        .iter()
        .map(|v| text(v, path))
        .collect()
}
fn trigger(value: &str, path: &str) -> Result<Trigger, String> {
    match value {
        "workflow_dispatch" => Ok(Trigger::Manual),
        "push" => Ok(Trigger::Push),
        "pull_request" => Ok(Trigger::PullRequest),
        _ => Err(fail(
            path,
            "only push, pull_request and workflow_dispatch without filters/inputs are supported",
        )),
    }
}

// r[impl kiln.hosted.pipelines]
/// Fail-closed GitHub Actions shell-job importer. All jobs run only after all
/// needs succeed; matrix needs expand to every dependency variant. Bash uses
/// --noprofile --norc -e -o pipefail; sh uses -e. Runner labels and triggers are
/// preserved for separately reviewed source and isolation bindings.
pub fn migrate(yaml: &str) -> Result<Migration, String> {
    bounded_text(yaml, MAX_CONFIG_BYTES, "$yaml")?;
    let parsed: serde_yaml::Value =
        serde_yaml::from_str(yaml).map_err(|e| fail("$yaml", &e.to_string()))?;
    let source = serde_json::to_value(parsed).map_err(|e| fail("$yaml", &e.to_string()))?;
    fields(&source, &["name", "on", "env", "jobs"], "$")?;
    let source_obj = object(&source, "$")?;
    let events = source_obj
        .get("on")
        .ok_or_else(|| fail("$.on", "explicit triggers required"))?;
    let triggers = if let Some(events) = events.as_object() {
        let mut triggers = vec![];
        for (event, config) in events {
            if !config.is_null() && config.as_object().is_none_or(|v| !v.is_empty()) {
                return Err(fail(
                    &format!("$.on.{event}"),
                    "trigger filters, activity types and inputs require an explicit native mapping",
                ));
            }
            triggers.push(trigger(event, &format!("$.on.{event}"))?);
        }
        triggers
    } else {
        string_list(events, "$.on")?
            .iter()
            .map(|v| trigger(v, "$.on"))
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut jobs = BTreeMap::new();
    let source_jobs = source_obj
        .get("jobs")
        .ok_or_else(|| fail("$.jobs", "required"))?;
    for (id, source_job) in object(source_jobs, "$.jobs")? {
        let path = format!("$.jobs.{id}");
        fields(
            source_job,
            &[
                "name",
                "runs-on",
                "needs",
                "strategy",
                "env",
                "steps",
                "timeout-minutes",
            ],
            &path,
        )?;
        let source_job = object(source_job, &path)?;
        // A display name cannot silently disappear from migrated semantics.
        if source_job.contains_key("name") {
            return Err(fail(
                &format!("{path}.name"),
                "job display names are not supported; use the job id",
            ));
        }
        let runner = text(
            source_job
                .get("runs-on")
                .ok_or_else(|| fail(&path, "runs-on is required"))?,
            &format!("{path}.runs-on"),
        )?;
        let needs = source_job
            .get("needs")
            .map(|v| string_list(v, &format!("{path}.needs")))
            .transpose()?
            .unwrap_or_default();
        let mut matrix = BTreeMap::new();
        if let Some(strategy) = source_job.get("strategy") {
            fields(
                strategy,
                &["matrix", "fail-fast", "max-parallel"],
                &format!("{path}.strategy"),
            )?;
            // Parallel cancellation is not equivalent to dependency semantics.
            if strategy.get("fail-fast") != Some(&Value::Bool(false)) {
                return Err(fail(
                    &format!("{path}.strategy.fail-fast"),
                    "matrix requires explicit fail-fast: false; implicit/true cancellation is unsupported",
                ));
            }
            if strategy.get("max-parallel").is_some() {
                return Err(fail(
                    &format!("{path}.strategy.max-parallel"),
                    "parallel scheduling limits require explicit capacity mapping",
                ));
            }
            let axes = strategy
                .get("matrix")
                .ok_or_else(|| fail(&format!("{path}.strategy.matrix"), "required"))?;
            for (key, values) in object(axes, &format!("{path}.strategy.matrix"))? {
                if key == "include" || key == "exclude" {
                    return Err(fail(
                        &format!("{path}.strategy.matrix.{key}"),
                        "include/exclude matrix semantics are unsupported",
                    ));
                }
                let values = values.as_array().ok_or_else(|| {
                    fail(
                        &format!("{path}.strategy.matrix.{key}"),
                        "expected literal array",
                    )
                })?;
                let values = values
                    .iter()
                    .map(|v| match v {
                        Value::String(v) => Ok(v.clone()),
                        Value::Number(v) => Ok(v.to_string()),
                        Value::Bool(v) => Ok(v.to_string()),
                        _ => Err(fail(&path, "matrix values must be literal scalar values")),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                matrix.insert(key.clone(), values);
            }
        }
        let timeout_ms = source_job
            .get("timeout-minutes")
            .map(|v| {
                v.as_u64()
                    .and_then(|v| v.checked_mul(MILLISECONDS_PER_MINUTE))
                    .ok_or_else(|| {
                        fail(
                            &format!("{path}.timeout-minutes"),
                            "expected bounded positive integer minutes",
                        )
                    })
            })
            .transpose()?
            .unwrap_or(ACTIONS_DEFAULT_TIMEOUT_MS);
        let source_steps = source_job
            .get("steps")
            .and_then(Value::as_array)
            .ok_or_else(|| fail(&format!("{path}.steps"), "expected shell steps"))?;
        let mut steps = vec![];
        for (index, source_step) in source_steps.iter().enumerate() {
            let path = format!("{path}.steps[{index}]");
            fields(
                source_step,
                &["name", "run", "shell", "env", "working-directory"],
                &path,
            )?;
            let step = object(source_step, &path)?;
            let shell = match step.get("shell").and_then(Value::as_str) {
                Some("bash") => Shell::Bash,
                Some("sh") => Shell::Sh,
                None => {
                    return Err(fail(
                        &format!("{path}.shell"),
                        "explicit bash or sh required; Actions implicit shell has different error semantics",
                    ));
                }
                _ => {
                    return Err(fail(
                        &format!("{path}.shell"),
                        "only literal bash or sh is supported",
                    ));
                }
            };
            steps.push(Step::Run(RunStep {
                name: step
                    .get("name")
                    .map(|v| text(v, &format!("{path}.name")))
                    .transpose()?
                    .unwrap_or_default(),
                run: text(
                    step.get("run").ok_or_else(|| {
                        fail(
                            &format!("{path}.run"),
                            "required; uses/actions have no native lowering",
                        )
                    })?,
                    &format!("{path}.run"),
                )?,
                shell,
                env: env_value(step.get("env"), &format!("{path}.env"))?,
                working_directory: step
                    .get("working-directory")
                    .map(|v| text(v, &format!("{path}.working-directory")))
                    .transpose()?
                    .unwrap_or_else(root_directory),
            }));
        }
        jobs.insert(
            id.clone(),
            Job {
                template: None,
                parameters: Strings::new(),
                runner: Some(runner),
                steps,
                env: env_value(source_job.get("env"), &format!("{path}.env"))?,
                needs,
                matrix,
                timeout_ms: Some(timeout_ms),
            },
        );
    }
    let pipeline = Pipeline {
        schema: PIPELINE_SCHEMA.into(),
        name: source_obj
            .get("name")
            .map(|v| text(v, "$.name"))
            .transpose()?
            .unwrap_or_else(|| "Migrated Actions workflow".into()),
        triggers,
        parameters: Strings::new(),
        env: env_value(source_obj.get("env"), "$.env")?,
        templates: BTreeMap::new(),
        steps: BTreeMap::new(),
        jobs,
    };
    // Native parameter expressions have no GitHub equivalent and must not be
    // newly introduced by migration. Only matrix expressions survive import.
    let value = serde_json::to_value(&pipeline).map_err(|e| e.to_string())?;
    reject_non_matrix_expressions(&value, "$")?;
    let compiled = compile(&pipeline)?;
    Ok(Migration { pipeline, compiled, diagnostics: vec!["Runner labels and trigger names are preserved, not authorized: bind an exact reviewed source connection and isolation runner mapping before dispatch. Jobs require all dependencies to succeed; steps run sequentially with fresh shell processes in the acquired source tree. GitHub default environment/context and GITHUB_ENV/GITHUB_OUTPUT/step-output protocols are not supplied.".into()] })
}
fn reject_non_matrix_expressions(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::String(value) => {
            let mut rest = value.as_str();
            while let Some((_, expression)) = rest.split_once("${{") {
                let (key, remaining) = expression
                    .split_once("}}")
                    .ok_or_else(|| fail(path, "unterminated expression"))?;
                let key = key.trim();
                if !key.strip_prefix("matrix.").is_some_and(|v| {
                    !v.is_empty()
                        && v.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                }) {
                    return Err(fail(
                        path,
                        "only simple matrix.KEY expressions have migration semantics",
                    ));
                }
                rest = remaining;
            }
            // These runner-provided protocols have no native equivalent.
            for name in ["GITHUB_", "RUNNER_", "ACTIONS_"] {
                if value.contains(name) {
                    return Err(fail(
                        path,
                        "GitHub runner environment/file-command protocols require explicit native replacement",
                    ));
                }
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                reject_non_matrix_expressions(value, &format!("{path}[{index}]"))?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                reject_non_matrix_expressions(value, &format!("{path}.{key}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_pipelines (organization TEXT NOT NULL, project TEXT NOT NULL, name TEXT NOT NULL, revision INTEGER NOT NULL, identity TEXT NOT NULL, config TEXT NOT NULL, plan TEXT NOT NULL, reviewer TEXT NOT NULL, reviewed_ms INTEGER NOT NULL, PRIMARY KEY(organization,project,name)); CREATE INDEX IF NOT EXISTS hosted_pipeline_identity ON hosted_pipelines(organization,project,identity);").map_err(|e| e.to_string())
}

/// Load a currently reviewed exact plan. Never use an unreviewed compile result
/// to authorize dispatch. Registry updates revoke the previous exact identity.
pub fn reviewed(
    db: &Connection,
    actor: &crate::Actor,
    project: &str,
    identity: &str,
) -> Result<CompiledPipeline, String> {
    name(project, "$.project")?;
    crate::accounts::require_project(
        db,
        actor,
        project,
        &["owner", "admin", "member", "viewer", "billing"],
    )?;
    let config: Option<String> = db.query_row("SELECT config FROM hosted_pipelines WHERE organization=?1 AND project=?2 AND identity=?3 LIMIT 1", params![actor.organization, project, identity], |row| row.get(0)).optional().map_err(|e| e.to_string())?;
    let config = config.ok_or_else(|| {
        "pipeline is not currently reviewed for this organization/project".to_string()
    })?;
    let plan = compile_value(serde_json::from_str(&config).map_err(|e| e.to_string())?)?;
    if plan.identity != identity {
        return Err("stored pipeline identity mismatch".into());
    }
    Ok(plan)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Publish {
    project: String,
    name: String,
    expected_revision: u64,
    config: Pipeline,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrateInput {
    yaml: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lookup {
    project: String,
    identity: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Listing {
    project: String,
}

pub fn handle(
    db: &mut Connection,
    actor: &crate::Actor,
    action: &str,
    input: Value,
    now_ms: u64,
) -> Result<Value, String> {
    match action {
        "compile" => Ok(serde_json::to_value(compile_value(input)?).map_err(|e| e.to_string())?),
        "migrate" => {
            let input: MigrateInput = serde_json::from_value(input).map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(migrate(&input.yaml)?).map_err(|e| e.to_string())?)
        }
        "examples" => Ok(examples()),
        "review" => {
            let input: Publish = serde_json::from_value(input).map_err(|e| e.to_string())?;
            name(&input.project, "$.project")?;
            name(&input.name, "$.name")?;
            let plan = compile(&input.config)?;
            let next = input
                .expected_revision
                .checked_add(1)
                .filter(|v| *v <= i64::MAX as u64)
                .ok_or_else(|| "pipeline revision overflow".to_string())?;
            let next_sql = crate::sqlite_integer(next)?;
            let now = i64::try_from(now_ms).map_err(|_| "timestamp overflow")?;
            let transaction = db
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            crate::accounts::require_project(
                &transaction,
                actor,
                &input.project,
                &["owner", "admin"],
            )?;
            let current: Option<(u64, String)> = transaction.query_row("SELECT revision, identity FROM hosted_pipelines WHERE organization=?1 AND project=?2 AND name=?3", params![actor.organization, input.project, input.name], |row| Ok((crate::sqlite_u64(row, 0)?, row.get(1)?))).optional().map_err(|e| e.to_string())?;
            if current.as_ref().map_or(0, |v| v.0) != input.expected_revision {
                return Err("stale pipeline registry revision".into());
            }
            if current.is_none() {
                let rows: i64 = transaction
                    .query_row(
                        "SELECT COUNT(*) FROM hosted_pipelines WHERE organization=?1",
                        [&actor.organization],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                if rows >= MAX_REGISTRY_ROWS {
                    return Err("organization pipeline registry is full".into());
                }
            }
            transaction.execute("INSERT INTO hosted_pipelines(organization,project,name,revision,identity,config,plan,reviewer,reviewed_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(organization,project,name) DO UPDATE SET revision=excluded.revision,identity=excluded.identity,config=excluded.config,plan=excluded.plan,reviewer=excluded.reviewer,reviewed_ms=excluded.reviewed_ms", params![actor.organization, input.project, input.name, next_sql, plan.identity, serde_json::to_string(&input.config).map_err(|e| e.to_string())?, serde_json::to_string(&plan).map_err(|e| e.to_string())?, actor.peer, now]).map_err(|e| e.to_string())?;
            transaction.commit().map_err(|e| e.to_string())?;
            Ok(
                json!({"identity": plan.identity, "revision": next, "project":input.project, "name":input.name}),
            )
        }
        "get" => {
            let input: Lookup = serde_json::from_value(input).map_err(|e| e.to_string())?;
            Ok(
                serde_json::to_value(reviewed(db, actor, &input.project, &input.identity)?)
                    .map_err(|e| e.to_string())?,
            )
        }
        "list" => {
            let input: Listing = serde_json::from_value(input).map_err(|e| e.to_string())?;
            name(&input.project, "$.project")?;
            crate::accounts::require_project(
                db,
                actor,
                &input.project,
                &["owner", "admin", "member", "viewer", "billing"],
            )?;
            let mut statement = db.prepare("SELECT name,revision,identity,reviewer,reviewed_ms FROM hosted_pipelines WHERE organization=?1 AND project=?2 ORDER BY name LIMIT 1024").map_err(|e| e.to_string())?;
            let rows = statement.query_map(params![actor.organization, input.project], |row| Ok(json!({"name":row.get::<_,String>(0)?,"revision":crate::sqlite_u64(row,1)?,"identity":row.get::<_,String>(2)?,"reviewer":row.get::<_,String>(3)?,"reviewed_ms":crate::sqlite_u64(row,4)?}))).map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
            Ok(json!({"pipelines":rows}))
        }
        _ => Err(format!("unknown pipeline action {action}")),
    }
}

pub fn examples() -> Value {
    json!({"pipeline": {
        "schema":PIPELINE_SCHEMA,"name":"Repository checks","triggers":["manual","push"],
        "parameters":{"message":"hello"},
        "steps":{"greet":{"parameters":{"text":"hello"},"step":{"run":"printf '%s\\n' '${{ parameters.text }}'","shell":"bash"}}},
        "templates":{"check":{"runner":"linux-reviewed","steps":[{"kind":"use","step":"greet","parameters":{"text":"${{ parameters.message }}"}},{"kind":"run","run":"printf '%s\\n' '${{ matrix.version }}'","shell":"bash"}]}},
        "jobs":{"check":{"template":"check","matrix":{"version":["stable","next"]}},"finish":{"runner":"linux-reviewed","needs":["check"],"steps":[{"kind":"run","run":"printf 'checks succeeded\\n'","shell":"bash"}]}}
    },"supported_actions_subset":{"triggers":["push","pull_request","workflow_dispatch"],"trigger_options":false,"steps":"explicit bash or sh run steps only","expressions":"simple matrix.KEY only","matrix":"Cartesian literal scalar axes; explicit fail-fast: false; no include/exclude/max-parallel","dependencies":"all expanded needs must succeed","environment":"workflow < job < step literal string maps","unsupported":["uses/actions","permissions","if/conditions","services","containers","secrets/context expressions","outputs","runner file-command protocols","implicit shell","job display names"]}})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|error| unreachable!("unexpected failure: {error:?}"))
    }
    fn some<T>(value: Option<T>) -> T {
        value.unwrap_or_else(|| unreachable!("missing fixture value"))
    }
    fn error<T, E: std::fmt::Display>(result: Result<T, E>) -> String {
        match result {
            Err(error) => error.to_string(),
            Ok(_) => unreachable!("expected rejection"),
        }
    }
    fn example() -> Value {
        some(examples().get("pipeline").cloned())
    }
    fn fixture() -> Pipeline {
        ok(serde_json::from_value(example()))
    }
    fn set(config: &mut Value, path: &str, key: &str, value: Value) {
        some(config.pointer_mut(path).and_then(Value::as_object_mut)).insert(key.into(), value);
    }
    fn job(plan: &CompiledPipeline, index: usize) -> &CompiledJob {
        some(plan.jobs.get(index))
    }
    fn step(job: &CompiledJob, index: usize) -> &CompiledStep {
        some(job.steps.get(index))
    }

    // r[verify kiln.hosted.pipelines.scenario.compile]
    #[test]
    fn deterministic_templates_matrix_and_dependency_semantics() {
        let config = fixture();
        let first = ok(compile(&config));
        assert_eq!(first, ok(compile(&config)));
        assert_eq!(
            first.jobs.iter().map(|j| j.id.as_str()).collect::<Vec<_>>(),
            vec!["check[0]", "check[1]", "finish[0]"]
        );
        assert_eq!(job(&first, 2).needs, vec!["check[0]", "check[1]"]);
        assert_eq!(step(job(&first, 0), 0).run, "printf '%s\\n' 'hello'");
        assert_eq!(step(job(&first, 1), 1).run, "printf '%s\\n' 'next'");
        let mut changed = config;
        changed
            .parameters
            .insert("message".into(), "changed".into());
        assert_ne!(first.identity, ok(compile(&changed)).identity);
    }
    // r[verify kiln.hosted.pipelines.scenario.compile]
    #[test]
    fn cycle_overflow_missing_template_and_unknown_fields_fail() {
        let mut config = fixture();
        some(config.jobs.get_mut("check"))
            .needs
            .push("finish".into());
        assert!(error(compile(&config)).contains("cycle"));
        let mut config = fixture();
        some(config.jobs.get_mut("check")).matrix = (0..3)
            .map(|i| (format!("axis{i}"), (0..32).map(|v| v.to_string()).collect()))
            .collect();
        assert!(error(compile(&config)).contains("expansion"));
        let mut config = fixture();
        some(config.jobs.get_mut("check")).template = Some("missing".into());
        assert!(error(compile(&config)).contains("missing template"));
        for path in ["/jobs/finish", "/jobs/finish/steps/0"] {
            let mut config = example();
            set(&mut config, path, "permission", json!("write"));
            assert!(error(compile_value(config)).contains("unknown field"));
        }
    }
    // r[verify kiln.hosted.pipelines.scenario.compile]
    #[test]
    fn deadline_path_environment_and_expression_bounds_fail_closed() {
        let mut config = fixture();
        some(config.jobs.get_mut("finish")).timeout_ms = Some(MAX_TIMEOUT_MS + 1);
        assert!(error(compile(&config)).contains("timeout"));
        for (path, key, value, diagnostic) in [
            (
                "/jobs/finish/steps/0",
                "working_directory",
                json!("../outside"),
                "acquired repository",
            ),
            (
                "/jobs/finish",
                "env",
                json!({"BAD-NAME":"value"}),
                "environment variable",
            ),
            (
                "/jobs/finish/steps/0",
                "run",
                json!("echo '${{ secrets.TOKEN }}'"),
                "unsupported expression",
            ),
            (
                "/jobs/finish/steps/0",
                "run",
                json!("x".repeat(MAX_COMMAND_BYTES + 1)),
                "bound",
            ),
        ] {
            let mut config = example();
            set(&mut config, path, key, value);
            assert!(error(compile_value(config)).contains(diagnostic));
        }
        let overflow = ACTIONS.replace(
            "    needs: check",
            "    needs: check\n    timeout-minutes: 18446744073709551615",
        );
        assert!(error(migrate(&overflow)).contains("timeout-minutes"));
    }
    const ACTIONS: &str = "name: Checks\non: [push, workflow_dispatch]\nenv:\n  GLOBAL: literal\njobs:\n  check:\n    runs-on: linux-reviewed\n    strategy:\n      fail-fast: false\n      matrix:\n        version: [stable, next]\n    steps:\n      - run: echo '${{ matrix.version }}'\n        shell: bash\n        env:\n          GLOBAL: override\n  finish:\n    runs-on: linux-reviewed\n    needs: check\n    steps:\n      - run: echo done\n        shell: sh\n";
    #[test]
    fn migrated_shell_matrix_env_and_needs_are_usable() {
        let migrated = ok(migrate(ACTIONS));
        assert_eq!(
            job(&migrated.compiled, 2).needs,
            vec!["check[0]", "check[1]"]
        );
        assert_eq!(step(job(&migrated.compiled, 1), 0).run, "echo 'next'");
        assert_eq!(
            step(job(&migrated.compiled, 0), 0)
                .env
                .get("GLOBAL")
                .map(String::as_str),
            Some("override")
        );
        assert_eq!(migrated.compiled, ok(compile(&migrated.pipeline)));
    }
    // r[verify kiln.hosted.pipelines.scenario.compile]
    #[test]
    fn unsupported_actions_features_fail_with_paths() {
        for (input, path) in [
            (
                ACTIONS.replace("run: echo done", "uses: actions/checkout@v4"),
                ".uses",
            ),
            (
                ACTIONS.replace("echo done", "echo '${{ secrets.TOKEN }}'"),
                ".run",
            ),
            (
                ACTIONS.replace(
                    "    needs: check",
                    "    needs: check\n    permissions: write-all",
                ),
                ".permissions",
            ),
            (
                ACTIONS.replace("    needs: check", "    needs: check\n    if: success()"),
                ".if",
            ),
            (
                ACTIONS.replace("    needs: check", "    needs: check\n    services: {}"),
                ".services",
            ),
            (
                ACTIONS.replace("fail-fast: false", "fail-fast: true"),
                ".fail-fast",
            ),
            (ACTIONS.replace("echo done", "echo $GITHUB_OUTPUT"), ".run"),
        ] {
            assert!(error(migrate(&input)).contains(path), "{path}");
        }
    }
    #[test]
    fn review_is_scoped_revision_bound_and_recomputed() {
        let mut db = ok(Connection::open_in_memory());
        ok(init(&db));
        ok(crate::accounts::init(&db));
        let actor = crate::Actor {
            peer: "a".repeat(64),
            organization: "one".into(),
            role: "owner".into(),
        };
        ok(crate::accounts::bootstrap(
            &mut db,
            &actor.peer,
            &actor.organization,
        ));
        let input =
            json!({"project":"project","name":"checks","expected_revision":0,"config":example()});
        let saved = ok(handle(&mut db, &actor, "review", input.clone(), 1));
        let identity = some(saved.get("identity").and_then(Value::as_str));
        assert_eq!(
            ok(reviewed(&db, &actor, "project", identity)).identity,
            identity
        );
        assert!(error(handle(&mut db, &actor, "review", input, 2)).contains("stale"));
        let other = crate::Actor {
            organization: "two".into(),
            ..actor.clone()
        };
        assert!(reviewed(&db, &other, "project", identity).is_err());
        let scoped = crate::Actor {
            peer: "b".repeat(64),
            organization: "one".into(),
            role: "owner".into(),
        };
        ok(db.execute(
            "INSERT INTO hosted_memberships VALUES(?1,?2,'viewer','[\"other\"]',1,1,?3)",
            params![actor.organization, scoped.peer, actor.peer],
        ));
        assert!(error(reviewed(&db, &scoped, "project", identity)).contains("scope"));
        assert!(
            error(handle(
                &mut db,
                &scoped,
                "list",
                json!({"project":"project"}),
                2
            ))
            .contains("scope")
        );
        ok(db.execute(
            "UPDATE hosted_memberships SET projects='[\"project\"]' WHERE peer=?1",
            [&scoped.peer],
        ));
        assert_eq!(
            ok(reviewed(&db, &scoped, "project", identity)).identity,
            identity
        );
        assert!(handle(&mut db,&scoped,"review",json!({"project":"project","name":"other","expected_revision":0,"config":example()}),2).is_err());
        ok(db.execute("UPDATE hosted_pipelines SET config='{}'", []));
        assert!(reviewed(&db, &actor, "project", identity).is_err());
    }
}
