use std::{env, fs, process::ExitCode};

use drift::{ChangeClass, compare};
use serde_json::Value;

// r[impl kiln.web.http_contract.drift] Drift's directional incompatibility classes
// have no semantic ordering; an unhandled comparison always fails closed.
fn policy(classes: impl IntoIterator<Item = ChangeClass>, version_changed: bool) -> bool {
    classes.into_iter().all(|class| match class {
        ChangeClass::Trivial => true,
        ChangeClass::BackwardIncompatible
        | ChangeClass::ForwardIncompatible
        | ChangeClass::Incompatible => version_changed,
        ChangeClass::Unhandled => false,
    })
}

fn version(document: &Value) -> Result<&str, String> {
    document
        .pointer("/info/version")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "missing nonempty info.version".to_owned())
}

fn run(baseline: &str, current: &str) -> Result<bool, String> {
    let old: Value =
        serde_json::from_str(baseline).map_err(|error| format!("baseline JSON: {error}"))?;
    let new: Value =
        serde_json::from_str(current).map_err(|error| format!("current JSON: {error}"))?;
    let version_changed = version(&old)? != version(&new)?;
    let changes = compare(&old, &new).map_err(|error| format!("Drift comparison: {error:#}"))?;
    let mut records = Vec::new();
    let mut classes = Vec::new();
    for change in changes {
        let mut paths: Vec<_> = change
            .paths
            .iter()
            .map(|path| {
                format!(
                    "{:?} old={:?} new={:?}",
                    path.comparison, path.old, path.new
                )
            })
            .collect();
        paths.sort();
        for info in change.changes {
            classes.push(info.class);
            records.push(format!(
                "{:?} {:?}: {} -> {}: {} [{}]",
                info.class,
                info.details,
                info.old_subpath,
                info.new_subpath,
                info.message,
                paths.join(", ")
            ));
        }
    }
    records.sort();
    for record in records {
        println!("{record}");
    }
    let passed = policy(classes, version_changed);
    println!(
        "decision={} version_changed={version_changed}",
        if passed { "pass" } else { "fail" }
    );
    Ok(passed)
}

fn exit_code(result: Result<bool, String>) -> ExitCode {
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(_) => ExitCode::from(2),
    }
}

fn main() -> ExitCode {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: kiln-http-contract-drift BASELINE.json CURRENT.json");
        return ExitCode::from(2);
    }
    let result = fs::read_to_string(&args[1])
        .map_err(|error| format!("baseline {}: {error}", args[1]))
        .and_then(|baseline| {
            fs::read_to_string(&args[2])
                .map_err(|error| format!("current {}: {error}", args[2]))
                .and_then(|current| run(&baseline, &current))
        });
    if let Err(error) = &result {
        eprintln!("{error}");
    }
    exit_code(result)
}
#[cfg(test)]
mod tests {
    use super::*;

    // r[verify kiln.web.http_contract.drift.scenario.policy]
    #[test]
    fn incompatible_requires_version_change_in_all_directions() {
        for class in [
            ChangeClass::BackwardIncompatible,
            ChangeClass::ForwardIncompatible,
            ChangeClass::Incompatible,
        ] {
            assert!(!policy([class], false));
            assert!(policy([class], true));
        }
    }

    // r[verify kiln.web.http_contract.drift.scenario.policy]
    #[test]
    fn tool_error_is_not_a_policy_rejection() {
        assert_eq!(exit_code(Ok(true)), ExitCode::SUCCESS);
        assert_eq!(exit_code(Ok(false)), ExitCode::from(1));
        assert_eq!(exit_code(Err("invalid document".into())), ExitCode::from(2));
    }
    // r[verify kiln.web.http_contract.drift.scenario.policy]
    #[test]
    fn unhandled_fails_even_on_version_change_and_trivial_passes() {
        assert!(policy([ChangeClass::Trivial], false));
        assert!(policy([ChangeClass::Trivial], true));
        assert!(!policy([ChangeClass::Unhandled], false));
        assert!(!policy([ChangeClass::Unhandled], true));
        assert!(!policy(
            [ChangeClass::Trivial, ChangeClass::Unhandled],
            true
        ));
        assert!(policy([], false));
    }

    // r[verify kiln.web.http_contract.drift]
    #[test]
    fn malformed_versions_fail_closed() {
        assert!(version(&serde_json::json!({"info": {}})).is_err());
        assert!(version(&serde_json::json!({"info": {"version": ""}})).is_err());
        assert_eq!(
            version(&serde_json::json!({"info": {"version": "v1"}})),
            Ok("v1")
        );
    }
}
