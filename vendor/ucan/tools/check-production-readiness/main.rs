//! Explicit readiness catalog, profile, fixture, and documentation checker.

mod artifacts;
mod cli;
mod error;
mod nickel;
mod suite;

use std::env;
use std::process::ExitCode;

use cli::Mode;
use error::ShellError;
use ucan::integration_profile::support_catalog;
use ucan::integration_profile::support_catalog_blake3;

const FAILURE_EXIT_CODE: u8 = 1;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("production-readiness: {error}");
            ExitCode::from(FAILURE_EXIT_CODE)
        }
    }
}

fn run() -> Result<(), ShellError> {
    let arguments = env::args().collect::<Vec<_>>();
    let options = cli::parse(&arguments)?;
    let catalog = support_catalog();
    let identity = support_catalog_blake3(&catalog).map_err(|error| {
        ShellError::new(
            "catalog",
            error
                .issues
                .iter()
                .map(|issue| format!("{}:{}:{}", issue.code, issue.path, issue.message))
                .collect::<Vec<_>>()
                .join("; "),
        )
    })?;
    match options.mode {
        Mode::Generate => {
            artifacts::generate(&options.root, &catalog)?;
            println!("production-readiness: GENERATED catalog_blake3={identity}");
        }
        Mode::Check => {
            artifacts::check(&options.root, &catalog)?;
            let stats = suite::run(&options.root, &options.nickel, &catalog)?;
            println!(
                "production-readiness: PASS catalog_blake3={identity} profiles_accepted={} profiles_rejected={} manifests_accepted={} manifests_rejected={} nickel_rejections={}",
                stats.accepted_profiles,
                stats.rejected_profiles,
                stats.accepted_manifests,
                stats.rejected_manifests,
                stats.nickel_rejections,
            );
        }
    }
    Ok(())
}
