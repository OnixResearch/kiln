//! `kiln.fleet.v1` service configuration.
//!
//! The JSON is an export of typed Nickel `config/fleet.ncl`. Every object
//! refuses unknown fields. [`FleetConfig::validate`] checks the document
//! alone, without touching the paths it names, so `check-config` can run on
//! a build machine; `run` additionally loads the referenced files.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use kiln_adapter_herdr::AgentKind;
use kiln_adapter_swarm::SwarmConfig;
use kiln_core::fleet::{
    DEFAULT_ATTEMPT_LEASE_MS, DEFAULT_BLOCKED_DECISION_AFTER_MS, DEFAULT_CLAIM_LEASE_MS,
    DEFAULT_MAX_NUDGES, DEFAULT_NUDGE_AFTER_MS, DEFAULT_RENEW_BEFORE_MS,
    DEFAULT_STALE_KEEP_WAITING_LEASES, DEFAULT_STALE_OBSERVATION_AFTER_MS, DEFAULT_TICK_MS,
    FleetPolicy, MAX_CLAIM_LEASE_MS, MIN_CLAIM_LEASE_MS,
};
use kiln_core::implementation::swarm::from_address;
use serde::{Deserialize, Serialize};

pub const FLEET_CONFIG_SCHEMA: &str = "kiln.fleet.v1";
pub const DEFAULT_DECISION_GROUP: &str = "bots.general";
/// Domain of the per-machine swarm addresses (`<machine>@swarm.local`).
pub const SWARM_LOCAL_DOMAIN: &str = "swarm.local";
pub const MAX_CONFIG_BYTES: u64 = 256 * 1_024;
pub const MAX_LISTEN_ADDRESSES: usize = 8;
pub const MAX_PEERS: usize = 64;
pub const MAX_OPERATORS: usize = 64;
pub const MAX_MACHINE_LABEL_BYTES: usize = 63;
pub const MAX_NODE_NAME_BYTES: usize = 253;
pub const MAX_PATH_BYTES: usize = 4_096;
pub const MAX_CAPTURE_REPOSITORIES: usize = 256;
pub const MAX_REPOSITORY_BYTES: usize = 128;
pub const MAX_ARGV: usize = 64;
pub const MAX_ARG_BYTES: usize = 4_096;
pub const MAX_GROUP_BYTES: usize = 64;
pub const MAX_CONSOLE_URL_BYTES: usize = 2_048;
pub const MAX_REPOSITORY_SCAN_ROOTS: usize = 32;
/// Environment variable naming the unified `kiln-web-service` Factory Control Plane base URL.
pub const CONSOLE_URL_ENVIRONMENT: &str = "KILN_CONSOLE_URL";
/// Placeholders substituted as whole arguments in `verify_argv`/`publish_argv`.
pub const PLACEHOLDERS: [&str; 8] = [
    "{attempt}",
    "{fence}",
    "{database}",
    "{enrollment}",
    "{verification_profile}",
    "{authority}",
    "{handoff}",
    "{repository}",
];

/// The whole service configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetConfig {
    pub schema: String,
    /// Coordinator machine label.
    pub machine: String,
    /// `ip:port` listen addresses; loopback or tailnet only.
    pub listen: Vec<String>,
    /// Tailscale node name to machine label.
    pub peers: BTreeMap<String, String>,
    /// Machines allowed to cancel attempts and answer decisions.
    pub operators: Vec<String>,
    /// Absolute path of the `tailscale` CLI used for `whois`.
    pub tailscale: PathBuf,
    pub implementer: ImplementerPaths,
    /// `kiln.fleet-inventory.v1` JSON for the herdr adapter.
    pub inventory: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) herdr_core: Option<HerdrCoreConfig>,
    #[serde(default)]
    pub swarm: Option<SwarmConfig>,
    #[serde(default = "default_decision_group")]
    pub decision_group: String,
    #[serde(default)]
    pub capture: Option<CaptureConfig>,
    #[serde(default)]
    pub policy: PolicyConfig,
    pub dispatch: DispatchConfig,
    /// Explicit local-operator profile for protected implementation effects.
    #[serde(default)]
    pub authority_operator: Option<String>,
    /// Exported `kiln.implementation-authority-config.v1`; exclusive with
    /// `authority_operator`.
    #[serde(default)]
    pub authority_config: Option<PathBuf>,
    /// Optional base URL of the unified `kiln-web-service` Factory Control Plane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub console_url: Option<String>,
    /// Optional local directory roots scanned shallowly (`1..=2`) for checkout discovery.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repository_scan_roots: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HerdrCoreConfig {
    pub(crate) socket: PathBuf,
    pub(crate) version: String,
    /// Fleet label to Core nickname; the empty nickname identifies the local core.
    pub(crate) hosts: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementerPaths {
    pub database: PathBuf,
    pub enrollment: PathBuf,
}

