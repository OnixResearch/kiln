//! Read-only projection of an explicitly configured deployment owner's export.
use std::{collections::BTreeSet, fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(crate) const SCHEMA: &str = "kiln.delivery-environments.v1";
const MAX_BYTES: u64 = 128 * 1024;
const MAX_ROWS: usize = 128;
const FRESH_MS: u64 = 5 * 60 * 1000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    schema: String,
    observations: Vec<Observation>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    environment: String,
    service: String,
    owner: String,
    intended_revision: Option<String>,
    observed_revision: Option<String>,
    rollback_revision: Option<String>,
    observed_at_ms: u64,
    health: Health,
    blocker: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Health { Healthy, Degraded, Unhealthy, Unknown }

fn label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-:/".contains(&byte))
}

fn revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn project(input: &[u8], now_ms: u64) -> Result<Value, &'static str> {
    let export: Export = serde_json::from_slice(input).map_err(|_| "delivery_observation_invalid")?;
    let mut seen = BTreeSet::new();
    if export.schema != SCHEMA || export.observations.len() > MAX_ROWS || export.observations.iter().any(|row| {
        !label(&row.environment) || !label(&row.service) || !label(&row.owner)
            || !seen.insert((row.environment.as_str(), row.service.as_str()))
            || [&row.intended_revision, &row.observed_revision, &row.rollback_revision].into_iter().flatten().any(|id| !revision(id))
            || row.blocker.as_ref().is_some_and(|value| value.is_empty() || value.len() > 512 || value.chars().any(char::is_control))
    }) { return Err("delivery_observation_invalid"); }
    let observations: Vec<_> = export.observations.into_iter().map(|row| {
        let fresh = now_ms.checked_sub(row.observed_at_ms).is_some_and(|age| age <= FRESH_MS);
        let freshness = if fresh { "fresh" } else { "stale" };
        let mut value = json!(row);
        value["freshness"] = json!(freshness);
        value
    }).collect();
    Ok(json!({"schema":SCHEMA,"configured":true,"observations":observations}))
}

pub(crate) fn read(path: Option<&Path>, now_ms: u64) -> Result<Value, &'static str> {
    let Some(path) = path else { return Ok(json!({"schema":SCHEMA,"configured":false,"observations":[]})); };
    if !path.is_absolute() { return Err("delivery_observation_invalid"); }
    let file = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)
        .map_err(|_| "delivery_observation_unavailable")?;
    let meta = file.metadata().map_err(|_| "delivery_observation_unavailable")?;
    if !meta.is_file() || meta.len() > MAX_BYTES { return Err("delivery_observation_invalid"); }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|_| "delivery_observation_unavailable")?;
    if bytes.len() as u64 > MAX_BYTES { return Err("delivery_observation_invalid"); }
    project(&bytes, now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_readback_never_borrows_desired_or_other_target() -> Result<(), Box<dyn std::error::Error>> {
        let mut export = json!({"schema":SCHEMA,"observations":[{
            "environment":"production","service":"kiln","owner":"onixos", "intended_revision":"a".repeat(40),
            "observed_revision":null,"observed_at_ms":1,"health":"unknown","blocker":"Host claim in use"
        }]});
        let readback = project(&serde_json::to_vec(&export)?, FRESH_MS + 2)?;
        assert!(readback["observations"][0]["observed_revision"].is_null());
        assert_eq!(readback["observations"][0]["freshness"], "stale");
        let duplicate = export["observations"][0].clone();
        if let Some(rows) = export["observations"].as_array_mut() { rows.push(duplicate); }
        assert!(project(&serde_json::to_vec(&export)?, 1).is_err());
        Ok(())
    }
}
