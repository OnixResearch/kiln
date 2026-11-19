//! Operator configuration for the renews/iroh swarm client.
//!
//! The client never receives credentials. It runs the pinned `iroh-news`
//! executable with a cleared environment, and that client resolves its own
//! endpoint key and password through SecretSpec or its state directory.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_GROUP: &str = "bots.claims";
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_READ_LIMIT: usize = 50;
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;
pub const MIN_TIMEOUT_MS: u64 = 100;
pub const MAX_TIMEOUT_MS: u64 = 300_000;
pub const MAX_READ_LIMIT: usize = 500;
pub const MAX_OUTPUT_BYTES: usize = 4 * 1_048_576;
pub const MAX_GROUP_BYTES: usize = 64;
pub const MAX_FROM_BYTES: usize = 256;
pub const MAX_SERVER_BYTES: usize = 512;

/// One explicit environment entry handed to the client process.
pub type Environment = BTreeMap<String, String>;

/// Reviewed swarm mirror configuration. Disabled unless an operator supplies it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmConfig {
    /// Absolute path to the pinned `iroh-news` executable.
    pub executable: PathBuf,
    /// State directory holding the client key and cursors.
    pub state_dir: PathBuf,
    /// Optional generated client configuration file.
    #[serde(default)]
    pub config_file: Option<PathBuf>,
    /// Optional server ticket, endpoint id or DNS name.
    #[serde(default)]
    pub server: Option<String>,
    /// Claim group.
    #[serde(default = "default_group")]
    pub group: String,
    /// Our `From` header, used to tell our claims from other authors'.
    pub from: String,
    /// Per-command deadline.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Maximum articles returned by one read.
    #[serde(default = "default_read_limit")]
    pub read_limit: usize,
    /// Maximum retained stdout/stderr bytes.
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,
    /// Dial only ticket addresses instead of n0 relays.
    #[serde(default)]
    pub no_relays: bool,
    /// Explicit child environment; the ambient environment is cleared.
    #[serde(default)]
    pub environment: Environment,
}

fn default_group() -> String {
    DEFAULT_GROUP.to_owned()
}

const fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

const fn default_read_limit() -> usize {
    DEFAULT_READ_LIMIT
}

const fn default_max_output_bytes() -> usize {
    DEFAULT_MAX_OUTPUT_BYTES
}

impl SwarmConfig {
    /// Validate the configuration without contacting the server.
    ///
    /// # Errors
    /// Returns a bounded diagnostic for a non-absolute path, an invalid group,
    /// an invalid `From` header, an out-of-range limit, or an invalid
    /// environment entry.
    pub fn validate(&self) -> Result<(), String> {
        if !self.executable.is_absolute() {
            return Err("swarm executable must be an absolute path".to_owned());
        }
        if !self.state_dir.is_absolute() {
            return Err("swarm state directory must be an absolute path".to_owned());
        }
        if self
            .config_file
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
        {
            return Err("swarm config file must be an absolute path".to_owned());
        }
        if let Some(server) = &self.server {
            if !bounded_text(server, MAX_SERVER_BYTES) {
                return Err("swarm server is invalid".to_owned());
            }
        }
        if !group(&self.group) {
            return Err("swarm group is invalid".to_owned());
        }
        if !bounded_text(&self.from, MAX_FROM_BYTES) {
            return Err("swarm From header is invalid".to_owned());
        }
        if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&self.timeout_ms) {
            return Err("swarm timeout is out of range".to_owned());
        }
        if !(1..=MAX_READ_LIMIT).contains(&self.read_limit) {
            return Err("swarm read limit is out of range".to_owned());
        }
        if !(1_024..=MAX_OUTPUT_BYTES).contains(&self.max_output_bytes) {
            return Err("swarm output bound is out of range".to_owned());
        }
        for (name, value) in &self.environment {
            if !environment_name(name)
                || name.contains("PASSWORD")
                || name.contains("SECRET_KEY")
                || value.len() > MAX_SERVER_BYTES
                || value.bytes().any(|byte| byte == 0)
            {
                return Err("swarm environment entry is invalid".to_owned());
            }
        }
        Ok(())
    }
}

fn group(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_GROUP_BYTES
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-_".contains(&byte))
        && !value.starts_with('.')
        && !value.ends_with('.')
}

fn bounded_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && value.is_ascii()
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn environment_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        && name.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> SwarmConfig {
        SwarmConfig {
            executable: PathBuf::from("/nix/store/iroh-news/bin/iroh-news"),
            state_dir: PathBuf::from("/var/lib/kiln/swarm"),
            config_file: None,
            server: Some("swarm.blr.dev".to_owned()),
            group: DEFAULT_GROUP.to_owned(),
            from: "kiln-aspen1 <kiln-aspen1@swarm.local>".to_owned(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            read_limit: DEFAULT_READ_LIMIT,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            no_relays: true,
            environment: Environment::new(),
        }
    }

    #[test]
    fn valid_configuration_is_admitted() {
        config().validate().unwrap_or_else(|_| unreachable!());
    }

    #[test]
    fn invalid_configuration_is_refused() {
        let mut relative = config();
        relative.executable = PathBuf::from("iroh-news");
        assert!(relative.validate().is_err());

        let mut group = config();
        group.group = "bots claims".to_owned();
        assert!(group.validate().is_err());

        let mut timeout = config();
        timeout.timeout_ms = 0;
        assert!(timeout.validate().is_err());

        let mut environment = config();
        environment
            .environment
            .insert("IROH_NEWS_PASSWORD".to_owned(), "x".to_owned());
        assert!(environment.validate().is_err());

        let mut from = config();
        from.from = "kiln\nforged".to_owned();
        assert!(from.validate().is_err());
    }
}
