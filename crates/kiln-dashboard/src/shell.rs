//! The imperative dashboard shell.
//!
//! This module owns the files, the clock, and the terminal. The pure core in
//! [`crate::profile`], [`crate::journal`], [`crate::report`], and
//! [`crate::render`] decides admission, projection, and rendering.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

use kiln_core::types::{AttemptId, RunId};

use crate::journal::{AdmittedJournal, JournalDocument, MAX_JOURNAL_BYTES, admit_document};
use crate::profile::{MAX_PROFILE_BYTES, ProfileDocument};
use crate::render::html::render_html_with_refresh;
use crate::render::{RenderStyle, render_report};
use crate::report::{DashboardSelection, build_report};
use crate::store::{admit_store, parse_reference};

/// The usage text of the dashboard shell.
pub const USAGE: &str = "\
usage: kiln-dashboard --journal <path> --profile <path> [--run <id>] [--attempt <id>]
                      [--watch] [--plain] [--format text|html] [--output <path>]
       kiln-dashboard --store <path> --stream <ns:kind:id> --profile <path>
                      [--run <id>] [--attempt <id>] [--watch] [--plain]
                      [--format text|html] [--output <path>]

  --journal <path>        canonical Kiln journal document to observe
  --store <path>          canonical SQLite event store to observe, read only
  --stream <ns:kind:id>   the event store stream to observe
  --profile <path>        canonical dashboard profile document
  --run <id>              run to detail; defaults to the first observed run
  --attempt <id>          attempt to detail; defaults to the selected run's active attempt
  --watch                 re-read the input on the profile's watch interval
  --plain                 render without ANSI styling
  --format <text|html>    report format; text is the default
  --output <path>         write the report to a file instead of standard output
";

/// The report format the dashboard shell renders.
// r[impl kiln.observe.dashboard.html.format]
// r[impl kiln.observe.dashboard.html.format.scenario.selection]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportFormat {
    /// The deterministic terminal report.
    Text,
    /// A self-contained HTML document.
    Html,
}

impl ReportFormat {
    /// Parse the report format named on the command line.
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "html" => Ok(Self::Html),
            other => Err(format!("unsupported report format: {other}")),
        }
    }
}

/// The observed input of the dashboard shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Input {
    /// A canonical Kiln journal document.
    Journal { path: PathBuf },
    /// A canonical SQLite event store, opened read only.
    Store {
        path: PathBuf,
        stream: Box<kiln_core::types::ExternalRef>,
    },
}

/// The parsed dashboard shell options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    pub input: Input,
    pub profile_path: PathBuf,
    pub selection: DashboardSelection,
    pub watch: bool,
    pub plain: bool,
    pub format: ReportFormat,
    pub output: Option<PathBuf>,
}

/// The outcome of parsing dashboard shell arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Parsed {
    /// The shell has options to run.
    Options(Options),
    /// The operator asked for the usage text.
    Help,
}