/// Capture of agent commits into the trusted repositories.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureConfig {
    pub git: PathBuf,
    pub tar: PathBuf,
    /// Directory holding one fresh export per captured attempt.
    pub export_root: PathBuf,
    /// Directory of `<profile content id>.json` verification profiles.
    pub verification_profiles: PathBuf,
    /// `owner/name` to its verifier authority.
    pub repositories: BTreeMap<String, CaptureRepository>,
    #[serde(default)]
    pub verify_argv: Option<Vec<String>>,
    #[serde(default)]
    pub publish_argv: Option<Vec<String>>,
    /// Preserve valid Git AI (`refs/notes/ai`, `authorship/3.0.0`) notes across
    /// handoff rematerialization. Off by default; observation-only.
    #[serde(default)]
    pub preserve_git_ai_notes: bool,
}

impl CaptureConfig {
    /// The authority of `repository`, compared ASCII-case-insensitively.
    #[must_use]
    pub fn repository(&self, repository: &str) -> Option<&CaptureRepository> {
        self.repositories
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(repository))
            .map(|(_, value)| value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureRepository {
    /// `VerifierAuthority` JSON for this repository.
    pub authority: PathBuf,
}

/// Dispatch and claim limits; every field has a named default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyConfig {
    #[serde(default = "default_tick_ms")]
    pub tick_ms: u64,
    #[serde(default = "default_attempt_lease_ms")]
    pub attempt_lease_ms: u64,
    #[serde(default = "default_renew_before_ms")]
    pub renew_before_ms: u64,
    #[serde(default = "default_blocked_decision_after_ms")]
    pub blocked_decision_after_ms: u64,
    #[serde(default = "default_nudge_after_ms")]
    pub nudge_after_ms: u64,
    #[serde(default = "default_max_nudges")]
    pub max_nudges: u32,
    #[serde(default = "default_claim_lease_ms")]
    pub claim_lease_ms: u64,
    #[serde(default = "default_max_claim_lease_ms")]
    pub max_claim_lease_ms: u64,
    #[serde(default = "default_stale_observation_after_ms")]
    pub stale_observation_after_ms: u64,
    #[serde(default = "default_stale_keep_waiting_leases")]
    pub stale_keep_waiting_leases: u32,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            tick_ms: DEFAULT_TICK_MS,
            attempt_lease_ms: DEFAULT_ATTEMPT_LEASE_MS,
            renew_before_ms: DEFAULT_RENEW_BEFORE_MS,
            blocked_decision_after_ms: DEFAULT_BLOCKED_DECISION_AFTER_MS,
            nudge_after_ms: DEFAULT_NUDGE_AFTER_MS,
            max_nudges: DEFAULT_MAX_NUDGES,
            claim_lease_ms: DEFAULT_CLAIM_LEASE_MS,
            max_claim_lease_ms: MAX_CLAIM_LEASE_MS,
            stale_observation_after_ms: DEFAULT_STALE_OBSERVATION_AFTER_MS,
            stale_keep_waiting_leases: DEFAULT_STALE_KEEP_WAITING_LEASES,
        }
    }
}

impl PolicyConfig {
    /// The core tick policy.
    #[must_use]
    pub const fn fleet(&self) -> FleetPolicy {
        FleetPolicy {
            tick_ms: self.tick_ms,
            renew_before_ms: self.renew_before_ms,
            blocked_decision_after_ms: self.blocked_decision_after_ms,
            nudge_after_ms: self.nudge_after_ms,
            max_nudges: self.max_nudges,
            attempt_lease_ms: self.attempt_lease_ms,
            stale_observation_after_ms: self.stale_observation_after_ms,
            stale_keep_waiting_leases: self.stale_keep_waiting_leases,
        }
    }
}

