//! Admit one Cairn implementation-ready snapshot into the durable ledger.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kiln_adapter_cairn::implementation::{CairnImplementationPolicy, admit_cairn_snapshot};
use kiln_core::implementation::Enrollment;
use kiln_implementer::Implementer;

// r[impl kiln.implementation.contract]
fn main() {
    if run().is_err() {
        std::process::exit(2);
    }
}

fn run() -> Result<(), ()> {
    let flags = match parse_flags(std::env::args().skip(1).collect()) {
        Ok(flags) => flags,
        Err(error) => return fail(&error),
    };
    let database = match required_path(&flags, "--database") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let enrollment_file = match required_path(&flags, "--enrollment") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let snapshot_file = match required_path(&flags, "--snapshot") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let policy_file = match required_path(&flags, "--policy") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let sequence = match flags.get("--sequence") {
        Some(value) => match value.parse::<u64>() {
            Ok(sequence) => sequence,
            Err(_) => return fail("--sequence must be a non-negative integer"),
        },
        None => return fail("--sequence is required"),
    };
    let enrollment: Enrollment = match read_json(&enrollment_file) {
        Ok(enrollment) => enrollment,
        Err(error) => return fail(&error),
    };
    let policy: CairnImplementationPolicy = match read_json(&policy_file) {
        Ok(policy) => policy,
        Err(error) => return fail(&error),
    };
    let snapshot = match std::fs::read(&snapshot_file) {
        Ok(bytes) => bytes,
        Err(_) => return fail("snapshot file is unreadable"),
    };
    let admission = match admit_cairn_snapshot(&policy, sequence, &snapshot) {
        Ok(admission) => admission,
        Err(error) => return fail(&format!("{error:?}")),
    };
    let mut implementer = match Implementer::open(&database, enrollment) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let mut admitted = 0usize;
    let mut duplicates = 0usize;
    let mut superseded = 0usize;
    for work in &admission.works {
        match implementer.admit(work, None) {
            Ok(kiln_core::implementation::AdmissionDecision::Duplicate { .. }) => {
                duplicates += 1;
            }
            Ok(kiln_core::implementation::AdmissionDecision::Superseded { .. }) => {
                superseded += 1;
            }
            Ok(_) => admitted += 1,
            Err(error) => return fail(&error),
        }
    }
    println!(
        "{{\"outcome\":\"{}\",\"selected\":{},\"admitted\":{admitted},\"duplicate\":{duplicates},\"superseded\":{superseded}}}",
        admission.outcome,
        admission.works.len()
    );
    Ok(())
}

fn parse_flags(values: Vec<String>) -> Result<BTreeMap<String, String>, String> {
    let mut flags = BTreeMap::new();
    let mut values = values.into_iter();
    while let Some(flag) = values.next() {
        if !flag.starts_with("--") {
            return Err(format!("unexpected argument {flag}"));
        }
        let value = values
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        if flags.insert(flag.clone(), value).is_some() {
            return Err(format!("{flag} is repeated"));
        }
    }
    Ok(flags)
}

fn required_path(flags: &BTreeMap<String, String>, key: &str) -> Result<PathBuf, String> {
    let value = flags.get(key).ok_or_else(|| format!("{key} is required"))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    Ok(path)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|_| format!("{} is unreadable", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|_| format!("{} is not valid JSON", path.display()))
}

fn fail(message: &str) -> Result<(), ()> {
    eprintln!("kiln-cairn-intake: {message}");
    Err(())
}
