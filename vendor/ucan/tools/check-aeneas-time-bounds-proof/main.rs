//! Validate the exact-source Aeneas `TimeBounds` feasibility evidence lane.

mod artifacts;
mod cli;
mod error;
mod model;

use std::env;
use std::process::ExitCode;

use error::CheckerError;

const FAILURE_EXIT_CODE: u8 = 1;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aeneas-time-bounds-proof: {error}");
            ExitCode::from(FAILURE_EXIT_CODE)
        }
    }
}

fn run() -> Result<(), CheckerError> {
    let arguments = env::args().collect::<Vec<_>>();
    let options = cli::parse(&arguments)?;
    let stats = artifacts::check(&options.root, &options.nickel)?;
    println!(
        "aeneas-time-bounds-proof: PASS outcome=blocked selected_items={} negative_cases={} generated_artifacts={}",
        stats.selected_items, stats.negative_cases, stats.generated_artifacts
    );
    Ok(())
}