const fn default_tick_ms() -> u64 {
    DEFAULT_TICK_MS
}
const fn default_attempt_lease_ms() -> u64 {
    DEFAULT_ATTEMPT_LEASE_MS
}
const fn default_renew_before_ms() -> u64 {
    DEFAULT_RENEW_BEFORE_MS
}
const fn default_blocked_decision_after_ms() -> u64 {
    DEFAULT_BLOCKED_DECISION_AFTER_MS
}
const fn default_nudge_after_ms() -> u64 {
    DEFAULT_NUDGE_AFTER_MS
}
const fn default_max_nudges() -> u32 {
    DEFAULT_MAX_NUDGES
}
const fn default_claim_lease_ms() -> u64 {
    DEFAULT_CLAIM_LEASE_MS
}
const fn default_max_claim_lease_ms() -> u64 {
    MAX_CLAIM_LEASE_MS
}
const fn default_stale_observation_after_ms() -> u64 {
    DEFAULT_STALE_OBSERVATION_AFTER_MS
}
const fn default_stale_keep_waiting_leases() -> u32 {
    DEFAULT_STALE_KEEP_WAITING_LEASES
}
fn default_decision_group() -> String {
    DEFAULT_DECISION_GROUP.to_owned()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchConfig {
    /// `false` runs the claim authority alone.
    pub enabled: bool,
    /// Agent kinds dispatch may start, in preference order.
    pub kinds: Vec<AgentKind>,
}

impl FleetConfig {
    /// Read, parse and validate one configuration file.
    ///
    /// # Errors
    /// Returns a bounded diagnostic for an unreadable, oversized, malformed
    /// or invalid document.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = read_bounded(path, MAX_CONFIG_BYTES)?;
        Self::from_json(&bytes)
    }

    /// Parse and validate one JSON document.
    ///
    /// # Errors
    /// Returns the parse or validation diagnostic.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let config: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("fleet configuration is malformed: {error}"))?;
        config.validate()?;
        Ok(config)
    }

    /// The configured swarm, when present and enabled.
    #[must_use]
    pub fn active_swarm(&self) -> Option<&SwarmConfig> {
        self.swarm.as_ref().filter(|swarm| swarm.enabled)
    }

    /// Parsed listen addresses.
    ///
    /// # Errors
    /// Returns a diagnostic for an address that does not parse.
    pub fn listen_addresses(&self) -> Result<Vec<SocketAddr>, String> {
        self.listen
            .iter()
            .map(|value| {
                value
                    .parse::<SocketAddr>()
                    .map_err(|_| format!("listen address {value:?} is not ip:port"))
            })
            .collect()
    }

    /// Every machine the service can authenticate: the coordinator and each
    /// peer machine.
    #[must_use]
    pub fn machines(&self) -> BTreeSet<&str> {
        std::iter::once(self.machine.as_str())
            .chain(self.peers.values().map(String::as_str))
            .collect()
    }

    #[must_use]
    pub fn is_operator(&self, machine: &str) -> bool {
        self.operators.iter().any(|operator| operator == machine)
    }

    /// Validate the document alone.
    ///
    /// # Errors
    /// Returns the first violated rule.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FLEET_CONFIG_SCHEMA {
            return Err(format!(
                "fleet configuration schema must be {FLEET_CONFIG_SCHEMA}"
            ));
        }
        if !machine_label(&self.machine) {
            return Err("machine must be a label [a-z0-9][a-z0-9-]*".to_owned());
        }
        self.validate_listen()?;
        if self.peers.len() > MAX_PEERS {
            return Err("too many peers".to_owned());
        }
        for (node, machine) in &self.peers {
            if !node_name(node) {
                return Err(format!("peer node name {node:?} is invalid"));
            }
            if !machine_label(machine) {
                return Err(format!("peer machine {machine:?} is not a label"));
            }
        }
        if self.operators.len() > MAX_OPERATORS {
            return Err("too many operators".to_owned());
        }
        let machines = self.machines();
        let mut seen = BTreeSet::new();
        for operator in &self.operators {
            if !machines.contains(operator.as_str()) || !seen.insert(operator.as_str()) {
                return Err(format!(
                    "operator {operator:?} is duplicated or not the coordinator or a peer machine"
                ));
            }
        }
        for (path, field) in [
            (&self.tailscale, "tailscale"),
            (&self.implementer.database, "implementer.database"),
            (&self.implementer.enrollment, "implementer.enrollment"),
            (&self.inventory, "inventory"),
        ] {
            absolute(path, field)?;
        }
        if let Some(core) = &self.herdr_core {
            absolute(&core.socket, "herdr_core.socket")?;
            let mut aliases = BTreeSet::new();
            if core.version != "0.1.19"
                || core.hosts.is_empty()
                || core.hosts.iter().any(|(host, alias)| {
                    !machines.contains(host.as_str())
                        || (!alias.is_empty() && !machine_label(alias))
                        || !aliases.insert(alias)
                })
            {
                return Err(
                    "herdr_core requires version 0.1.19 and unique aliases for enrolled hosts"
                        .to_owned(),
                );
            }
        }
        if let Some(swarm) = &self.swarm {
            swarm.validate()?;
            self.validate_from(swarm)?;
        }
        if !group_name(&self.decision_group) {
            return Err("decision_group is not a group name".to_owned());
        }
        if let Some(capture) = &self.capture {
            validate_capture(capture)?;
        }
        if let Some(console_url) = &self.console_url {
            validate_console_url(console_url)?;
        }
        if self.repository_scan_roots.len() > MAX_REPOSITORY_SCAN_ROOTS {
            return Err(format!(
                "repository_scan_roots must name at most {MAX_REPOSITORY_SCAN_ROOTS} directories"
            ));
        }
        for root in &self.repository_scan_roots {
            absolute(root, "repository_scan_roots")?;
        }
        self.validate_policy()?;
        self.validate_dispatch()
    }

    fn validate_listen(&self) -> Result<(), String> {
        if self.listen.is_empty() || self.listen.len() > MAX_LISTEN_ADDRESSES {
            return Err(format!(
                "listen must name 1 to {MAX_LISTEN_ADDRESSES} addresses"
            ));
        }
        let addresses = self.listen_addresses()?;
        let mut seen = BTreeSet::new();
        for address in addresses {
            if !seen.insert(address) {
                return Err(format!("listen address {address} is duplicated"));
            }
            if !(address.ip().is_loopback() || tailnet(address.ip())) {
                return Err(format!(
                    "listen address {address} is neither loopback nor a tailnet address"
                ));
            }
        }
        Ok(())
    }

    /// Kiln posts with its own From address: refuse one that names any
    /// listed machine's `<machine>@swarm.local`.
    fn validate_from(&self, swarm: &SwarmConfig) -> Result<(), String> {
        let address = from_address(&swarm.from).to_ascii_lowercase();
        let listed = self
            .machines()
            .into_iter()
            .chain(self.operators.iter().map(String::as_str));
        for machine in listed {
            if address == format!("{machine}@{SWARM_LOCAL_DOMAIN}") {
                return Err(format!(
                    "swarm.from must be Kiln's own address, not machine {machine}'s"
                ));
            }
        }
        Ok(())
    }

    fn validate_policy(&self) -> Result<(), String> {
        let policy = &self.policy;
        policy
            .fleet()
            .validate()
            .map_err(|error| format!("policy: {}", error.message))?;
        let lease = MIN_CLAIM_LEASE_MS..=MAX_CLAIM_LEASE_MS;
        if !lease.contains(&policy.claim_lease_ms)
            || !lease.contains(&policy.max_claim_lease_ms)
            || policy.claim_lease_ms > policy.max_claim_lease_ms
        {
            return Err(format!(
                "policy claim leases must lie within {MIN_CLAIM_LEASE_MS}..={MAX_CLAIM_LEASE_MS} ms with claim_lease_ms <= max_claim_lease_ms"
            ));
        }
        Ok(())
    }

    fn validate_dispatch(&self) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        if self.dispatch.kinds.iter().any(|kind| !seen.insert(*kind)) {
            return Err("dispatch.kinds has a duplicate kind".to_owned());
        }
        if self.dispatch.enabled {
            if self.dispatch.kinds.is_empty() {
                return Err("dispatch.kinds must name a kind when dispatch is enabled".to_owned());
            }
            if self.capture.is_none() {
                return Err("dispatch requires capture configuration".to_owned());
            }
        }
        Ok(())
    }
}