/// Parse the dashboard shell arguments.
pub fn parse_options(arguments: impl Iterator<Item = String>) -> Result<Parsed, String> {
    let mut journal_path: Option<PathBuf> = None;
    let mut store_path: Option<PathBuf> = None;
    let mut stream: Option<String> = None;
    let mut profile_path: Option<PathBuf> = None;
    let mut selection = DashboardSelection::default();
    let mut watch = false;
    let mut plain = false;
    let mut format = ReportFormat::Text;
    let mut output: Option<PathBuf> = None;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--journal" => {
                journal_path = Some(PathBuf::from(next_value(&mut arguments, "--journal")?));
            }
            "--store" => {
                store_path = Some(PathBuf::from(next_value(&mut arguments, "--store")?));
            }
            "--stream" => {
                stream = Some(next_value(&mut arguments, "--stream")?);
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
            "--format" => {
                let value = next_value(&mut arguments, "--format")?;
                format = ReportFormat::parse(&value)?;
            }
            "--output" => {
                output = Some(PathBuf::from(next_value(&mut arguments, "--output")?));
            }
            "--help" | "-h" => return Ok(Parsed::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let profile_path = profile_path.ok_or_else(|| "--profile is required".to_string())?;
    let input = match (journal_path, store_path) {
        (Some(_), Some(_)) => {
            return Err("--journal and --store are mutually exclusive".to_string());
        }
        (Some(path), None) => Input::Journal { path },
        (None, Some(path)) => {
            let stream = stream.ok_or_else(|| "--stream is required".to_string())?;
            Input::Store {
                path,
                stream: Box::new(parse_reference(&stream)?),
            }
        }
        (None, None) => return Err("--journal or --store is required".to_string()),
    };
    Ok(Parsed::Options(Options {
        input,
        profile_path,
        selection,
        watch,
        plain,
        format,
        output,
    }))
}

/// Run the dashboard shell with the process arguments.
///
/// The returned code is the process exit code.
pub fn run_from_env() -> i32 {
    let parsed = match parse_options(std::env::args().skip(1)) {
        Ok(Parsed::Options(options)) => options,
        Ok(Parsed::Help) => {
            print!("{USAGE}");
            return 0;
        }
        Err(message) => {
            eprintln!("kiln-dashboard: {message}");
            eprintln!();
            eprint!("{USAGE}");
            return 1;
        }
    };
    run(parsed, "kiln-dashboard")
}

/// Run the dashboard shell over parsed options.
///
/// The label names the invoking surface in operator-facing diagnostics. The
/// returned code is the process exit code.
pub fn run(options: Options, label: &str) -> i32 {
    let profile = match load_profile(&options.profile_path) {
        Ok(profile) => profile,
        Err(message) => {
            eprintln!("{label}: {message}");
            return 1;
        }
    };
    let style = if options.plain || !std::io::stdout().is_terminal() {
        RenderStyle::Plain
    } else {
        RenderStyle::Ansi
    };
    let mut frame = 0u64;
    loop {
        if options.watch && frame > 0 && style == RenderStyle::Ansi {
            print!("\u{1b}[H\u{1b}[2J");
        }
        if let Err(message) = render_frame(&options, &profile, style) {
            eprintln!("{label}: {message}");
            return 1;
        }
        if !options.watch {
            return 0;
        }
        frame += 1;
        std::thread::sleep(Duration::from_millis(profile.profile.watch_interval_ms));
    }
}

/// Render one frame of the dashboard report.
pub fn render_frame(
    options: &Options,
    profile: &ProfileDocument,
    style: RenderStyle,
) -> Result<(), String> {
    let journal = admit_input(options)?;
    let report = build_report(&profile.profile, &journal, &options.selection)
        .map_err(|error| format!("dashboard projection: {}: {}", error.code, error.message))?;
    let rendered = match options.format {
        ReportFormat::Text => render_report(&report, style),
        ReportFormat::Html => {
            // A watched HTML report reloads itself so a browser tab follows
            // the journal; an unwatched one stays a snapshot.
            let refresh = options
                .watch
                .then(|| profile.profile.watch_interval_ms.div_ceil(1000).max(1));
            render_html_with_refresh(&report, refresh)
        }
    };
    match &options.output {
        Some(path) => write_atomic(path, &rendered)?,
        None => print!("{rendered}"),
    }
    Ok(())
}

/// Write a report so a reader never sees a half-written document.
fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let temporary = path.with_extension("html.tmp");
    std::fs::write(&temporary, content).map_err(|error| format!("report write failed: {error}"))?;
    std::fs::rename(&temporary, path).map_err(|error| format!("report replace failed: {error}"))
}

/// Admit the observed input into canonical Kiln state.
pub fn admit_input(options: &Options) -> Result<AdmittedJournal, String> {
    match &options.input {
        Input::Journal { path } => {
            let bytes = read_bounded(path, MAX_JOURNAL_BYTES)?;
            let document = JournalDocument::parse(&bytes)
                .map_err(|error| format!("journal document: {}: {}", error.code, error.message))?;
            admit_document(&document)
                .map_err(|error| format!("journal admission: {}: {}", error.code, error.message))
        }
        Input::Store { path, stream } => {
            let store_profile = kiln_adapter_sqlite::canonical_profile();
            admit_store(path, &store_profile, stream)
        }
    }
}

/// Load and admit the dashboard profile document.
pub fn load_profile(path: &Path) -> Result<ProfileDocument, String> {
    let bytes = read_bounded(path, MAX_PROFILE_BYTES)?;
    let document: ProfileDocument = serde_json::from_slice(&bytes).map_err(|error| {
        format!("profile document is not a canonical dashboard profile: {error}")
    })?;
    document
        .admit()
        .map_err(|error| format!("profile admission: {}: {}", error.code, error.message))?;
    Ok(document)
}

/// Read one file under its named byte bound.
pub fn read_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
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

fn next_value(arguments: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{name} requires a value"))
}
