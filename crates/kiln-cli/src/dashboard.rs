//! The `kiln dashboard` operator subcommand.
//!
//! The subcommand drives the shared dashboard shell. It adds no view
//! vocabulary, no projection logic, and no authority: it forwards the operator
//! arguments to the dashboard shell and reports its result.

use kiln_dashboard::shell::{self, Parsed};

/// Run the dashboard subcommand over the remaining arguments.
///
/// The returned code is the process exit code.
// r[impl kiln.observe.dashboard.cli]
// r[impl kiln.observe.dashboard.cli.scenario.match]
pub fn dashboard(arguments: impl Iterator<Item = String>) -> i32 {
    match shell::parse_options(arguments) {
        Ok(Parsed::Options(options)) => shell::run(options, "kiln dashboard"),
        Ok(Parsed::Help) => {
            print!("{}", shell::USAGE);
            0
        }
        Err(message) => {
            eprintln!("kiln dashboard: {message}");
            eprintln!();
            eprint!("{}", shell::USAGE);
            2
        }
    }
}
