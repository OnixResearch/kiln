//! Operator inventory of herdr machines and agent slots.
//!
//! `HerdrInventory` is the JSON export of typed Nickel
//! `config/fleet-inventory.ncl`, schema `kiln.fleet-inventory.v1`. It names
//! the herdr executable, the local headless session, the explicit child
//! environment and every machine with its repositories and agent slots. It
//! holds no credentials.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const INVENTORY_SCHEMA: &str = "kiln.fleet-inventory.v1";
pub const DEFAULT_SESSION: &str = "kiln";
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// herdr's own start default is 30 s; interactive agents may load slower.
pub const DEFAULT_START_TIMEOUT_MS: u64 = 60_000;
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;
pub const MIN_TIMEOUT_MS: u64 = 100;
pub const MAX_TIMEOUT_MS: u64 = 300_000;
pub const MIN_OUTPUT_BYTES: usize = 1_024;
pub const MAX_OUTPUT_BYTES: usize = 4 * 1_048_576;
pub const MAX_MACHINES: usize = 64;
pub const MAX_SLOTS_PER_KIND: u32 = 16;
pub const MAX_AGENT_ARGS: usize = 32;
pub const MAX_AGENT_ARG_BYTES: usize = 256;
pub const MAX_REPOSITORIES: usize = 256;
pub const MAX_LABEL_BYTES: usize = 63;
pub const MAX_SESSION_BYTES: usize = 64;
pub const MAX_HERDR_MACHINE_BYTES: usize = 128;
pub const MAX_SSH_TARGET_BYTES: usize = 128;
pub const MAX_REPOSITORY_BYTES: usize = 128;
pub const MAX_PATH_BYTES: usize = 512;
pub const MAX_SEARCH_PATH_BYTES: usize = 4_096;

/// Interactive agent kinds Kiln admits; herdr knows more kinds than these.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Omp,
    Claude,
    Codex,
    Agy,
}

impl AgentKind {
    pub const ALL: [Self; 4] = [Self::Omp, Self::Claude, Self::Codex, Self::Agy];

    /// herdr's `--kind` value and the label it reports for a detected agent.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Omp => "omp",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Agy => "agy",
        }
    }

    /// Admit one herdr kind label; unadmitted kinds are `None`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// Agent slots of one kind on one machine.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSlots {
    pub kind: AgentKind,
    pub slots: u32,
    /// Native agent arguments passed after `--`.
    #[serde(default)]
    pub args: Vec<String>,
}

/// One machine that hosts interactive agents.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HerdrMachine {
    /// Fleet machine label, `[a-z0-9][a-z0-9-]*`.
    pub label: String,
    /// Saved herdr machine label, or `None` for the local headless session.
    #[serde(default)]
    pub herdr_machine: Option<String>,
    /// SSH target for git transport, e.g. `brittonr@aspen3`.
    pub ssh_target: String,
    /// Absolute directory that holds attempt worktrees on this machine.
    pub worktree_root: String,
    /// `owner/name` to an absolute checkout path on this machine.
    #[serde(default)]
    pub repositories: BTreeMap<String, String>,
    #[serde(default)]
    pub agents: Vec<AgentSlots>,
}

impl HerdrMachine {
    /// Checkout path for `owner/name`, compared ASCII-case-insensitively.
    #[must_use]
    pub fn checkout(&self, repository: &str) -> Option<&str> {
        self.repositories
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(repository))
            .map(|(_, path)| path.as_str())
    }

    /// Configured slots for one kind; zero when the kind is not configured.
    #[must_use]
    pub fn slots(&self, kind: AgentKind) -> u32 {
        self.agent(kind).map_or(0, |agent| agent.slots)
    }

    /// Configured agent entry for one kind.
    #[must_use]
    pub fn agent(&self, kind: AgentKind) -> Option<&AgentSlots> {
        self.agents.iter().find(|agent| agent.kind == kind)
    }
}

// Deviation from design section 10, which lists only `executable`, `session`
// and `machines`: the explicit child environment, deadlines and output bound
// are configuration too, so they live here with named defaults.
/// Explicit environment for every herdr child; the ambient one is cleared.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HerdrEnvironment {
    pub home: String,
    pub path: String,
    #[serde(default)]
    pub xdg_runtime_dir: Option<String>,
    #[serde(default)]
    pub xdg_config_home: Option<String>,
    /// SSH agent socket for `--machine` transport, when keys are agent-held.
    #[serde(default)]
    pub ssh_auth_sock: Option<String>,
}

