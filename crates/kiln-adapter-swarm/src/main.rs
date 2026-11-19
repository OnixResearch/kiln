//! Operator CLI for the renews/iroh swarm claim client.
//!
//! Usage:
//!   kiln-swarm read [--config-json FILE | --executable PATH --state-dir DIR --from HEADER]
//!                   [--server NAME] [--config FILE] [--group G] [--no-relays]
//!                   [--timeout-ms N] [--since N] [--limit N]
//!   kiln-swarm post --subject SUBJECT [--references MESSAGE_ID] [...same flags]
//!
//! `post` reads the body from stdin and always prints a post receipt. `read`
//! prints the article array or exits non-zero when the group is unavailable.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::PathBuf;

use kiln_adapter_swarm::{PostOutcome, SwarmClient, SwarmConfig};
use kiln_core::implementation::swarm::MAX_BODY_BYTES;

const STDIN_LIMIT_BYTES: u64 = MAX_BODY_BYTES as u64 + 2;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        return Err(usage());
    };
    let flags = parse_flags(arguments.collect())?;
    let config = load_config(&flags)?;
    let client = SwarmClient::new(config)?;
    match command.as_str() {
        "read" => {
            let since = optional_u64(&flags, "--since")?;
            let limit =
                optional_usize(&flags, "--limit")?.unwrap_or_else(|| client.config().read_limit);
            let articles = client
                .read(since, limit)
                .map_err(|failure| failure.detail)?;
            println!(
                "{}",
                serde_json::to_string(&articles).map_err(|_| "article encoding failed")?
            );
            Ok(())
        }
        "post" => {
            let subject = required(&flags, "--subject")?;
            let references = flags.get("--references").map(String::as_str);
            let mut body = String::new();
            std::io::stdin()
                .take(STDIN_LIMIT_BYTES)
                .read_to_string(&mut body)
                .map_err(|_| "swarm body could not be read")?;
            let receipt = client.post(subject, references, &body);
            println!(
                "{}",
                serde_json::to_string(&receipt).map_err(|_| "receipt encoding failed")?
            );
            if receipt.outcome == PostOutcome::Rejected {
                return Err(receipt.detail.unwrap_or_else(|| "post rejected".to_owned()));
            }
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn load_config(flags: &BTreeMap<String, String>) -> Result<SwarmConfig, String> {
    if let Some(path) = flags.get("--config-json") {
        let text =
            std::fs::read_to_string(path).map_err(|_| "swarm configuration could not be read")?;
        return serde_json::from_str(&text)
            .map_err(|_| "swarm configuration is invalid".to_owned());
    }
    Ok(SwarmConfig {
        enabled: true,
        executable: PathBuf::from(required(flags, "--executable")?),
        state_dir: PathBuf::from(required(flags, "--state-dir")?),
        config_file: flags.get("--config").map(PathBuf::from),
        server: flags.get("--server").cloned(),
        group: flags
            .get("--group")
            .cloned()
            .unwrap_or_else(|| kiln_adapter_swarm::config::DEFAULT_GROUP.to_owned()),
        from: required(flags, "--from")?.to_owned(),
        timeout_ms: optional_u64(flags, "--timeout-ms")?
            .unwrap_or(kiln_adapter_swarm::config::DEFAULT_TIMEOUT_MS),
        read_limit: optional_usize(flags, "--read-limit")?
            .unwrap_or(kiln_adapter_swarm::config::DEFAULT_READ_LIMIT),
        max_output_bytes: optional_usize(flags, "--max-output-bytes")?
            .unwrap_or(kiln_adapter_swarm::config::DEFAULT_MAX_OUTPUT_BYTES),
        no_relays: !flags.contains_key("--relays"),
        environment: BTreeMap::new(),
    })
}

fn parse_flags(arguments: Vec<String>) -> Result<BTreeMap<String, String>, String> {
    let mut flags = BTreeMap::new();
    let mut arguments = arguments.into_iter();
    while let Some(flag) = arguments.next() {
        if !flag.starts_with("--") {
            return Err(usage());
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        flags.insert(flag, value);
    }
    Ok(flags)
}

fn required<'a>(flags: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str, String> {
    flags
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} is required"))
}

fn optional_u64(flags: &BTreeMap<String, String>, name: &str) -> Result<Option<u64>, String> {
    flags
        .get(name)
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| format!("{name} must be a number"))
        })
        .transpose()
}

fn optional_usize(flags: &BTreeMap<String, String>, name: &str) -> Result<Option<usize>, String> {
    flags
        .get(name)
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| format!("{name} must be a number"))
        })
        .transpose()
}

fn usage() -> String {
    "usage: kiln-swarm <read|post> [--config-json FILE | --executable PATH --state-dir DIR --from HEADER] [--server NAME] [--config FILE] [--group G] [--timeout-ms N] [--since N] [--limit N] [--subject TEXT] [--references ID]".to_owned()
}
