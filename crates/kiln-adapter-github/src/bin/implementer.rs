//! Operator shell for the durable implementation ledger.
//!
//! `drain` moves authenticated queued GitHub issue deliveries into admitted
//! work. `status` reports retained work and attempt counts. Neither command
//! claims, dispatches, verifies or publishes implementation work.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use kiln_adapter_github::implementation::{ImplementationIssuePolicy, drain_issue_queue};
use kiln_adapter_github::profile::ImplementationQueueConfig;
use kiln_core::implementation::Enrollment;
use kiln_implementer::Implementer;

const EXIT_ERROR: i32 = 2;

// r[impl kiln.implementation.operator_cli]
fn main() {
    if run().is_err() {
        std::process::exit(EXIT_ERROR);
    }
}

fn run() -> Result<(), ()> {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        return fail("usage: kiln-implementer <drain|status> --database PATH ...");
    };
    let flags = match parse_flags(arguments.collect()) {
        Ok(flags) => flags,
        Err(error) => return fail(&error),
    };
    match command.as_str() {
        "drain" => drain(&flags),
        "status" => status(&flags),
        _ => fail("unknown command"),
    }
}

fn drain(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let database = match required_path(flags, "--database") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let queue = match required_path(flags, "--queue") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let secret_file = match required_path(flags, "--secret-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let policy_file = match required_path(flags, "--policy") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let enrollment_file = match required_path(flags, "--enrollment") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let maximum_deliveries = match required_positive(flags, "--maximum-deliveries") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let limit = match required_positive(flags, "--limit") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let secret = match std::fs::read(&secret_file) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        _ => return fail("secret file is empty or unreadable"),
    };
    let policy: ImplementationIssuePolicy = match read_json(&policy_file) {
        Ok(policy) => policy,
        Err(error) => return fail(&error),
    };
    let enrollment: Enrollment = match read_json(&enrollment_file) {
        Ok(enrollment) => enrollment,
        Err(error) => return fail(&error),
    };
    let mut implementer = match Implementer::open(&database, enrollment.clone()) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let actors: BTreeSet<String> = enrollment.actors;
    let config = ImplementationQueueConfig {
        path: queue,
        maximum_deliveries,
    };
    match drain_issue_queue(&config, &secret, &policy, &actors, &mut implementer, limit) {
        Ok(drained) => {
            println!("{{\"drained\":{drained}}}");
            Ok(())
        }
        Err(error) => fail(&format!("{error:?}")),
    }
}

fn status(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let database = match required_path(flags, "--database") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let enrollment_file = match required_path(flags, "--enrollment") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let enrollment: Enrollment = match read_json(&enrollment_file) {
        Ok(enrollment) => enrollment,
        Err(error) => return fail(&error),
    };
    let mut implementer = match Implementer::open(&database, enrollment) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let work = match implementer.work() {
        Ok(work) => work,
        Err(error) => return fail(&error),
    };
    let attempts = match implementer.attempts() {
        Ok(attempts) => attempts,
        Err(error) => return fail(&error),
    };
    let active = attempts
        .attempts()
        .filter(|attempt| attempt.phase.is_active())
        .count();
    println!(
        "{{\"work\":{},\"attempts\":{},\"active\":{active}}}",
        work.current().count(),
        attempts.attempts().count()
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

fn required_positive(flags: &BTreeMap<String, String>, key: &str) -> Result<usize, String> {
    let value = flags.get(key).ok_or_else(|| format!("{key} is required"))?;
    value
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{key} must be positive"))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|_| format!("{} is unreadable", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|_| format!("{} is not valid JSON", path.display()))
}

fn fail(message: &str) -> Result<(), ()> {
    eprintln!("kiln-implementer: {message}");
    Err(())
}