impl HerdrEnvironment {
    /// Name/value pairs handed to the child, in a fixed order.
    #[must_use]
    pub fn entries(&self) -> Vec<(&'static str, &str)> {
        let mut entries = vec![("HOME", self.home.as_str()), ("PATH", self.path.as_str())];
        let optional = [
            ("XDG_RUNTIME_DIR", self.xdg_runtime_dir.as_deref()),
            ("XDG_CONFIG_HOME", self.xdg_config_home.as_deref()),
            ("SSH_AUTH_SOCK", self.ssh_auth_sock.as_deref()),
        ];
        entries.extend(
            optional
                .into_iter()
                .filter_map(|(name, value)| value.map(|value| (name, value))),
        );
        entries
    }
}

/// Reviewed herdr fleet inventory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HerdrInventory {
    pub schema: String,
    /// Absolute path to the pinned `herdr` executable.
    pub executable: PathBuf,
    /// Local headless session name.
    #[serde(default = "default_session")]
    pub session: String,
    pub environment: HerdrEnvironment,
    /// Deadline for every command except `agent start`.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// herdr's own readiness wait for `agent start`.
    #[serde(default = "default_start_timeout_ms")]
    pub start_timeout_ms: u64,
    /// Maximum retained stdout/stderr bytes per command.
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,
    pub machines: Vec<HerdrMachine>,
}

fn default_session() -> String {
    DEFAULT_SESSION.to_owned()
}

const fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

const fn default_start_timeout_ms() -> u64 {
    DEFAULT_START_TIMEOUT_MS
}

const fn default_max_output_bytes() -> usize {
    DEFAULT_MAX_OUTPUT_BYTES
}

impl HerdrInventory {
    /// Parse and validate one JSON export.
    ///
    /// # Errors
    /// Returns a bounded diagnostic when the JSON does not match the schema or
    /// fails validation.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let inventory: Self = serde_json::from_str(text)
            .map_err(|error| format!("fleet inventory is invalid JSON: {error}"))?;
        inventory.validate()?;
        Ok(inventory)
    }

    /// Machine by fleet label.
    #[must_use]
    pub fn machine(&self, label: &str) -> Option<&HerdrMachine> {
        self.machines.iter().find(|machine| machine.label == label)
    }

    /// Validate the inventory without running herdr.
    ///
    /// # Errors
    /// Returns a bounded diagnostic naming the first invalid field.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != INVENTORY_SCHEMA {
            return Err(format!("fleet inventory schema must be {INVENTORY_SCHEMA}"));
        }
        if !self.executable.is_absolute() {
            return Err("herdr executable must be an absolute path".to_owned());
        }
        if !session_name(&self.session) {
            return Err("herdr session name is invalid".to_owned());
        }
        self.validate_environment()?;
        if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&self.timeout_ms) {
            return Err("herdr timeout is out of range".to_owned());
        }
        if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&self.start_timeout_ms) {
            return Err("herdr start timeout is out of range".to_owned());
        }
        if !(MIN_OUTPUT_BYTES..=MAX_OUTPUT_BYTES).contains(&self.max_output_bytes) {
            return Err("herdr output bound is out of range".to_owned());
        }
        if self.machines.is_empty() || self.machines.len() > MAX_MACHINES {
            return Err("fleet inventory machine count is out of range".to_owned());
        }
        let mut labels = BTreeSet::new();
        for machine in &self.machines {
            validate_machine(machine)?;
            if !labels.insert(machine.label.as_str()) {
                return Err(format!("machine {} is listed twice", machine.label));
            }
        }
        Ok(())
    }

    fn validate_environment(&self) -> Result<(), String> {
        let environment = &self.environment;
        if !absolute_path(&environment.home) {
            return Err("herdr environment HOME must be an absolute path".to_owned());
        }
        if !search_path(&environment.path) {
            return Err("herdr environment PATH is invalid".to_owned());
        }
        for (name, value) in [
            ("XDG_RUNTIME_DIR", &environment.xdg_runtime_dir),
            ("XDG_CONFIG_HOME", &environment.xdg_config_home),
            ("SSH_AUTH_SOCK", &environment.ssh_auth_sock),
        ] {
            if value.as_deref().is_some_and(|value| !absolute_path(value)) {
                return Err(format!("herdr environment {name} must be an absolute path"));
            }
        }
        Ok(())
    }
}

