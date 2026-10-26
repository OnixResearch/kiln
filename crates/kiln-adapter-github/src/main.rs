use std::path::PathBuf;

use kiln_adapter_github::profile::load_config;
use kiln_adapter_github::server::{open_store, runtime, serve};
use kiln_core::types::ContentId;
use kiln_provider_api::PortFailure;

const USAGE: &str = "usage: kiln-adapter-github --profile PROFILE.json\n       kiln-adapter-github status --profile PROFILE.json --run RUN_BLAKE3\n       kiln-adapter-github reconcile --profile PROFILE.json --run RUN_BLAKE3\n       kiln-adapter-github export --profile PROFILE.json --run RUN_BLAKE3 --output JOURNAL.json [--database-snapshot SNAPSHOT.sqlite]";

fn run() -> Result<(), PortFailure> {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    if first.as_deref() == Some(kiln_adapter_github::status::DNS_CHILD_ARGUMENT) {
        let parent = args
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or_else(arguments)?;
        if args.next().is_some() {
            return Err(arguments());
        }
        std::process::exit(kiln_adapter_github::status::run_dns_child(parent));
    }
    if first.as_deref() == Some("--help") && args.next().is_none() {
        println!("{USAGE}");
        return Ok(());
    }
    let mode = match first.as_deref() {
        Some("--profile") => "serve",
        Some("status") => "status",
        Some("reconcile") => "reconcile",
        Some("export") => "export",
        _ => return Err(arguments()),
    };
    if mode != "serve" && args.next().as_deref() != Some("--profile") {
        return Err(arguments());
    }
    let profile = args.next().map(PathBuf::from).ok_or_else(arguments)?;
    let run_identity = if mode == "serve" {
        None
    } else {
        if args.next().as_deref() != Some("--run") {
            return Err(arguments());
        }
        Some(ContentId::from_blake3_hex(
            args.next().ok_or_else(arguments)?,
        )?)
    };
    let output = if mode == "export" {
        if args.next().as_deref() != Some("--output") {
            return Err(arguments());
        }
        Some(args.next().map(PathBuf::from).ok_or_else(arguments)?)
    } else {
        None
    };
    let database_snapshot = match args.next().as_deref() {
        Some("--database-snapshot") if mode == "export" => {
            Some(args.next().map(PathBuf::from).ok_or_else(arguments)?)
        }
        None => None,
        _ => return Err(arguments()),
    };
    if args.next().is_some() {
        return Err(arguments());
    }
    let admitted = load_config(&profile)?;
    if let Some(run) = run_identity {
        if let Some(output) = output {
            return kiln_adapter_github::export::export_observation(
                &admitted,
                &profile,
                &run,
                &output,
                database_snapshot.as_deref(),
            );
        }
        let record = if mode == "reconcile" {
            runtime(&admitted)?.reconcile_run(&run)?
        } else {
            open_store(&admitted)?.load(&run)?.ok_or_else(|| {
                PortFailure::Domain(kiln_core::types::Diagnostic::new(
                    "github_run_missing",
                    "github.run",
                    "GitHub run does not exist",
                ))
            })?
        };
        let value = serde_json::json!({
            "schema":"kiln.github-run-status.v1", "run_identity":record.run_identity,
            "run_id":record.run_id, "phase":record.phase, "outcome":record.outcome()?,
            "source":record.plan.source, "workflow_polls":record.workflow_polls,
            "statuses":record.statuses, "error_code":record.error_code,
        });
        println!(
            "{}",
            serde_json::to_string(&value).map_err(|_| arguments())?
        );
        Ok(())
    } else {
        serve(admitted)
    }
}

fn arguments() -> PortFailure {
    PortFailure::Domain(kiln_core::types::Diagnostic::new(
        "github_arguments",
        "github.arguments",
        USAGE,
    ))
}

fn main() {
    if let Err(error) = run() {
        match error {
            PortFailure::Domain(error) => eprintln!("{}: {}", error.code, error.message),
            PortFailure::Infrastructure(error)
            | PortFailure::Retryable(error)
            | PortFailure::TimedOut(error)
            | PortFailure::Cancelled(error)
            | PortFailure::Unknown(error) => eprintln!("{}: {}", error.code, error.message),
        }
        std::process::exit(1);
    }
}
