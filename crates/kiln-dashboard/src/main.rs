use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use kiln_core::types::{AttemptId, RunId};
use kiln_dashboard::journal::{JournalDocument, MAX_JOURNAL_BYTES, admit_document};
use kiln_dashboard::profile::{MAX_PROFILE_BYTES, ProfileDocument};
use kiln_dashboard::render::{RenderStyle, render_report};
use kiln_dashboard::report::{DashboardSelection, build_report};

const USAGE: &str = "\
usage: kiln-dashboard --journal <path> --profile <path> [--run <id>] [--attempt <id>]
                      [--watch] [--plain]

  --journal <path>   canonical Kiln journal document to observe
  --profile <path>   canonical dashboard profile document
  --run <id>         run to detail; defaults to the first observed run
  --attempt <id>     attempt to detail; defaults to the selected run's active attempt
  --watch            re-read the journal on the profile's watch interval
  --plain            render without ANSI styling
";

struct Options {
    journal_path: PathBuf,
    profile_path: PathBuf,
    selection: DashboardSelection,
    watch: bool,
    plain: bool,
}

enum Parsed {
    Options(Options),
    Help,
}

fn main() -> ExitCode {
    // r[impl kiln.observe.dashboard.shell]
    // r[impl kiln.observe.dashboard.shell.scenario.read_only]
    let parsed = match parse_options(std::env::args().skip(1)) {
        Ok(Parsed::Options(options)) => options,
        Ok(Parsed::Help) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("kiln-dashboard: {message}");
            eprintln!();
            eprint!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let profile = match load_profile(&parsed.profile_path) {
        Ok(profile) => profile,
        Err(message) => {
            eprintln!("kiln-dashboard: {message}");
            return ExitCode::FAILURE;
        }
    };
    let style = if parsed.plain || !std::io::stdout().is_terminal() {
        RenderStyle::Plain
    } else {
        RenderStyle::Ansi
    };
    let mut frame = 0u64;
    loop {
        if parsed.watch && frame > 0 && style == RenderStyle::Ansi {
            print!("\u{1b}[H\u{1b}[2J");
        }
        if let Err(message) = render_frame(&parsed, &profile, style) {
            eprintln!("kiln-dashboard: {message}");
            return ExitCode::FAILURE;
        }
        if !parsed.watch {
            return ExitCode::SUCCESS;
        }
        // r[impl kiln.observe.dashboard.shell.scenario.watch]
        frame += 1;
        std::thread::sleep(Duration::from_millis(profile.profile.watch_interval_ms));
    }
}

fn render_frame(
    options: &Options,
    profile: &ProfileDocument,
    style: RenderStyle,
) -> Result<(), String> {
    let bytes = read_bounded(&options.journal_path, MAX_JOURNAL_BYTES)?;
    let document = JournalDocument::parse(&bytes)
        .map_err(|error| format!("journal document: {}: {}", error.code, error.message))?;
    let journal = admit_document(&document)
        .map_err(|error| format!("journal admission: {}: {}", error.code, error.message))?;
    let report = build_report(&profile.profile, &journal, &options.selection)
        .map_err(|error| format!("dashboard projection: {}: {}", error.code, error.message))?;
    print!("{}", render_report(&report, style));
    Ok(())
}

fn load_profile(path: &PathBuf) -> Result<ProfileDocument, String> {
    let bytes = read_bounded(path, MAX_PROFILE_BYTES)?;
    let document: ProfileDocument = serde_json::from_slice(&bytes).map_err(|error| {
        format!("profile document is not a canonical dashboard profile: {error}")
    })?;
    document
        .admit()
        .map_err(|error| format!("profile admission: {}: {}", error.code, error.message))?;
    Ok(document)
}

fn read_bounded(path: &PathBuf, max_bytes: usize) -> Result<Vec<u8>, String> {
    let metadata =
        std::fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let bound = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if metadata.len() > bound {
        return Err(format!(
            "{}: file exceeds the named byte bound of {max_bytes}",
            path.display()
        ));
    }
    std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn parse_options(arguments: impl Iterator<Item = String>) -> Result<Parsed, String> {
    let mut journal_path: Option<PathBuf> = None;
    let mut profile_path: Option<PathBuf> = None;
    let mut selection = DashboardSelection::default();
    let mut watch = false;
    let mut plain = false;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--journal" => {
                journal_path = Some(PathBuf::from(next_value(&mut arguments, "--journal")?));
            }
            "--profile" => {
                profile_path = Some(PathBuf::from(next_value(&mut arguments, "--profile")?));
            }
            "--run" => {
                let value = next_value(&mut arguments, "--run")?;
                selection.run = Some(
                    RunId::new(value)
                        .map_err(|error| format!("run identity: {}", error.message))?,
                );
            }
            "--attempt" => {
                let value = next_value(&mut arguments, "--attempt")?;
                selection.attempt = Some(
                    AttemptId::new(value)
                        .map_err(|error| format!("attempt identity: {}", error.message))?,
                );
            }
            "--watch" => watch = true,
            "--plain" => plain = true,
            "--help" | "-h" => return Ok(Parsed::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let journal_path = journal_path.ok_or_else(|| "--journal is required".to_string())?;
    let profile_path = profile_path.ok_or_else(|| "--profile is required".to_string())?;
    Ok(Parsed::Options(Options {
        journal_path,
        profile_path,
        selection,
        watch,
        plain,
    }))
}

fn next_value(arguments: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{name} requires a value"))
}