fn validate_machine(machine: &HerdrMachine) -> Result<(), String> {
    if !machine_label(&machine.label) {
        return Err("machine label must match [a-z0-9][a-z0-9-]*".to_owned());
    }
    let label = &machine.label;
    if machine
        .herdr_machine
        .as_deref()
        .is_some_and(|value| !token(value, MAX_HERDR_MACHINE_BYTES))
    {
        return Err(format!("machine {label}: herdr_machine is invalid"));
    }
    if !token(&machine.ssh_target, MAX_SSH_TARGET_BYTES) || machine.ssh_target.starts_with('-') {
        return Err(format!("machine {label}: ssh_target is invalid"));
    }
    if !absolute_path(&machine.worktree_root) {
        return Err(format!(
            "machine {label}: worktree_root must be an absolute path"
        ));
    }
    if machine.repositories.len() > MAX_REPOSITORIES {
        return Err(format!("machine {label}: too many repositories"));
    }
    let mut repositories = BTreeSet::new();
    for (repository, checkout) in &machine.repositories {
        if !repository_name(repository) {
            return Err(format!("machine {label}: repository must be owner/name"));
        }
        if !repositories.insert(repository.to_ascii_lowercase()) {
            return Err(format!(
                "machine {label}: repository {repository} is listed twice"
            ));
        }
        if !absolute_path(checkout) {
            return Err(format!(
                "machine {label}: checkout for {repository} must be an absolute path"
            ));
        }
    }
    let mut kinds = BTreeSet::new();
    for agent in &machine.agents {
        if !kinds.insert(agent.kind) {
            return Err(format!(
                "machine {label}: agent kind {} is listed twice",
                agent.kind.as_str()
            ));
        }
        if !(1..=MAX_SLOTS_PER_KIND).contains(&agent.slots) {
            return Err(format!("machine {label}: agent slots are out of range"));
        }
        if agent.args.len() > MAX_AGENT_ARGS
            || agent
                .args
                .iter()
                .any(|arg| arg.is_empty() || !bounded_text(arg, MAX_AGENT_ARG_BYTES))
        {
            return Err(format!("machine {label}: agent args are invalid"));
        }
    }
    Ok(())
}

pub(crate) fn machine_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_LABEL_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn session_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SESSION_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        && !value.starts_with('-')
}

fn repository_name(value: &str) -> bool {
    let mut parts = value.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    value.len() <= MAX_REPOSITORY_BYTES
        && [owner, name].into_iter().all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
}

/// Printable ASCII without whitespace.
pub(crate) fn token(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && value.bytes().all(|byte| byte.is_ascii_graphic())
}

/// Printable ASCII; spaces allowed, controls refused.
pub(crate) fn bounded_text(value: &str, maximum: usize) -> bool {
    value.len() <= maximum && value.is_ascii() && !value.bytes().any(|byte| byte.is_ascii_control())
}

/// Absolute, normalized-enough path: leading `/`, no `..`, no controls.
pub(crate) fn absolute_path(value: &str) -> bool {
    value.starts_with('/')
        && value.len() <= MAX_PATH_BYTES
        && !value.chars().any(char::is_control)
        && !value.split('/').any(|segment| segment == "..")
}