fn validate_capture(capture: &CaptureConfig) -> Result<(), String> {
    for (path, field) in [
        (&capture.git, "capture.git"),
        (&capture.tar, "capture.tar"),
        (&capture.export_root, "capture.export_root"),
        (
            &capture.verification_profiles,
            "capture.verification_profiles",
        ),
    ] {
        absolute(path, field)?;
    }
    if capture.repositories.len() > MAX_CAPTURE_REPOSITORIES {
        return Err("too many capture repositories".to_owned());
    }
    let mut seen = BTreeSet::new();
    for (repository, entry) in &capture.repositories {
        if !repository_name(repository) || !seen.insert(repository.to_ascii_lowercase()) {
            return Err(format!(
                "capture repository {repository:?} is not a unique owner/name"
            ));
        }
        absolute(&entry.authority, "capture.repositories.authority")?;
    }
    for (argv, field) in [
        (&capture.verify_argv, "capture.verify_argv"),
        (&capture.publish_argv, "capture.publish_argv"),
    ] {
        if let Some(argv) = argv {
            validate_argv(argv, field)?;
        }
    }
    Ok(())
}

fn validate_argv(argv: &[String], field: &str) -> Result<(), String> {
    let Some(program) = argv.first() else {
        return Err(format!("{field} must name a program"));
    };
    if argv.len() > MAX_ARGV || !Path::new(program).is_absolute() {
        return Err(format!(
            "{field} must start with an absolute program and hold at most {MAX_ARGV} arguments"
        ));
    }
    for argument in argv {
        if argument.len() > MAX_ARG_BYTES || argument.contains('\0') {
            return Err(format!("{field} has an oversized or NUL-bearing argument"));
        }
        // Placeholders are whole arguments; a brace anywhere else would be a
        // placeholder the operator expected to be substituted inside text.
        if (argument.contains('{') || argument.contains('}'))
            && !PLACEHOLDERS.contains(&argument.as_str())
        {
            return Err(format!(
                "{field} argument {argument:?} is not a whole known placeholder"
            ));
        }
    }
    Ok(())
}

