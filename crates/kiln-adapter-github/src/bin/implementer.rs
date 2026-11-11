//! Operator shell for the durable implementation ledger.
//!
//! `drain` moves authenticated queued GitHub issue deliveries into admitted
//! work once. `run` repeats that drain on a bounded interval until a stop signal
//! or an explicit iteration count. `status` reports retained work and attempt
//! counts. None of the commands claim, dispatch, verify or publish work.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use kiln_adapter_github::implementation::{ImplementationIssuePolicy, drain_issue_queue};
use kiln_adapter_github::profile::ImplementationQueueConfig;
use kiln_core::implementation::Enrollment;
use kiln_implementer::Implementer;

const EXIT_ERROR: i32 = 2;

struct DrainInputs {
    database: PathBuf,
    queue: PathBuf,
    secret: Vec<u8>,
    policy: ImplementationIssuePolicy,
    enrollment: Enrollment,
    maximum_deliveries: usize,
    limit: usize,
}

// r[impl kiln.implementation.operator_cli]
fn main() {
    if run().is_err() {
        std::process::exit(EXIT_ERROR);
    }
}

fn run() -> Result<(), ()> {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        return fail("usage: kiln-implementer <drain|run|status> --database PATH ...");
    };
    let flags = match parse_flags(arguments.collect()) {
        Ok(flags) => flags,
        Err(error) => return fail(&error),
    };
    match command.as_str() {
        "drain" => drain(&flags),
        "run" => run_loop(&flags),
        "status" => status(&flags),
        _ => fail("unknown command"),
    }
}

fn drain(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let inputs = match drain_inputs(flags) {
        Ok(inputs) => inputs,
        Err(error) => return fail(&error),
    };
    match drain_once(&inputs) {
        Ok(drained) => {
            println!("{{\"drained\":{drained}}}");
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.drain_loop]
fn run_loop(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let inputs = match drain_inputs(flags) {
        Ok(inputs) => inputs,
        Err(error) => return fail(&error),
    };
    let interval_ms = match required_positive(flags, "--interval-ms") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let iterations = match optional_positive(flags, "--iterations") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        if signal_hook::flag::register(signal, Arc::clone(&stop)).is_err() {
            return fail("shutdown signal registration failed");
        }
    }
    let mut implementer = match Implementer::open(&inputs.database, inputs.enrollment.clone()) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let mut done = 0usize;
    let mut drained = 0usize;
    loop {
        match drain_into(&inputs, &mut implementer) {
            Ok(count) => drained = drained.saturating_add(count),
            Err(error) => return fail(&error),
        }
        done += 1;
        if iterations.is_some_and(|limit| done >= limit) || stop.load(Ordering::Acquire) {
            break;
        }
        std::thread::sleep(Duration::from_millis(
            u64::try_from(interval_ms).unwrap_or(u64::MAX),
        ));
    }
    println!("{{\"iterations\":{done},\"drained\":{drained}}}");
    Ok(())
}

fn drain_once(inputs: &DrainInputs) -> Result<usize, String> {
    let mut implementer = Implementer::open(&inputs.database, inputs.enrollment.clone())
        .map_err(|error| error.to_string())?;
    drain_into(inputs, &mut implementer)
}

fn drain_into(inputs: &DrainInputs, implementer: &mut Implementer) -> Result<usize, String> {
    let config = ImplementationQueueConfig {
        path: inputs.queue.clone(),
        maximum_deliveries: inputs.maximum_deliveries,
    };
    drain_issue_queue(
        &config,
        &inputs.secret,
        &inputs.policy,
        &inputs.enrollment.actors,
        implementer,
        inputs.limit,
    )
    .map_err(|error| format!("{error:?}"))
}

fn drain_inputs(flags: &BTreeMap<String, String>) -> Result<DrainInputs, String> {
    let database = required_path(flags, "--database")?;
    let queue = required_path(flags, "--queue")?;
    let secret_file = required_path(flags, "--secret-file")?;
    let policy_file = required_path(flags, "--policy")?;
    let enrollment_file = required_path(flags, "--enrollment")?;
    let maximum_deliveries = required_positive(flags, "--maximum-deliveries")?;
    let limit = required_positive(flags, "--limit")?;
    let secret = match std::fs::read(&secret_file) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        _ => return Err("secret file is empty or unreadable".to_string()),
    };
    let policy: ImplementationIssuePolicy = read_json(&policy_file)?;
    let enrollment: Enrollment = read_json(&enrollment_file)?;
    Ok(DrainInputs {
        database,
        queue,
        secret,
        policy,
        enrollment,
        maximum_deliveries,
        limit,
    })
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

fn optional_positive(flags: &BTreeMap<String, String>, key: &str) -> Result<Option<usize>, String> {
    match flags.get(key) {
        None => Ok(None),
        Some(value) => value
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .map(Some)
            .ok_or_else(|| format!("{key} must be positive")),
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|_| format!("{} is unreadable", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|_| format!("{} is not valid JSON", path.display()))
}

fn fail(message: &str) -> Result<(), ()> {
    eprintln!("kiln-implementer: {message}");
    Err(())
}