fn search_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SEARCH_PATH_BYTES
        && !value.chars().any(char::is_control)
        && value.split(':').all(absolute_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory() -> HerdrInventory {
        HerdrInventory {
            schema: INVENTORY_SCHEMA.to_owned(),
            executable: PathBuf::from("/run/current-system/sw/bin/herdr"),
            session: DEFAULT_SESSION.to_owned(),
            environment: HerdrEnvironment {
                home: "/home/kiln".to_owned(),
                path: "/run/current-system/sw/bin".to_owned(),
                xdg_runtime_dir: Some("/run/user/1000".to_owned()),
                xdg_config_home: None,
                ssh_auth_sock: None,
            },
            timeout_ms: DEFAULT_TIMEOUT_MS,
            start_timeout_ms: DEFAULT_START_TIMEOUT_MS,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            machines: vec![HerdrMachine {
                label: "aspen1".to_owned(),
                herdr_machine: None,
                ssh_target: "brittonr@aspen1".to_owned(),
                worktree_root: "/home/kiln/worktrees".to_owned(),
                repositories: BTreeMap::from([(
                    "OnixResearch/kiln".to_owned(),
                    "/home/kiln/src/kiln".to_owned(),
                )]),
                agents: vec![AgentSlots {
                    kind: AgentKind::Omp,
                    slots: 2,
                    args: Vec::new(),
                }],
            }],
        }
    }

    #[test]
    fn valid_inventory_is_admitted() {
        inventory()
            .validate()
            .unwrap_or_else(|error| unreachable!("{error}"));
        let machine = inventory()
            .machine("aspen1")
            .cloned()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            machine.checkout("onixresearch/KILN"),
            Some("/home/kiln/src/kiln")
        );
        assert_eq!(machine.slots(AgentKind::Omp), 2);
        assert_eq!(machine.slots(AgentKind::Codex), 0);
    }

    #[test]
    fn invalid_inventory_is_refused() {
        let cases: Vec<fn(&mut HerdrInventory)> = vec![
            |inventory| inventory.schema = "kiln.fleet-inventory.v0".to_owned(),
            |inventory| inventory.executable = PathBuf::from("herdr"),
            |inventory| inventory.session = "has space".to_owned(),
            |inventory| inventory.environment.home = "home".to_owned(),
            |inventory| inventory.environment.path = "bin:/usr/bin".to_owned(),
            |inventory| inventory.timeout_ms = 0,
            |inventory| inventory.max_output_bytes = 1,
            |inventory| inventory.machines.clear(),
            |inventory| {
                let machine = inventory.machines.first().cloned();
                inventory.machines.extend(machine);
            },
            |inventory| {
                inventory.machines.iter_mut().for_each(|machine| {
                    machine.label = "Aspen1".to_owned();
                });
            },
            |inventory| {
                inventory.machines.iter_mut().for_each(|machine| {
                    machine.worktree_root = "/home/../etc".to_owned();
                });
            },
            |inventory| {
                inventory.machines.iter_mut().for_each(|machine| {
                    machine
                        .repositories
                        .insert("kiln".to_owned(), "/src/kiln".to_owned());
                });
            },
            |inventory| {
                inventory.machines.iter_mut().for_each(|machine| {
                    machine
                        .repositories
                        .insert("onixresearch/kiln".to_owned(), "/src/kiln".to_owned());
                });
            },
            |inventory| {
                inventory.machines.iter_mut().for_each(|machine| {
                    machine.agents.iter_mut().for_each(|agent| agent.slots = 0);
                });
            },
            |inventory| {
                inventory.machines.iter_mut().for_each(|machine| {
                    let agent = machine.agents.first().cloned();
                    machine.agents.extend(agent);
                });
            },
            |inventory| {
                inventory.machines.iter_mut().for_each(|machine| {
                    machine
                        .agents
                        .iter_mut()
                        .for_each(|agent| agent.args = vec!["a\nb".to_owned()]);
                });
            },
        ];
        for (index, mutate) in cases.into_iter().enumerate() {
            let mut candidate = inventory();
            mutate(&mut candidate);
            assert!(
                candidate.validate().is_err(),
                "case {index} must be refused"
            );
        }
    }

    #[test]
    fn unadmitted_kind_and_unknown_fields_fail_to_parse() {
        let text = serde_json::to_string(&inventory()).unwrap_or_else(|_| unreachable!());
        assert!(HerdrInventory::from_json(&text).is_ok());
        let gemini = text.replace("\"omp\"", "\"gemini\"");
        assert!(HerdrInventory::from_json(&gemini).is_err());
        let extra = text.replacen('{', "{\"password\":\"x\",", 1);
        assert!(HerdrInventory::from_json(&extra).is_err());
    }

    #[test]
    fn environment_entries_are_explicit() {
        let inventory = inventory();
        assert_eq!(
            inventory.environment.entries(),
            vec![
                ("HOME", "/home/kiln"),
                ("PATH", "/run/current-system/sw/bin"),
                ("XDG_RUNTIME_DIR", "/run/user/1000"),
            ]
        );
    }
}