/// Validate and normalize a `kiln-web-service` Factory Control Plane base URL.
///
/// Rejects control characters, whitespace, credentials, queries, fragments, and
/// non-HTTP(S) schemes so redirect `Location` headers cannot be injected.
///
/// # Errors
/// Returns a diagnostic when the URL is empty, oversized, malformed, or unsafe.
pub fn validate_console_url(raw: &str) -> Result<String, String> {
    if raw.is_empty() || raw.len() > MAX_CONSOLE_URL_BYTES {
        return Err(format!(
            "console_url must be non-empty and at most {MAX_CONSOLE_URL_BYTES} bytes"
        ));
    }
    if raw
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err("console_url must not contain whitespace or control characters".to_owned());
    }
    let after_scheme = if let Some(rest) = raw.strip_prefix("https://") {
        rest
    } else if let Some(rest) = raw.strip_prefix("http://") {
        rest
    } else {
        return Err("console_url must use http:// or https://".to_owned());
    };
    let normalized_rest = after_scheme.trim_end_matches('/');
    let authority = normalized_rest.split('/').next().unwrap_or_default();
    if authority.is_empty()
        || authority.contains('@')
        || authority.starts_with(':')
        || normalized_rest.contains('?')
        || normalized_rest.contains('#')
        || normalized_rest
            .bytes()
            .any(|byte| matches!(byte, b'<' | b'>' | b'"' | b'\'' | b'`' | b'\\'))
    {
        return Err(
            "console_url must be a clean http:// or https:// base URL without credentials, query, or fragment"
                .to_owned(),
        );
    }
    let scheme = raw
        .get(..raw.len().saturating_sub(after_scheme.len()))
        .unwrap_or_default();
    Ok(format!("{scheme}{normalized_rest}"))
}

