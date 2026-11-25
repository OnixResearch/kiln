//! Reviewed runbook registry, parameter validation, target resolution, and RBAC grants.

#[path = "runbooks/secrets.rs"]
mod secrets;

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use kiln_adapter_github::profile::{ConfigEdge, ConfigNode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::access::Principal;
use crate::engine::Role;
use crate::profile::{ProjectProfile, ServiceProfile};

pub const RUNBOOK_REGISTRY_SCHEMA: &str = "kiln.runbook-registry.v1";
pub const RUNBOOKS_VIEW_SCHEMA: &str = "kiln.web-runbooks-view.v1";
pub const RUNBOOK_DISPATCH_RESULT_SCHEMA: &str = "kiln.runbook-dispatch-result.v1";
pub const PARAM_ENV_PREFIX: &str = "LATTICE_PARAM_";

pub const MAX_PROFILE_BYTES: usize = 1_048_576;
pub const MAX_RUNBOOKS: usize = 64;
pub const MAX_PARAMS: usize = 16;
pub const MAX_PARAM_BINDINGS: usize = 8;
pub const MAX_ENUM_VALUES: usize = 32;
pub const MAX_TARGETS: usize = 64;
pub const MAX_TARGET_REFS: usize = 64;
pub const MAX_GRANTS: usize = 128;
pub const MAX_GRANT_RUNBOOKS: usize = 64;
pub const MAX_NOTIFICATIONS: usize = 32;
pub const MAX_RELATIVE_PATH_BYTES: usize = 256;
pub const MAX_NODES: usize = 32;
pub const MAX_EDGES: usize = 64;
pub const MAX_PARAM_ENV_VALUE_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunbookMode {
    ReadOnly,
    Mutating,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookRegistry {
    pub schema: String,
    pub runbooks: Vec<RunbookDefinition>,
    #[serde(default)]
    pub grants: Vec<RunbookGrant>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notifications: Vec<RunbookNotificationProfile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookNotificationProfile {
    pub id: String,
    pub enabled: bool,
    #[serde(default)]
    pub runbooks: Vec<String>,
    pub on_success: bool,
    pub on_failure: bool,
    #[serde(default = "default_true")]
    pub on_unknown: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub addresses: Vec<IpAddr>,
    pub bearer_secret_file: Option<PathBuf>,
    pub ca_certificate_file: Option<PathBuf>,
    pub allow_loopback: bool,
    pub allow_private: bool,
    pub timeout_ms: u64,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_resolver_candidates: usize,
}

impl RunbookNotificationProfile {
    #[must_use]
    pub fn applies_to(&self, runbook_id: &str) -> bool {
        self.runbooks.is_empty() || self.runbooks.iter().any(|id| id == runbook_id)
    }

    #[must_use]
    pub fn to_automation_profile(&self) -> crate::automation::NotificationProfile {
        crate::automation::NotificationProfile {
            id: self.id.clone(),
            project_id: "runbooks".to_owned(),
            enabled: self.enabled,
            on_success: self.on_success,
            on_failure: self.on_failure,
            host: self.host.clone(),
            port: self.port,
            path: self.path.clone(),
            addresses: self.addresses.clone(),
            bearer_secret_file: self.bearer_secret_file.clone(),
            ca_certificate_file: self.ca_certificate_file.clone(),
            allow_loopback: self.allow_loopback,
            allow_private: self.allow_private,
            timeout_ms: self.timeout_ms,
            max_request_bytes: self.max_request_bytes,
            max_response_bytes: self.max_response_bytes,
            max_resolver_candidates: self.max_resolver_candidates,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookFanoutConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_one_u16")]
    pub default_max_concurrency: u16,
    #[serde(default = "default_max_concurrency_limit")]
    pub max_concurrency_limit: u16,
    #[serde(default = "default_budget_cap")]
    pub default_budget_cap: u64,
    #[serde(default = "default_max_budget_cap")]
    pub max_budget_cap: u64,
    #[serde(default = "default_max_targets")]
    pub max_targets: usize,
    #[serde(default)]
    pub write_scopes: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_one_u16() -> u16 {
    1
}

fn default_max_concurrency_limit() -> u16 {
    8
}

fn default_budget_cap() -> u64 {
    16
}

fn default_max_budget_cap() -> u64 {
    256
}

fn default_max_targets() -> usize {
    kiln_core::implementation::MAX_FANOUT_TARGETS
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookDefinition {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub mode: RunbookMode,
    pub lattice: RunbookLattice,
    #[serde(default)]
    pub params: Vec<ParamDeclaration>,
    pub allowed_targets: Vec<RunbookTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fanout: Option<RunbookFanoutConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookLattice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    pub bundle_identity: String,
    pub workflow_name: String,
    pub workflow_revision: String,
    pub nodes: Vec<ConfigNode>,
    #[serde(default)]
    pub edges: Vec<ConfigEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamDeclaration {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: ParamType,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<ParamValue>,
    pub bindings: Vec<ParamBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParamType {
    Bool {},
    Integer { min: i64, max: i64 },
    Enum { values: Vec<String> },
    RelativePath { max_bytes: usize },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Bool(bool),
    Integer(i64),
    Text(String),
}

impl ParamValue {
    pub fn render(&self) -> String {
        match self {
            Self::Bool(v) => v.to_string(),
            Self::Integer(v) => v.to_string(),
            Self::Text(v) => v.clone(),
        }
    }

    pub fn kind_tag(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Integer(_) => "integer",
            Self::Text(_) => "text",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamBinding {
    pub node: String,
    pub shell_env: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookTarget {
    pub repository: String,
    pub project_id: String,
    pub references: Vec<String>,
    pub allow_commit_sha: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookGrant {
    pub principal: String,
    pub runbooks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_after_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunbookRunBinding {
    pub runbook_id: String,
    pub mode: RunbookMode,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lattice_bundle_id: Option<String>,
    pub lattice_bundle_identity: String,
    pub lattice_workflow_name: String,
    pub lattice_workflow_revision: String,
    pub params_identity: String,
    pub params: BTreeMap<String, ParamValue>,
    pub env_bindings: BTreeMap<String, BTreeMap<String, String>>,
    pub source_env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchRunbookRequest {
    pub request_id: String,
    pub target: String,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunbookListResponse {
    pub schema: &'static str,
    pub configured: bool,
    pub principal: String,
    pub runbooks: Vec<RunbookSummaryView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunbookSummaryView {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub mode: RunbookMode,
    pub lattice_bundle_id: Option<String>,
    pub lattice_bundle_identity: String,
    pub lattice_workflow_name: String,
    pub lattice_workflow_revision: String,
    pub params: Vec<RunbookParamView>,
    pub allowed_targets: Vec<RunbookTargetView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fanout: Option<RunbookFanoutConfig>,
    pub can_dispatch: bool,
    pub dispatch_refusal: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunbookParamView {
    pub name: String,
    pub kind: &'static str,
    pub required: bool,
    pub default: Option<ParamValue>,
    pub enum_values: Option<Vec<String>>,
    pub integer_min: Option<i64>,
    pub integer_max: Option<i64>,
    pub relative_path_max_bytes: Option<usize>,
    pub bindings: Vec<ParamBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunbookTargetView {
    pub repository: String,
    pub project_id: String,
    pub references: Vec<String>,
    pub allow_commit_sha: bool,
}

#[derive(Debug, Serialize)]
pub struct RunbookDispatchResponse {
    pub schema: &'static str,
    pub request_id: String,
    pub replayed: bool,
    pub runbook_id: String,
    pub mode: RunbookMode,
    pub target: String,
    pub project_id: String,
    pub source_revision: String,
    pub lattice_bundle_id: Option<String>,
    pub lattice_bundle_identity: String,
    pub lattice_workflow_name: String,
    pub lattice_workflow_revision: String,
    pub params_identity: String,
    pub params: BTreeMap<String, ParamValue>,
    pub env_bindings: BTreeMap<String, BTreeMap<String, String>>,
    pub source_env: BTreeMap<String, String>,
    pub run_id: String,
    pub run: crate::engine::model::RunView,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedParams {
    pub identity: String,
    pub values: BTreeMap<String, ParamValue>,
    pub env_bindings: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParamError {
    pub code: &'static str,
}

pub fn valid_runbook_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub fn valid_param_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub fn valid_param_env_key(key: &str) -> bool {
    let Some(suffix) = key.strip_prefix(PARAM_ENV_PREFIX) else {
        return false;
    };
    key.len() <= 64
        && !suffix.is_empty()
        && suffix
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

pub fn valid_write_scope_path(path: &str) -> bool {
    path == "."
        || (!path.is_empty()
            && path.len() <= 256
            && path.bytes().all(|b| b.is_ascii_graphic())
            && kiln_core::paths::canonical(path))
}

pub fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'))
        && path
            .split('/')
            .all(|c| !c.is_empty() && c != ".." && !c.starts_with('-'))
}

pub fn is_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn is_hex40(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn valid_display_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.trim().is_empty()
        && !value.bytes().any(|b| b.is_ascii_control())
}

fn valid_description_text(value: &str) -> bool {
    value.len() <= 512 && !value.bytes().any(|b| b.is_ascii_control())
}

fn valid_lattice_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b':' | b'-')
        })
}

fn valid_enum_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 64
        && token
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && token.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}

fn valid_principal(principal: &str) -> bool {
    !principal.is_empty()
        && principal.len() <= 256
        && !principal
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control() || b == b'*')
}

fn valid_repository_name(repo: &str) -> bool {
    !repo.is_empty()
        && repo.len() <= 256
        && !repo
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control() || b == b'*' || b == b'@')
}

fn valid_reference(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= 1024
        && !reference.starts_with('-')
        && !reference
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
}

fn is_dag(nodes: &[ConfigNode], edges: &[ConfigEdge]) -> bool {
    let mut in_degree: BTreeMap<&str, usize> = nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in edges {
        if edge.source == edge.target
            || !in_degree.contains_key(edge.source.as_str())
            || !in_degree.contains_key(edge.target.as_str())
        {
            return false;
        }
        if let Some(count) = in_degree.get_mut(edge.target.as_str()) {
            *count += 1;
        }
        adjacency
            .entry(edge.source.as_str())
            .or_default()
            .push(edge.target.as_str());
    }
    let mut ready: Vec<&str> = in_degree
        .iter()
        .filter_map(|(id, deg)| (*deg == 0).then_some(*id))
        .collect();
    let mut visited = 0usize;
    while let Some(node) = ready.pop() {
        visited += 1;
        if let Some(targets) = adjacency.get(node) {
            for target in targets {
                if let Some(deg) = in_degree.get_mut(target) {
                    *deg -= 1;
                    if *deg == 0 {
                        ready.push(target);
                    }
                }
            }
        }
    }
    visited == nodes.len()
}

fn check_param_value_bounds(
    param_type: &ParamType,
    value: &ParamValue,
) -> Result<(), &'static str> {
    match (param_type, value) {
        (ParamType::Bool {}, ParamValue::Bool(_)) => Ok(()),
        (ParamType::Integer { min, max }, ParamValue::Integer(v)) if (*min..=*max).contains(v) => {
            Ok(())
        }
        (ParamType::Enum { values }, ParamValue::Text(v)) if values.iter().any(|x| x == v) => {
            Ok(())
        }
        (ParamType::RelativePath { max_bytes }, ParamValue::Text(v))
            if v.len() <= *max_bytes && valid_relative_path(v) =>
        {
            Ok(())
        }
        _ => Err("runbook_param_invalid"),
    }
}

impl RunbookRegistry {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, MAX_PROFILE_BYTES, false)?;
        let registry: Self =
            serde_json::from_slice(&bytes).map_err(|_| "runbook registry JSON invalid")?;
        registry.admit()?;
        Ok(registry)
    }

    pub fn admit(&self) -> Result<(), String> {
        if self.schema != RUNBOOK_REGISTRY_SCHEMA {
            return Err("runbook registry schema invalid".into());
        }
        if self.runbooks.is_empty() || self.runbooks.len() > MAX_RUNBOOKS {
            return Err("runbook registry count outside bounds".into());
        }
        if self.grants.len() > MAX_GRANTS {
            return Err("runbook registry grants exceed bound".into());
        }

        let mut runbook_ids = BTreeSet::new();
        for runbook in &self.runbooks {
            if !valid_runbook_id(&runbook.id) || !runbook_ids.insert(runbook.id.as_str()) {
                return Err("runbook registry id invalid or duplicated".into());
            }
            if !valid_display_text(&runbook.display_name)
                || !valid_description_text(&runbook.description)
            {
                return Err("runbook registry display metadata invalid".into());
            }
            if let Some(bundle_id) = &runbook.lattice.bundle_id
                && !valid_runbook_id(bundle_id)
            {
                return Err("runbook lattice bundle_id invalid".into());
            }
            if !is_hex64(&runbook.lattice.bundle_identity)
                || !is_hex64(&runbook.lattice.workflow_revision)
                || !valid_lattice_label(&runbook.lattice.workflow_name)
            {
                return Err("runbook lattice identity or workflow name invalid".into());
            }
            if runbook.lattice.nodes.is_empty()
                || runbook.lattice.nodes.len() > MAX_NODES
                || runbook.lattice.edges.len() > MAX_EDGES
            {
                return Err("runbook lattice graph outside bounds".into());
            }
            let mut node_ids = BTreeSet::new();
            for node in &runbook.lattice.nodes {
                if !valid_lattice_label(&node.id)
                    || !node_ids.insert(node.id.as_str())
                    || !matches!(node.kind.as_str(), "build" | "review" | "test")
                    || !matches!(node.adapter_kind.as_str(), "local" | "remote")
                    || !is_hex64(&node.operation_identity)
                {
                    return Err("runbook lattice node invalid".into());
                }
            }
            let mut edge_keys = BTreeSet::new();
            for edge in &runbook.lattice.edges {
                if !edge_keys.insert((edge.source.as_str(), edge.target.as_str())) {
                    return Err("runbook lattice edge duplicated".into());
                }
            }
            if !is_dag(&runbook.lattice.nodes, &runbook.lattice.edges) {
                return Err("runbook lattice graph invalid or cyclic".into());
            }

            if runbook.params.len() > MAX_PARAMS {
                return Err("runbook params exceed bound".into());
            }
            let mut param_names = BTreeSet::new();
            let mut binding_keys = BTreeSet::new();
            for param in &runbook.params {
                if !valid_param_name(&param.name) || !param_names.insert(param.name.as_str()) {
                    return Err("runbook param name invalid or duplicated".into());
                }
                match &param.param_type {
                    ParamType::Bool {} => {}
                    ParamType::Integer { min, max } => {
                        if min > max {
                            return Err("runbook integer param bounds invalid".into());
                        }
                    }
                    ParamType::Enum { values } => {
                        let mut unique = BTreeSet::new();
                        if values.is_empty()
                            || values.len() > MAX_ENUM_VALUES
                            || values
                                .iter()
                                .any(|v| !valid_enum_token(v) || !unique.insert(v.as_str()))
                        {
                            return Err("runbook enum param values invalid".into());
                        }
                    }
                    ParamType::RelativePath { max_bytes } => {
                        if *max_bytes == 0 || *max_bytes > MAX_RELATIVE_PATH_BYTES {
                            return Err("runbook relative_path max_bytes invalid".into());
                        }
                    }
                }
                match (param.required, &param.default) {
                    (true, None) => {}
                    (false, Some(default)) => {
                        check_param_value_bounds(&param.param_type, default)
                            .map_err(|_| "runbook param default invalid")?;
                        if matches!(param.param_type, ParamType::RelativePath { .. })
                            && let ParamValue::Text(text) = default
                            && secrets::is_secret_value(text)
                        {
                            return Err("runbook param default looks secret-like".into());
                        }
                    }
                    _ => return Err("runbook param required/default invariant violated".into()),
                }
                if param.bindings.is_empty() || param.bindings.len() > MAX_PARAM_BINDINGS {
                    return Err("runbook param bindings outside bounds".into());
                }
                for binding in &param.bindings {
                    if !node_ids.contains(binding.node.as_str())
                        || !valid_param_env_key(&binding.shell_env)
                        || !binding_keys.insert((binding.node.as_str(), binding.shell_env.as_str()))
                    {
                        return Err("runbook param binding invalid or duplicated".into());
                    }
                }
                if matches!(param.param_type, ParamType::RelativePath { .. }) {
                    let bad_name = secrets::is_secret_identifier(&param.name)
                        || param.bindings.iter().any(|b| {
                            b.shell_env
                                .strip_prefix(PARAM_ENV_PREFIX)
                                .is_some_and(secrets::is_secret_identifier)
                        });
                    if bad_name {
                        return Err("runbook relative_path param name looks secret-like".into());
                    }
                }
            }

            if runbook.allowed_targets.is_empty() || runbook.allowed_targets.len() > MAX_TARGETS {
                return Err("runbook allowed_targets outside bounds".into());
            }
            let mut target_repos = BTreeSet::new();
            let mut target_projects = BTreeSet::new();
            for target in &runbook.allowed_targets {
                if !valid_repository_name(&target.repository)
                    || !target_repos.insert(target.repository.as_str())
                    || !crate::profile::identifier(&target.project_id)
                    || !target_projects.insert(target.project_id.as_str())
                {
                    return Err("runbook allowed_target repository or project_id invalid".into());
                }
                if target.references.is_empty() || target.references.len() > MAX_TARGET_REFS {
                    return Err("runbook target references outside bounds".into());
                }
                let mut refs = BTreeSet::new();
                for reference in &target.references {
                    if !valid_reference(reference) || !refs.insert(reference.as_str()) {
                        return Err("runbook target reference invalid or duplicated".into());
                    }
                }
            }
            if let Some(fanout) = &runbook.fanout {
                if fanout.default_max_concurrency == 0
                    || fanout.default_max_concurrency > fanout.max_concurrency_limit
                    || fanout.max_concurrency_limit
                        > kiln_core::implementation::MAX_FANOUT_CONCURRENCY
                {
                    return Err("runbook fanout concurrency bounds invalid".into());
                }
                if fanout.default_budget_cap == 0
                    || fanout.default_budget_cap > fanout.max_budget_cap
                    || fanout.max_budget_cap > kiln_core::implementation::MAX_FANOUT_BUDGET_UNITS
                {
                    return Err("runbook fanout budget bounds invalid".into());
                }
                if fanout.max_targets == 0
                    || fanout.max_targets > kiln_core::implementation::MAX_FANOUT_TARGETS
                {
                    return Err("runbook fanout max_targets bound invalid".into());
                }
                if fanout.write_scopes.len() > 32 {
                    return Err("runbook fanout write_scopes exceeds bound".into());
                }
                if runbook.mode == RunbookMode::ReadOnly && !fanout.write_scopes.is_empty() {
                    return Err("read_only runbook cannot declare fanout write_scopes".into());
                }
                let mut seen_scopes = BTreeSet::new();
                for scope in &fanout.write_scopes {
                    if !valid_write_scope_path(scope) || !seen_scopes.insert(scope.as_str()) {
                        return Err("runbook fanout write_scope invalid or duplicated".into());
                    }
                }
            }
        }

        let mut principals = BTreeSet::new();
        for grant in &self.grants {
            if !valid_principal(&grant.principal) || !principals.insert(grant.principal.as_str()) {
                return Err("runbook grant principal invalid or duplicated".into());
            }
            if grant.runbooks.is_empty() || grant.runbooks.len() > MAX_GRANT_RUNBOOKS {
                return Err("runbook grant runbooks outside bounds".into());
            }
            if grant.not_after_ms == Some(0) {
                return Err("runbook grant not_after_ms must be positive".into());
            }
            let mut granted = BTreeSet::new();
            for id in &grant.runbooks {
                if !runbook_ids.contains(id.as_str()) || !granted.insert(id.as_str()) {
                    return Err("runbook grant references unknown or duplicate runbook".into());
                }
            }
        }

        if self.notifications.len() > MAX_NOTIFICATIONS {
            return Err("runbook registry notifications exceed bound".into());
        }
        let mut notification_ids = BTreeSet::new();
        for notification in &self.notifications {
            if !crate::profile::identifier(&notification.id)
                || !notification_ids.insert(notification.id.as_str())
            {
                return Err("runbook notification id invalid or duplicated".into());
            }
            if notification.runbooks.len() > MAX_RUNBOOKS {
                return Err("runbook notification runbooks exceed bound".into());
            }
            let mut scoped = BTreeSet::new();
            for rb_id in &notification.runbooks {
                if !valid_runbook_id(rb_id)
                    || !runbook_ids.contains(rb_id.as_str())
                    || !scoped.insert(rb_id.as_str())
                {
                    return Err(
                        "runbook notification references unknown or duplicate runbook".into(),
                    );
                }
            }
            let profile = notification.to_automation_profile();
            let webhook = profile.webhook()?;
            kiln_adapter_webhook::admit_addresses(&webhook, &notification.addresses)
                .map_err(|_| "runbook notification network addresses denied")?;
            let canonical: Vec<_> = notification
                .addresses
                .iter()
                .map(|address| match address {
                    IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(*address),
                    _ => *address,
                })
                .collect();
            kiln_adapter_webhook::admit_addresses(&webhook, &canonical)
                .map_err(|_| "runbook notification mapped network addresses denied")?;
            let url = reqwest::Url::parse(&format!(
                "https://{}:{}{}",
                notification.host, notification.port, notification.path
            ))
            .map_err(|_| "runbook notification URL invalid")?;
            if url.host_str() != Some(notification.host.as_str())
                || url.path() != notification.path
                || url.query().is_some()
                || url.fragment().is_some()
                || notification
                    .host
                    .parse::<IpAddr>()
                    .is_ok_and(|ip| !notification.addresses.contains(&ip))
            {
                return Err(
                    "runbook notification destination is not an exact registered authority".into(),
                );
            }
            if notification.timeout_ms == 0
                || notification.timeout_ms > 30_000
                || notification.max_request_bytes == 0
                || notification.max_request_bytes > MAX_PROFILE_BYTES
                || notification.max_response_bytes == 0
                || notification.max_response_bytes > MAX_PROFILE_BYTES
                || notification.max_resolver_candidates == 0
                || notification.max_resolver_candidates > 16
                || notification.host.contains([':', '?', '#', '\\'])
                || notification.path.contains(['?', '#', '\\'])
                || notification.path.starts_with("//")
                || !notification
                    .host
                    .bytes()
                    .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'.' | b'-'))
                || notification
                    .path
                    .bytes()
                    .any(|v| v.is_ascii_control() || v == b' ')
                || notification
                    .bearer_secret_file
                    .as_ref()
                    .is_some_and(|p| !crate::profile::absolute(p))
                || notification
                    .ca_certificate_file
                    .as_ref()
                    .is_some_and(|p| !crate::profile::absolute(p))
            {
                return Err("runbook notification authority invalid".into());
            }
        }
        Ok(())
    }

    pub fn admit_for_service(&self, service: &ServiceProfile) -> Result<(), String> {
        self.admit()?;
        for runbook in &self.runbooks {
            for target in &runbook.allowed_targets {
                let project = service
                    .projects
                    .iter()
                    .find(|p| p.id == target.project_id)
                    .ok_or_else(|| {
                        format!(
                            "runbook {} targets unknown project {}",
                            runbook.id, target.project_id
                        )
                    })?;
                for reference in &target.references {
                    if reference != &project.reference
                        && !project.admitted_references.iter().any(|r| r == reference)
                    {
                        return Err(format!(
                            "runbook {} target reference {} is not admitted on project {}",
                            runbook.id, reference, project.id
                        ));
                    }
                }
                if runbook.lattice.nodes != project.workflow.nodes
                    || runbook.lattice.edges != project.workflow.edges
                {
                    return Err(format!(
                        "runbook {} lattice workflow graph does not match project {}",
                        runbook.id, project.id
                    ));
                }
                if runbook.mode == RunbookMode::Mutating
                    && let Some(fanout) = &runbook.fanout
                    && target.repository.contains('/')
                {
                    for scope_path in &fanout.write_scopes {
                        kiln_core::fleet::parse_scope(&target.repository, scope_path).map_err(
                            |_| {
                                format!(
                                    "runbook {} write_scope {scope_path} invalid for {}",
                                    runbook.id, target.repository
                                )
                            },
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn find(&self, id: &str) -> Option<&RunbookDefinition> {
        self.runbooks.iter().find(|r| r.id == id)
    }

    pub fn has_active_grant(&self, principal_id: &str, runbook_id: &str, now_ms: u64) -> bool {
        self.grants.iter().any(|grant| {
            grant.principal == principal_id
                && grant.runbooks.iter().any(|id| id == runbook_id)
                && grant.not_after_ms.is_none_or(|limit| now_ms < limit)
        })
    }

    pub fn permits(
        &self,
        principal: &Principal,
        runbook: &RunbookDefinition,
        now_ms: u64,
    ) -> Result<(), &'static str> {
        match principal.role {
            Role::Worker => Err("worker_scope"),
            Role::Viewer => Err("operator_required"),
            Role::Operator => match runbook.mode {
                RunbookMode::ReadOnly => Ok(()),
                RunbookMode::Mutating => {
                    if self.has_active_grant(&principal.id, &runbook.id, now_ms) {
                        Ok(())
                    } else {
                        Err("runbook_grant_required")
                    }
                }
            },
        }
    }

    pub fn list_view(&self, principal: &Principal, now_ms: u64) -> RunbookListResponse {
        let runbooks = self
            .runbooks
            .iter()
            .map(|rb| {
                let (can_dispatch, dispatch_refusal) = match self.permits(principal, rb, now_ms) {
                    Ok(()) => (true, None),
                    Err(code) => (false, Some(code)),
                };
                RunbookSummaryView {
                    id: rb.id.clone(),
                    display_name: rb.display_name.clone(),
                    description: rb.description.clone(),
                    mode: rb.mode,
                    lattice_bundle_id: rb.lattice.bundle_id.clone(),
                    lattice_bundle_identity: rb.lattice.bundle_identity.clone(),
                    lattice_workflow_name: rb.lattice.workflow_name.clone(),
                    lattice_workflow_revision: rb.lattice.workflow_revision.clone(),
                    params: rb.params.iter().map(param_view).collect(),
                    allowed_targets: rb
                        .allowed_targets
                        .iter()
                        .map(|t| RunbookTargetView {
                            repository: t.repository.clone(),
                            project_id: t.project_id.clone(),
                            references: t.references.clone(),
                            allow_commit_sha: t.allow_commit_sha,
                        })
                        .collect(),
                    fanout: rb.fanout.clone(),
                    can_dispatch,
                    dispatch_refusal,
                }
            })
            .collect();
        RunbookListResponse {
            schema: RUNBOOKS_VIEW_SCHEMA,
            configured: true,
            principal: principal.id.clone(),
            runbooks,
        }
    }
}

fn param_view(param: &ParamDeclaration) -> RunbookParamView {
    match &param.param_type {
        ParamType::Bool {} => RunbookParamView {
            name: param.name.clone(),
            kind: "bool",
            required: param.required,
            default: param.default.clone(),
            enum_values: None,
            integer_min: None,
            integer_max: None,
            relative_path_max_bytes: None,
            bindings: param.bindings.clone(),
        },
        ParamType::Integer { min, max } => RunbookParamView {
            name: param.name.clone(),
            kind: "integer",
            required: param.required,
            default: param.default.clone(),
            enum_values: None,
            integer_min: Some(*min),
            integer_max: Some(*max),
            relative_path_max_bytes: None,
            bindings: param.bindings.clone(),
        },
        ParamType::Enum { values } => RunbookParamView {
            name: param.name.clone(),
            kind: "enum",
            required: param.required,
            default: param.default.clone(),
            enum_values: Some(values.clone()),
            integer_min: None,
            integer_max: None,
            relative_path_max_bytes: None,
            bindings: param.bindings.clone(),
        },
        ParamType::RelativePath { max_bytes } => RunbookParamView {
            name: param.name.clone(),
            kind: "relative_path",
            required: param.required,
            default: param.default.clone(),
            enum_values: None,
            integer_min: None,
            integer_max: None,
            relative_path_max_bytes: Some(*max_bytes),
            bindings: param.bindings.clone(),
        },
    }
}

pub fn parse_target(raw: &str) -> Result<(&str, &str), &'static str> {
    if raw.is_empty()
        || raw.len() > 1280
        || raw
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        return Err("runbook_target_invalid");
    }
    let Some((repo, reference)) = raw.split_once('@') else {
        return Err("runbook_target_invalid");
    };
    if repo.is_empty()
        || repo.len() > 256
        || reference.is_empty()
        || reference.len() > 1024
        || reference.contains('@')
        || reference.starts_with('-')
        || reference.contains("..")
    {
        return Err("runbook_target_invalid");
    }
    Ok((repo, reference))
}

pub fn resolve_target_rule<'a>(
    runbook: &'a RunbookDefinition,
    repo: &str,
) -> Option<&'a RunbookTarget> {
    runbook
        .allowed_targets
        .iter()
        .find(|t| t.repository == repo || t.project_id == repo)
}

pub fn resolve_params(
    declared: &[ParamDeclaration],
    supplied: &BTreeMap<String, Value>,
) -> Result<ResolvedParams, ParamError> {
    if supplied.len() > MAX_PARAMS {
        return Err(ParamError {
            code: "runbook_param_unknown",
        });
    }
    for key in supplied.keys() {
        if !declared.iter().any(|d| &d.name == key) {
            return Err(ParamError {
                code: "runbook_param_unknown",
            });
        }
    }

    let mut values = BTreeMap::new();
    let mut env_bindings: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"lattice.workflow-template-params.v1\0");

    let mut sorted_decls: Vec<&ParamDeclaration> = declared.iter().collect();
    sorted_decls.sort_by(|a, b| a.name.cmp(&b.name));

    for decl in sorted_decls {
        let value = match (supplied.get(&decl.name), &decl.default) {
            (Some(json_val), _) => decode_json_param(&decl.param_type, json_val)?,
            (None, Some(default_val)) => default_val.clone(),
            (None, None) => {
                return Err(ParamError {
                    code: "runbook_param_missing",
                });
            }
        };
        check_param_value_bounds(&decl.param_type, &value).map_err(|code| ParamError { code })?;
        if matches!(decl.param_type, ParamType::RelativePath { .. }) {
            let bad_name = secrets::is_secret_identifier(&decl.name)
                || decl.bindings.iter().any(|b| {
                    b.shell_env
                        .strip_prefix(PARAM_ENV_PREFIX)
                        .is_some_and(secrets::is_secret_identifier)
                });
            if bad_name {
                return Err(ParamError {
                    code: "runbook_param_secret_like",
                });
            }
            if let ParamValue::Text(text) = &value
                && secrets::is_secret_value(text)
            {
                return Err(ParamError {
                    code: "runbook_param_secret_like",
                });
            }
        }
        let rendered = value.render();
        if rendered.len() > MAX_PARAM_ENV_VALUE_BYTES || rendered.contains('\0') {
            return Err(ParamError {
                code: "runbook_param_invalid",
            });
        }
        hasher.update(decl.name.as_bytes());
        hasher.update(b"\0");
        hasher.update(value.kind_tag().as_bytes());
        hasher.update(b"\0");
        hasher.update(rendered.as_bytes());
        hasher.update(b"\0");

        for binding in &decl.bindings {
            env_bindings
                .entry(binding.node.clone())
                .or_default()
                .insert(binding.shell_env.clone(), rendered.clone());
        }
        values.insert(decl.name.clone(), value);
    }

    Ok(ResolvedParams {
        identity: hasher.finalize().to_hex().to_string(),
        values,
        env_bindings,
    })
}

fn decode_json_param(param_type: &ParamType, value: &Value) -> Result<ParamValue, ParamError> {
    match param_type {
        ParamType::Bool {} => value.as_bool().map(ParamValue::Bool).ok_or(ParamError {
            code: "runbook_param_invalid",
        }),
        ParamType::Integer { .. } => value.as_i64().map(ParamValue::Integer).ok_or(ParamError {
            code: "runbook_param_invalid",
        }),
        ParamType::Enum { .. } | ParamType::RelativePath { .. } => value
            .as_str()
            .map(|s| ParamValue::Text(s.to_owned()))
            .ok_or(ParamError {
                code: "runbook_param_invalid",
            }),
    }
}

pub fn validate_stored_binding(binding: &RunbookRunBinding) -> Result<(), String> {
    if !valid_runbook_id(&binding.runbook_id)
        || !is_hex64(&binding.lattice_bundle_identity)
        || !is_hex64(&binding.lattice_workflow_revision)
        || !is_hex64(&binding.params_identity)
        || !valid_lattice_label(&binding.lattice_workflow_name)
        || binding
            .lattice_bundle_id
            .as_deref()
            .is_some_and(|id| !valid_runbook_id(id))
    {
        return Err("stored runbook binding identity rejected".into());
    }
    for (node, envs) in &binding.env_bindings {
        if !valid_lattice_label(node) {
            return Err("stored runbook binding node rejected".into());
        }
        for (key, value) in envs {
            if !valid_param_env_key(key)
                || value.len() > MAX_PARAM_ENV_VALUE_BYTES
                || value.contains('\0')
            {
                return Err("stored runbook binding env rejected".into());
            }
        }
    }
    Ok(())
}

pub fn match_project<'a>(
    service: &'a ServiceProfile,
    project_id: &str,
) -> Option<&'a ProjectProfile> {
    service.projects.iter().find(|p| p.id == project_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_adapter_github::profile::ConfigEffectClass;

    fn sample_registry() -> RunbookRegistry {
        RunbookRegistry {
            schema: RUNBOOK_REGISTRY_SCHEMA.into(),
            runbooks: vec![
                RunbookDefinition {
                    id: "security-audit".into(),
                    display_name: "Security Audit".into(),
                    description: "Read-only audit".into(),
                    mode: RunbookMode::ReadOnly,
                    lattice: RunbookLattice {
                        bundle_id: Some("security-audit".into()),
                        bundle_identity: "a".repeat(64),
                        workflow_name: "security-audit".into(),
                        workflow_revision: "b".repeat(64),
                        nodes: vec![ConfigNode {
                            id: "audit".into(),
                            kind: "test".into(),
                            effect_class: ConfigEffectClass::Test,
                            adapter_kind: "local".into(),
                            operation_identity: "c".repeat(64),
                        }],
                        edges: vec![],
                    },
                    params: vec![
                        ParamDeclaration {
                            name: "strict".into(),
                            param_type: ParamType::Bool {},
                            required: false,
                            default: Some(ParamValue::Bool(true)),
                            bindings: vec![ParamBinding {
                                node: "audit".into(),
                                shell_env: "LATTICE_PARAM_STRICT".into(),
                            }],
                        },
                        ParamDeclaration {
                            name: "limit".into(),
                            param_type: ParamType::Integer { min: 1, max: 10 },
                            required: false,
                            default: Some(ParamValue::Integer(5)),
                            bindings: vec![ParamBinding {
                                node: "audit".into(),
                                shell_env: "LATTICE_PARAM_LIMIT".into(),
                            }],
                        },
                        ParamDeclaration {
                            name: "profile".into(),
                            param_type: ParamType::Enum {
                                values: vec!["fast".into(), "full".into()],
                            },
                            required: false,
                            default: Some(ParamValue::Text("fast".into())),
                            bindings: vec![ParamBinding {
                                node: "audit".into(),
                                shell_env: "LATTICE_PARAM_PROFILE".into(),
                            }],
                        },
                        ParamDeclaration {
                            name: "target_dir".into(),
                            param_type: ParamType::RelativePath { max_bytes: 64 },
                            required: true,
                            default: None,
                            bindings: vec![ParamBinding {
                                node: "audit".into(),
                                shell_env: "LATTICE_PARAM_TARGET_DIR".into(),
                            }],
                        },
                    ],
                    allowed_targets: vec![RunbookTarget {
                        repository: "onixresearch/kiln".into(),
                        project_id: "kiln-audit".into(),
                        references: vec!["refs/heads/main".into()],
                        allow_commit_sha: false,
                    }],
                    fanout: None,
                },
                RunbookDefinition {
                    id: "flake-update".into(),
                    display_name: "Flake Update".into(),
                    description: "Mutating flake update".into(),
                    mode: RunbookMode::Mutating,
                    lattice: RunbookLattice {
                        bundle_id: None,
                        bundle_identity: "1".repeat(64),
                        workflow_name: "flake-update".into(),
                        workflow_revision: "2".repeat(64),
                        nodes: vec![ConfigNode {
                            id: "job".into(),
                            kind: "build".into(),
                            effect_class: ConfigEffectClass::Build,
                            adapter_kind: "local".into(),
                            operation_identity: "3".repeat(64),
                        }],
                        edges: vec![],
                    },
                    params: vec![],
                    allowed_targets: vec![RunbookTarget {
                        repository: "onixresearch/kiln".into(),
                        project_id: "kiln-flake".into(),
                        references: vec!["refs/heads/main".into()],
                        allow_commit_sha: true,
                    }],
                    fanout: None,
                },
            ],
            grants: vec![RunbookGrant {
                principal: "local:operator".into(),
                runbooks: vec!["flake-update".into()],
                not_after_ms: Some(2_000_000_000_000),
            }],
            notifications: Vec::new(),
        }
    }

    #[test]
    fn registry_admission_validates_invariants_and_rejects_bad_configs() {
        let valid = sample_registry();
        assert!(valid.admit().is_ok());

        let mut wildcard = valid.clone();
        if let Some(grant) = wildcard.grants.first_mut() {
            grant.principal = "local:*".into();
        }
        assert!(wildcard.admit().is_err());

        let mut unknown_grant = valid.clone();
        if let Some(grant) = unknown_grant.grants.first_mut() {
            grant.runbooks = vec!["missing-rb".into()];
        }
        assert!(unknown_grant.admit().is_err());

        let mut bad_binding = valid.clone();
        if let Some(binding) = bad_binding
            .runbooks
            .first_mut()
            .and_then(|r| r.params.first_mut())
            .and_then(|p| p.bindings.first_mut())
        {
            binding.node = "missing".into();
        }
        assert!(bad_binding.admit().is_err());

        let mut bad_env = valid.clone();
        if let Some(binding) = bad_env
            .runbooks
            .first_mut()
            .and_then(|r| r.params.first_mut())
            .and_then(|p| p.bindings.first_mut())
        {
            binding.shell_env = "HOME".into();
        }
        assert!(bad_env.admit().is_err());

        let mut secret_param_name = valid.clone();
        if let Some(param) = secret_param_name
            .runbooks
            .first_mut()
            .and_then(|r| r.params.get_mut(3))
        {
            param.name = "github_token".into();
        }
        assert!(secret_param_name.admit().is_err());

        let mut duplicate_target_repo = valid;
        if let Some(rb) = duplicate_target_repo.runbooks.first_mut()
            && let Some(cloned_target) = rb.allowed_targets.first().cloned()
        {
            rb.allowed_targets.push(RunbookTarget {
                project_id: "other-project".into(),
                ..cloned_target
            });
        }
        assert!(duplicate_target_repo.admit().is_err());
    }

    #[test]
    fn resolve_params_checks_types_bounds_and_screens_secrets() {
        let reg = sample_registry();
        let decls = &reg
            .runbooks
            .first()
            .unwrap_or_else(|| unreachable!("runbook missing"))
            .params;

        let mut supplied = BTreeMap::new();
        supplied.insert(
            "target_dir".into(),
            Value::String("crates/kiln-core".into()),
        );
        supplied.insert("limit".into(), serde_json::json!(8));
        let resolved = resolve_params(decls, &supplied).unwrap_or_else(|e| unreachable!("{e:?}"));
        assert_eq!(resolved.values.get("strict"), Some(&ParamValue::Bool(true)));
        assert_eq!(resolved.values.get("limit"), Some(&ParamValue::Integer(8)));
        assert_eq!(
            resolved
                .env_bindings
                .get("audit")
                .and_then(|env| env.get("LATTICE_PARAM_TARGET_DIR"))
                .map(String::as_str),
            Some("crates/kiln-core")
        );

        let mut unknown = supplied.clone();
        unknown.insert("extra".into(), Value::Bool(true));
        assert_eq!(
            resolve_params(decls, &unknown).err().map(|e| e.code),
            Some("runbook_param_unknown")
        );

        let mut bad_bool = supplied.clone();
        bad_bool.insert("strict".into(), Value::String("true".into()));
        assert_eq!(
            resolve_params(decls, &bad_bool).err().map(|e| e.code),
            Some("runbook_param_invalid")
        );

        let mut out_of_bounds = supplied.clone();
        out_of_bounds.insert("limit".into(), serde_json::json!(99));
        assert_eq!(
            resolve_params(decls, &out_of_bounds).err().map(|e| e.code),
            Some("runbook_param_invalid")
        );

        let mut secret_val = BTreeMap::new();
        secret_val.insert(
            "target_dir".into(),
            Value::String("ghp_1234567890abcdef1234567890abcdef12".into()),
        );
        assert_eq!(
            resolve_params(decls, &secret_val).err().map(|e| e.code),
            Some("runbook_param_secret_like")
        );
    }

    #[test]
    fn permits_checks_roles_and_grant_expiry() {
        let reg = sample_registry();
        let ro = reg
            .runbooks
            .first()
            .unwrap_or_else(|| unreachable!("ro missing"));
        let mut_rb = reg
            .runbooks
            .get(1)
            .unwrap_or_else(|| unreachable!("mutating missing"));
        let op = Principal {
            id: "local:operator".into(),
            label: "Operator".into(),
            role: Role::Operator,
            agent: None,
        };
        let other_op = Principal {
            id: "local:other".into(),
            label: "Other".into(),
            role: Role::Operator,
            agent: None,
        };
        let viewer = Principal {
            id: "local:viewer".into(),
            label: "Viewer".into(),
            role: Role::Viewer,
            agent: None,
        };
        assert_eq!(reg.permits(&op, ro, 1_000), Ok(()));
        assert_eq!(reg.permits(&other_op, ro, 1_000), Ok(()));
        assert_eq!(reg.permits(&op, mut_rb, 1_000), Ok(()));
        assert_eq!(
            reg.permits(&op, mut_rb, 2_000_000_000_000),
            Err("runbook_grant_required")
        );
        assert_eq!(
            reg.permits(&other_op, mut_rb, 1_000),
            Err("runbook_grant_required")
        );
        assert_eq!(reg.permits(&viewer, ro, 1_000), Err("operator_required"));
    }
}
