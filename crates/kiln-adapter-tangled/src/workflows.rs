//! Reviewed `.tangled/workflows` mapping.
//!
//! Tangled workflow files are executable configuration, so they are not run
//! verbatim. Like the Actions `migrate` subset, only explicitly reviewed fields
//! are mapped; every other field fails closed with a path-qualified diagnostic.

use std::collections::{BTreeMap, BTreeSet};

use kiln_codec::encode_and_identify;
use kiln_core::paths::valid_pattern;
use kiln_core::trigger_filter::admit_filter;
use kiln_core::types::ContentId;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::nsid;
use crate::profile::EngineBinding;
use crate::when::{EventKind, TriggerFilter, TriggerRule, pull_request_type};

pub const WORKFLOW_DIRECTORY: &str = ".tangled/workflows/";
pub const DEFINITION_SCHEMA: &str = "kiln.tangled-workflow.v1";
/// Matches the #13 trigger-filter bounds.
pub const MAXIMUM_FILTER_PATTERNS: usize = 32;
pub const MAXIMUM_FILTER_PATTERN_BYTES: usize = 256;
pub const MAXIMUM_CONDITIONS: usize = 32;
pub const MAXIMUM_STEPS: usize = 256;
pub const MAXIMUM_COMMAND_BYTES: usize = 32_768;
pub const MAXIMUM_ENVIRONMENT_ENTRIES: usize = 256;
pub const MAXIMUM_ENVIRONMENT_BYTES: usize = 4096;
pub const MAXIMUM_NAME_BYTES: usize = 128;
pub const MAXIMUM_DEPENDENCIES: usize = 256;
/// Deepest history a reviewed workflow may request.
pub const MAXIMUM_CLONE_DEPTH: u64 = 10_000;
/// Encoded definitions larger than this cannot be identified.
pub const MAXIMUM_DEFINITION_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloneOptions {
    pub skip: bool,
    /// Commits to fetch counting back from the triggering commit (at least 1).
    pub depth: u64,
    pub tags: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedStep {
    pub name: String,
    pub command: String,
    pub environment: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedWorkflow {
    /// File name, used as Tangled workflow id and name.
    pub id: String,
    /// Repository-relative path of the definition.
    pub path: String,
    /// BLAKE3 of the definition bytes, served as the workflow `hash`.
    pub source_blake3: String,
    pub engine: String,
    pub image: Option<String>,
    pub dependencies: Vec<String>,
    pub clone: CloneOptions,
    pub environment: BTreeMap<String, String>,
    pub steps: Vec<ReviewedStep>,
    pub triggers: Vec<TriggerRule>,
    /// Non-fatal mapping notes shown to the pusher.
    pub notes: Vec<String>,
}

impl ReviewedWorkflow {
    /// Event kinds as lexicon NSIDs, for `org.tangled.ci.defs#workflow`.
    #[must_use]
    pub fn trigger_nsids(&self) -> Vec<&'static str> {
        self.triggers
            .iter()
            .map(|rule| match rule.event {
                EventKind::Push => nsid::PUSH,
                EventKind::PullRequest => nsid::PULL_REQUEST,
                EventKind::Manual => nsid::MANUAL,
                EventKind::Schedule => nsid::SCHEDULE,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    #[must_use]
    pub fn manually_dispatchable(&self) -> bool {
        self.triggers
            .iter()
            .any(|rule| rule.event == EventKind::Manual)
    }
}

/// The Kiln identity of a reviewed definition at one exact commit.
pub fn definition_identity(
    repo_did: &str,
    commit: &str,
    workflow: &ReviewedWorkflow,
) -> Option<ContentId> {
    encode_and_identify(
        &(DEFINITION_SCHEMA, repo_did, commit, workflow),
        MAXIMUM_DEFINITION_BYTES,
    )
    .ok()
    .map(|(_, identity)| identity)
}

/// The workflow id of a definition path, or `None` outside the directory.
#[must_use]
pub fn workflow_id(path: &str) -> Option<&str> {
    let name = path.strip_prefix(WORKFLOW_DIRECTORY)?;
    let stem_ok = name
        .strip_suffix(".yml")
        .or_else(|| name.strip_suffix(".yaml"))
        .is_some_and(|stem| !stem.is_empty());
    (stem_ok
        && name.len() <= MAXIMUM_NAME_BYTES
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')))
    .then_some(name)
}

fn fail(path: &str, message: &str) -> String {
    format!("{path}: {message}")
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| fail(path, "expected a mapping"))
}

/// Fields refused with a reason, so authors see why rather than "unknown".
fn refused_reason(key: &str) -> Option<&'static str> {
    Some(match key {
        "cache" => "caches are not mapped; Kiln run inputs are exact sources only",
        "runs_on" => "executor labels need an explicit Kiln runner mapping",
        "audience" => "OIDC ID tokens are not issued by Kiln for Tangled runs",
        "registry" | "substituters" => {
            "Nix registries and substituters belong to the reviewed engine binding"
        }
        "services" | "virtualisation" => "guest services need an explicit reviewed engine binding",
        _ => return None,
    })
}

fn fields(map: &Map<String, Value>, allowed: &[&str], path: &str) -> Result<(), String> {
    for key in map.keys() {
        if !allowed.contains(&key.as_str()) {
            let field = format!("{path}.{key}");
            return Err(fail(
                &field,
                refused_reason(key).unwrap_or(
                    "unsupported Tangled workflow field; it has no reviewed Kiln mapping",
                ),
            ));
        }
    }
    Ok(())
}

fn text(value: &Value, path: &str, maximum: usize) -> Result<String, String> {
    match value.as_str() {
        Some(text) if !text.is_empty() && text.len() <= maximum && !text.contains('\0') => {
            Ok(text.to_string())
        }
        _ => Err(fail(path, "expected a bounded non-empty string")),
    }
}

fn string_list(value: &Value, path: &str, maximum: usize) -> Result<Vec<String>, String> {
    let items: Vec<&Value> = match value {
        Value::String(_) => vec![value],
        Value::Array(items) if !items.is_empty() && items.len() <= maximum => {
            items.iter().collect()
        }
        _ => {
            return Err(fail(
                path,
                "expected a string or a bounded non-empty list of strings",
            ));
        }
    };
    items
        .into_iter()
        .map(|item| text(item, path, MAXIMUM_FILTER_PATTERN_BYTES))
        .collect()
}

fn patterns(value: &Value, path: &str) -> Result<Vec<String>, String> {
    let values = string_list(value, path, MAXIMUM_FILTER_PATTERNS)?;
    for pattern in &values {
        // `?` has no documented Tangled meaning, and `**` inside a segment
        // (`release-**`) differs from Kiln's whole-segment `**`.
        if !valid_pattern(pattern) || pattern.contains('?') {
            return Err(fail(
                path,
                "pattern uses glob syntax without an identical Kiln meaning (use * within a segment and whole-segment **)",
            ));
        }
    }
    Ok(values)
}

fn environment(value: Option<&Value>, path: &str) -> Result<BTreeMap<String, String>, String> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let map = object(value, path)?;
    if map.len() > MAXIMUM_ENVIRONMENT_ENTRIES {
        return Err(fail(path, "too many environment variables"));
    }
    let mut out = BTreeMap::new();
    for (name, value) in map {
        let valid_name = name.len() <= MAXIMUM_NAME_BYTES
            && name
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        let literal = match value {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            Value::Bool(flag) => flag.to_string(),
            _ => String::new(),
        };
        if !valid_name
            || literal.len() > MAXIMUM_ENVIRONMENT_BYTES
            || literal.contains('\0')
            || !value_is_scalar(value)
        {
            return Err(fail(
                &format!("{path}.{name}"),
                "environment entries must be literal scalars with shell variable names",
            ));
        }
        out.insert(name.clone(), literal);
    }
    Ok(out)
}

fn value_is_scalar(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))
}