/// Resolve the unified Factory Control Plane Fleet destination URL (`<base>/?page=fleet`).
///
/// # Errors
/// Returns a diagnostic when `base_url` is malformed or unsafe.
pub fn fleet_console_target(base_url: &str) -> Result<String, String> {
    let normalized = validate_console_url(base_url)?;
    Ok(format!("{normalized}/?page=fleet"))
}

fn absolute(path: &Path, field: &str) -> Result<(), String> {
    let text = path.to_string_lossy();
    if !path.is_absolute()
        || text.len() > MAX_PATH_BYTES
        || text.contains('\0')
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!("{field} must be an absolute path without `..`"));
    }
    Ok(())
}

/// `[a-z0-9][a-z0-9-]*`, bounded.
#[must_use]
pub fn machine_label(value: &str) -> bool {
    value.len() <= MAX_MACHINE_LABEL_BYTES
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn node_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_NODE_NAME_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-_".contains(&byte))
}

fn repository_name(value: &str) -> bool {
    let Some((owner, name)) = value.split_once('/') else {
        return false;
    };
    let part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    };
    value.len() <= MAX_REPOSITORY_BYTES && part(owner) && part(name)
}

fn group_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_GROUP_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-_".contains(&byte))
        && !value.starts_with('.')
        && !value.ends_with('.')
}

/// Tailscale's address ranges: 100.64.0.0/10 and fd7a:115c:a1e0::/48.
#[must_use]
pub fn tailnet(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            let [first, second, ..] = v4.octets();
            first == 100 && (second & 0xc0) == 64
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return tailnet(IpAddr::V4(v4));
            }
            let [first, second, third, ..] = v6.segments();
            first == 0xfd7a && second == 0x115c && third == 0xa1e0
        }
    }
}