fn events(value: &Value, path: &str) -> Result<BTreeSet<EventKind>, String> {
    let names = string_list(value, path, MAXIMUM_CONDITIONS)?;
    let mut kinds = BTreeSet::new();
    for name in names {
        let kind = match name.as_str() {
            "push" => EventKind::Push,
            "pull_request" => EventKind::PullRequest,
            "manual" => EventKind::Manual,
            "schedule" => {
                return Err(fail(
                    path,
                    "schedule triggers are not supported: Tangled hashes H fields from the repository DID and file name, so Kiln schedules would fire at different minutes",
                ));
            }
            _ => return Err(fail(path, "event must be push, pull_request or manual")),
        };
        kinds.insert(kind);
    }
    Ok(kinds)
}

// r[impl kiln.tangled.workflows]
fn triggers(value: Option<&Value>) -> Result<Vec<TriggerRule>, String> {
    let Some(value) = value else {
        // Without `when`, Tangled runs the workflow for every event.
        return Ok([EventKind::Push, EventKind::PullRequest, EventKind::Manual]
            .into_iter()
            .map(|event| TriggerRule {
                event,
                filter: TriggerFilter::default(),
            })
            .collect());
    };
    let conditions: Vec<&Value> = match value {
        Value::Array(items) if !items.is_empty() && items.len() <= MAXIMUM_CONDITIONS => {
            items.iter().collect()
        }
        _ => {
            return Err(fail(
                "$.when",
                "expected a bounded non-empty list of conditions",
            ));
        }
    };
    let mut rules = Vec::new();
    for (index, condition) in conditions.into_iter().enumerate() {
        let path = format!("$.when[{index}]");
        let map = object(condition, &path)?;
        if map.contains_key("schedule") {
            return Err(fail(
                &format!("{path}.schedule"),
                "schedule triggers are not supported",
            ));
        }
        fields(map, &["event", "branch", "tag", "types", "paths"], &path)?;
        let kinds = events(
            map.get("event")
                .ok_or_else(|| fail(&format!("{path}.event"), "required"))?,
            &format!("{path}.event"),
        )?;
        let branches = map
            .get("branch")
            .map(|v| patterns(v, &format!("{path}.branch")))
            .transpose()?
            .unwrap_or_default();
        let tags = map
            .get("tag")
            .map(|v| patterns(v, &format!("{path}.tag")))
            .transpose()?
            .unwrap_or_default();
        let paths = map
            .get("paths")
            .map(|v| patterns(v, &format!("{path}.paths")))
            .transpose()?
            .unwrap_or_default();
        let types = map
            .get("types")
            .map(|value| {
                string_list(value, &format!("{path}.types"), MAXIMUM_CONDITIONS)?
                    .iter()
                    .map(|name| {
                        pull_request_type(name).ok_or_else(|| {
                            fail(
                                &format!("{path}.types"),
                                "types must be opened, reopened, synchronize, closed or merged",
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        let non_push = kinds.iter().any(|kind| *kind != EventKind::Push);
        let non_pull = kinds.iter().any(|kind| *kind != EventKind::PullRequest);
        if !tags.is_empty() && non_push {
            return Err(fail(&format!("{path}.tag"), "tag applies only to push"));
        }
        if !types.is_empty() && non_pull {
            return Err(fail(
                &format!("{path}.types"),
                "types applies only to pull_request",
            ));
        }
        if kinds.contains(&EventKind::Push) && branches.is_empty() && tags.is_empty() {
            return Err(fail(&path, "push conditions need branch or tag"));
        }
        let filtered = !branches.is_empty() || !paths.is_empty();
        if kinds.len() == 1 && kinds.contains(&EventKind::Manual) && filtered {
            return Err(fail(
                &path,
                "manual runs ignore branch and paths; remove them",
            ));
        }
        for event in kinds {
            let filter = match event {
                // Tangled does not evaluate `when` filters for manual runs;
                // schedule conditions were refused above.
                EventKind::Manual | EventKind::Schedule => TriggerFilter::default(),
                EventKind::Push => TriggerFilter {
                    branches: branches.clone(),
                    tags: tags.clone(),
                    types: Vec::new(),
                    paths: paths.clone(),
                },
                EventKind::PullRequest => TriggerFilter {
                    branches: branches.clone(),
                    tags: Vec::new(),
                    types: types.clone(),
                    paths: paths.clone(),
                },
            };
            admit_filter(event, &filter).map_err(|error| {
                fail(&format!("{path}.{}", error.field.as_str()), error.message)
            })?;
            rules.push(TriggerRule { event, filter });
        }
    }
    Ok(rules)
}

fn dependencies(value: Option<&Value>, binding: &EngineBinding) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let names = match value {
        Value::Array(_) => string_list(value, "$.dependencies", MAXIMUM_DEPENDENCIES)?,
        Value::Object(map) => {
            // nixery form: registry → packages; only the nixpkgs registry is reviewed.
            fields(map, &["nixpkgs"], "$.dependencies")?;
            match map.get("nixpkgs") {
                Some(list) => string_list(list, "$.dependencies.nixpkgs", MAXIMUM_DEPENDENCIES)?,
                None => Vec::new(),
            }
        }
        _ => {
            return Err(fail(
                "$.dependencies",
                "expected a list of packages or a nixpkgs registry map",
            ));
        }
    };
    for name in &names {
        if !binding.packages.contains(name) {
            return Err(fail(
                "$.dependencies",
                "dependency is not provided by the reviewed engine binding",
            ));
        }
    }
    let unique: BTreeSet<String> = names.into_iter().collect();
    Ok(unique.into_iter().collect())
}

fn clone_options(value: Option<&Value>, notes: &mut Vec<String>) -> Result<CloneOptions, String> {
    let mut options = CloneOptions {
        skip: false,
        depth: 1,
        tags: true,
    };
    let Some(value) = value else {
        notes.push(
            "clone.submodules defaults to true in Tangled; Kiln fetches no submodules".into(),
        );
        return Ok(options);
    };
    let map = object(value, "$.clone")?;
    fields(map, &["skip", "depth", "submodules", "tags"], "$.clone")?;
    if let Some(skip) = map.get("skip") {
        options.skip = skip
            .as_bool()
            .ok_or_else(|| fail("$.clone.skip", "expected a boolean"))?;
    }
    if let Some(depth) = map.get("depth") {
        let depth = depth
            .as_u64()
            .filter(|depth| *depth <= MAXIMUM_CLONE_DEPTH)
            .ok_or_else(|| fail("$.clone.depth", "expected a bounded non-negative integer"))?;
        // Tangled: 0 or unset fetches one commit.
        options.depth = depth.max(1);
    }
    if let Some(tags) = map.get("tags") {
        options.tags = tags
            .as_bool()
            .ok_or_else(|| fail("$.clone.tags", "expected a boolean"))?;
    }
    match map.get("submodules") {
        Some(Value::Bool(false)) => {}
        Some(Value::Bool(true)) => {
            return Err(fail(
                "$.clone.submodules",
                "submodules are not fetched by Kiln exact acquisition; set false",
            ));
        }
        Some(_) => return Err(fail("$.clone.submodules", "expected a boolean")),
        None if !options.skip => {
            notes.push(
                "clone.submodules defaults to true in Tangled; Kiln fetches no submodules".into(),
            );
        }
        None => {}
    }
    Ok(options)
}

fn steps(value: Option<&Value>) -> Result<Vec<ReviewedStep>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .filter(|items| items.len() <= MAXIMUM_STEPS)
        .ok_or_else(|| fail("$.steps", "expected a bounded list of steps"))?;
    items
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let path = format!("$.steps[{index}]");
            let map = object(step, &path)?;
            fields(map, &["name", "command", "environment"], &path)?;
            Ok(ReviewedStep {
                name: text(
                    map.get("name")
                        .ok_or_else(|| fail(&format!("{path}.name"), "required"))?,
                    &format!("{path}.name"),
                    MAXIMUM_NAME_BYTES,
                )?,
                command: text(
                    map.get("command")
                        .ok_or_else(|| fail(&format!("{path}.command"), "required"))?,
                    &format!("{path}.command"),
                    MAXIMUM_COMMAND_BYTES,
                )?,
                environment: environment(map.get("environment"), &format!("{path}.environment"))?,
            })
        })
        .collect()
}

// r[impl kiln.tangled.workflows]
/// Map one workflow file onto the reviewed subset.
pub fn review(
    path: &str,
    bytes: &[u8],
    engines: &[EngineBinding],
    maximum_bytes: usize,
) -> Result<ReviewedWorkflow, String> {
    let id = workflow_id(path).ok_or_else(|| fail(path, "not a workflow definition path"))?;
    if bytes.is_empty() || bytes.len() > maximum_bytes {
        return Err(fail(path, "definition is empty or exceeds its bound"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| fail(path, "definition is not UTF-8"))?;
    let parsed: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|error| fail(path, &format!("YAML: {error}")))?;
    let source = serde_json::to_value(parsed)
        .map_err(|_| fail(path, "YAML has no JSON-compatible meaning"))?;
    let map = object(&source, "$").map_err(|message| format!("{path} {message}"))?;
    let qualify = |message: String| format!("{path} {message}");
    fields(
        map,
        &[
            "when",
            "engine",
            "image",
            "dependencies",
            "clone",
            "environment",
            "steps",
        ],
        "$",
    )
    .map_err(qualify)?;
    let engine = text_field(map.get("engine"), "$.engine").map_err(qualify)?;
    let binding = engines
        .iter()
        .find(|binding| binding.engine == engine)
        .ok_or_else(|| {
            qualify(fail(
                "$.engine",
                "engine is not bound by the reviewed profile",
            ))
        })?;
    let image = map
        .get("image")
        .map(|value| self::text(value, "$.image", MAXIMUM_NAME_BYTES))
        .transpose()
        .map_err(qualify)?;
    if let Some(image) = &image {
        if !binding.images.contains(image) {
            return Err(qualify(fail(
                "$.image",
                "image is not provided by the reviewed engine binding",
            )));
        }
    }
    let mut notes = Vec::new();
    Ok(ReviewedWorkflow {
        id: id.to_string(),
        path: path.to_string(),
        source_blake3: blake3::hash(bytes).to_hex().to_string(),
        engine,
        image,
        dependencies: dependencies(map.get("dependencies"), binding).map_err(qualify)?,
        clone: clone_options(map.get("clone"), &mut notes).map_err(qualify)?,
        environment: environment(map.get("environment"), "$.environment").map_err(qualify)?,
        steps: steps(map.get("steps")).map_err(qualify)?,
        triggers: triggers(map.get("when")).map_err(qualify)?,
        notes,
    })
}

fn text_field(value: Option<&Value>, path: &str) -> Result<String, String> {
    text(
        value.ok_or_else(|| fail(path, "required"))?,
        path,
        MAXIMUM_NAME_BYTES,
    )
}