/// Read at most `maximum` bytes from one file; a larger file is refused.
///
/// # Errors
/// Returns a diagnostic for an unreadable or oversized file.
pub fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(format!("{} exceeds {maximum} bytes", path.display()));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    pub(crate) fn document() -> Value {
        json!({
            "schema": "kiln.fleet.v1",
            "machine": "britton-desktop",
            "listen": ["127.0.0.1:8095", "100.110.43.11:8095"],
            "peers": { "aspen3-1.bison-tailor.ts.net.": "aspen3" },
            "operators": ["britton-desktop"],
            "tailscale": "/run/current-system/sw/bin/tailscale",
            "implementer": { "database": "/var/lib/kiln/implementer.sqlite", "enrollment": "/etc/kiln/enrollment.json" },
            "inventory": "/etc/kiln/fleet-inventory.json",
            "swarm": {
                "enabled": true,
                "executable": "/run/current-system/sw/bin/iroh-news",
                "state_dir": "/var/lib/kiln/swarm",
                "from": "kiln <kiln@swarm.local>"
            },
            "decision_group": "bots.general",
            "capture": {
                "git": "/run/current-system/sw/bin/git",
                "tar": "/run/current-system/sw/bin/tar",
                "export_root": "/var/lib/kiln/exports",
                "verification_profiles": "/etc/kiln/profiles",
                "repositories": { "OnixResearch/kiln": { "authority": "/etc/kiln/authority.json" } },
                "verify_argv": ["/run/current-system/sw/bin/kiln-web-service", "verify-implementation", "--attempt", "{attempt}", "--handoff", "{handoff}"],
                "publish_argv": null
            },
            "policy": { "tick_ms": 15000 },
            "dispatch": { "enabled": true, "kinds": ["omp", "claude"] }
        })
    }

    fn parse(value: &Value) -> Result<FleetConfig, String> {
        FleetConfig::from_json(&serde_json::to_vec(value).map_err(|error| error.to_string())?)
    }

    fn refused(edit: impl FnOnce(&mut Value)) -> bool {
        let mut value = document();
        edit(&mut value);
        parse(&value).is_err()
    }

    /// Set `pointer`, inserting the last key into its parent object.
    fn set(value: &mut Value, pointer: &str, replacement: Value) {
        let Some((parent, key)) = pointer.rsplit_once('/') else {
            return;
        };
        if let Some(Value::Object(object)) = value.pointer_mut(parent) {
            object.insert(key.to_owned(), replacement);
        }
    }

    #[test]
    fn core_observation_pins_version_and_refuses_ambiguous_hosts() {
        let mut value = document();
        value["herdr_core"] = json!({"socket":"/run/kiln/core.sock","version":"0.1.19","hosts":{"britton-desktop":"","aspen3":"west"}});
        assert!(parse(&value).is_ok());
        for (field, invalid) in [
            ("socket", json!("relative.sock")),
            ("version", json!("0.1.18")),
            ("hosts", json!({"britton-desktop":"west","aspen3":"west"})),
            ("hosts", json!({"not-enrolled":"west"})),
        ] {
            let mut rejected = value.clone();
            rejected["herdr_core"][field] = invalid;
            assert!(parse(&rejected).is_err(), "{field}");
        }
    }

    #[test]
    fn the_positive_document_is_admitted_with_defaults() {
        let config = parse(&document()).unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(config.policy.attempt_lease_ms, DEFAULT_ATTEMPT_LEASE_MS);
        assert_eq!(config.policy.claim_lease_ms, DEFAULT_CLAIM_LEASE_MS);
        assert!(config.is_operator("britton-desktop"));
        assert!(!config.is_operator("aspen3"));
        assert_eq!(
            config.machines().into_iter().collect::<Vec<_>>(),
            vec!["aspen3", "britton-desktop"]
        );
        let mut minimal = document();
        if let Some(object) = minimal.as_object_mut() {
            object.remove("decision_group");
            object.remove("policy");
        }
        set(&mut minimal, "/swarm", Value::Null);
        let minimal = parse(&minimal).unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(minimal.decision_group, DEFAULT_DECISION_GROUP);
        assert!(minimal.active_swarm().is_none());
    }

    #[test]
    fn invalid_documents_are_refused() {
        assert!(refused(|value| set(
            value,
            "/schema",
            json!("kiln.fleet.v2")
        )));
        assert!(refused(|value| set(value, "/machine", json!("Britton"))));
        assert!(refused(|value| set(value, "/listen", json!([]))));
        assert!(refused(|value| set(
            value,
            "/listen",
            json!(["0.0.0.0:8095"])
        )));
        assert!(refused(|value| set(
            value,
            "/listen",
            json!(["192.168.1.2:8095"])
        )));
        assert!(refused(|value| set(
            value,
            "/listen",
            json!(["localhost:8095"])
        )));
        assert!(refused(|value| set(
            value,
            "/listen",
            json!(["127.0.0.1:8095", "127.0.0.1:8095"])
        )));
        assert!(refused(|value| set(value, "/operators", json!(["aspen9"]))));
        assert!(refused(|value| set(
            value,
            "/peers",
            json!({"node": "Aspen"})
        )));
        assert!(refused(|value| set(
            value,
            "/tailscale",
            json!("tailscale")
        )));
        assert!(refused(|value| set(
            value,
            "/implementer/database",
            json!("/a/../b")
        )));
        assert!(refused(|value| set(
            value,
            "/decision_group",
            json!("bots general")
        )));
        assert!(refused(|value| {
            if let Some(object) = value.as_object_mut() {
                object.insert("unknown".into(), json!(1));
            }
        }));
        assert!(refused(|value| set(value, "/policy/max_nudges", json!(99))));
        assert!(refused(|value| set(
            value,
            "/policy/stale_keep_waiting_leases",
            json!(0)
        )));
        assert!(refused(|value| set(
            value,
            "/policy/claim_lease_ms",
            json!(1)
        )));
        assert!(refused(|value| set(value, "/dispatch/kinds", json!([]))));
        assert!(refused(|value| set(
            value,
            "/dispatch/kinds",
            json!(["gemini"])
        )));
        assert!(refused(|value| set(
            value,
            "/dispatch/kinds",
            json!(["omp", "omp"])
        )));
        assert!(refused(|value| set(value, "/capture", Value::Null)));
        assert!(refused(|value| set(
            value,
            "/capture/verify_argv",
            json!(["relative", "{attempt}"])
        )));
        assert!(refused(|value| set(
            value,
            "/capture/verify_argv",
            json!(["/bin/v", "--attempt={attempt}"])
        )));
        assert!(refused(|value| set(
            value,
            "/capture/verify_argv",
            json!(["/bin/v", "{secret}"])
        )));
        assert!(refused(|value| set(
            value,
            "/capture/repositories",
            json!({"not-a-repository": {"authority": "/a.json"}})
        )));
        // Dispatch off runs the claim authority alone and needs no capture.
        let mut claims_only = document();
        set(
            &mut claims_only,
            "/dispatch",
            json!({"enabled": false, "kinds": []}),
        );
        set(&mut claims_only, "/capture", Value::Null);
        assert!(parse(&claims_only).is_ok());
    }

    #[test]
    fn kiln_must_post_with_its_own_from_address() {
        for from in [
            "britton <britton-desktop@swarm.local>",
            "aspen3@swarm.local",
            "x <ASPEN3@SWARM.LOCAL>",
        ] {
            assert!(
                refused(|value| set(value, "/swarm/from", json!(from))),
                "{from}"
            );
        }
        let mut other = document();
        set(
            &mut other,
            "/swarm/from",
            json!("kiln-desktop <kiln-desktop@swarm.local>"),
        );
        assert!(parse(&other).is_ok());
    }

    #[test]
    fn tailnet_ranges_are_exact() {
        for (address, expected) in [
            ("100.64.0.1", true),
            ("100.127.255.255", true),
            ("100.128.0.1", false),
            ("100.63.255.255", false),
            ("fd7a:115c:a1e0::1", true),
            ("fd7a:115c:a1e1::1", false),
            ("::ffff:100.100.1.1", true),
        ] {
            let ip: IpAddr = address.parse().unwrap_or_else(|_| unreachable!());
            assert_eq!(tailnet(ip), expected, "{address}");
        }
    }

    #[test]
    fn console_url_validation_accepts_clean_http_urls_and_refuses_injection() {
        for (raw, expected) in [
            (
                "https://britton-desktop.bison-tailor.ts.net:9443",
                "https://britton-desktop.bison-tailor.ts.net:9443/?page=fleet",
            ),
            (
                "https://britton-desktop.bison-tailor.ts.net:9443/",
                "https://britton-desktop.bison-tailor.ts.net:9443/?page=fleet",
            ),
            (
                "http://127.0.0.1:8080/kiln/",
                "http://127.0.0.1:8080/kiln/?page=fleet",
            ),
            ("http://[::1]:8080", "http://[::1]:8080/?page=fleet"),
        ] {
            assert_eq!(fleet_console_target(raw).as_deref(), Ok(expected), "{raw}");
            let mut value = document();
            set(&mut value, "/console_url", json!(raw));
            assert!(parse(&value).is_ok(), "{raw}");
        }

        let oversized = format!("https://example.com/{}", "a".repeat(MAX_CONSOLE_URL_BYTES));
        for bad in [
            "",
            " ",
            "https://",
            "https:///fleet",
            "javascript:alert(1)",
            "ftp://example.com",
            "https://example.com\r\nX-Injected: 1",
            "https://example.com\n",
            "https://example.com/a b",
            "https://user@example.com",
            "https://:8080",
            "https://example.com/?x=1",
            "https://example.com/#frag",
            "https://example.com/<script>",
            &oversized,
        ] {
            assert!(validate_console_url(bad).is_err(), "{bad:?}");
            assert!(
                refused(|value| set(value, "/console_url", json!(bad))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn repository_scan_roots_are_optional_bounded_and_absolute() {
        let mut positive = document();
        set(
            &mut positive,
            "/repository_scan_roots",
            json!(["/home/brittonr/git", "/var/lib/kiln/repos"]),
        );
        let parsed = parse(&positive).unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(parsed.repository_scan_roots.len(), 2);

        assert!(refused(|value| set(
            value,
            "/repository_scan_roots",
            json!(["relative/path"])
        )));
        assert!(refused(|value| set(
            value,
            "/repository_scan_roots",
            json!(["/home/../etc"])
        )));
        let too_many: Vec<String> = (0..=MAX_REPOSITORY_SCAN_ROOTS)
            .map(|i| format!("/tmp/scan-{i}"))
            .collect();
        assert!(refused(|value| set(
            value,
            "/repository_scan_roots",
            json!(too_many)
        )));
    }
}
