//! The imperative dashboard shell.
//!
//! This module owns the files, the clock, and the terminal. The pure core in
//! [`crate::profile`], [`crate::journal`], [`crate::report`], and
//! [`crate::render`] decides admission, projection, and rendering.

use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use kiln_core::types::{AttemptId, RunId};
use kiln_observe_core::ReviewWindow;

use crate::catalog::{Catalog, CatalogRenderer, HistoryInput, MAX_CATALOG_BYTES};
use crate::journal::{AdmittedJournal, JournalDocument, MAX_JOURNAL_BYTES, admit_document};
use crate::profile::{DashboardProfile, MAX_PROFILE_BYTES, ProfileDocument};
use crate::render::html::{render_html, render_html_with_refresh};
use crate::render::{RenderStyle, render_report};
use crate::report::{DashboardSelection, build_html_report, build_report};
use crate::store::{admit_store, parse_reference};

/// Named bound on one review window document read.
pub const MAX_REVIEW_WINDOW_BYTES: usize = 1024 * 1024;

/// The usage text of the dashboard shell.
pub const USAGE: &str = "\
usage: kiln-dashboard --journal <path> --profile <path> [--run <id>] [--attempt <id>]
                      [--review <path>] [--watch] [--plain] [--format text|html]
                      [--output <path>]
       kiln-dashboard --store <path> --stream <ns:kind:id> --profile <path>
                      [--run <id>] [--attempt <id>] [--review <path>] [--watch]
                      [--plain] [--format text|html] [--output <path>]
       kiln-dashboard --catalog <path> --profile <path> --format html --output <path>
                      [--watch | --refresh-seconds <seconds>]

  --journal <path>        canonical Kiln journal document to observe
  --catalog <path>        bounded repository/history catalog (HTML output only)
  --store <path>          canonical SQLite event store to observe, read only
  --stream <ns:kind:id>   the event store stream to observe
  --profile <path>        canonical dashboard profile document
  --run <id>              run to detail; defaults to the first observed run
  --attempt <id>          attempt to detail; defaults to the selected run's active attempt
  --review <path>         canonical review window document; adds the review friction
                          and suggestion anchor views (authorship is self-declared
                          and unverified; not with --catalog)
  --watch                 re-read the input on the profile's watch interval
  --refresh-seconds <n>   finite HTML generation with browser refresh (1..86400)
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
    /// Independently admitted repository histories, rendered only as HTML.
    Catalog { path: PathBuf },
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
    pub refresh_seconds: Option<u64>,
    /// Canonical review window document observed beside the journal or store.
    pub review: Option<PathBuf>,
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
    let mut catalog_path: Option<PathBuf> = None;
    let mut stream: Option<String> = None;
    let mut profile_path: Option<PathBuf> = None;
    let mut selection = DashboardSelection::default();
    let mut watch = false;
    let mut plain = false;
    let mut format = ReportFormat::Text;
    let mut output: Option<PathBuf> = None;
    let mut refresh_seconds = None;
    let mut review: Option<PathBuf> = None;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--catalog" => {
                catalog_path = Some(PathBuf::from(next_value(&mut arguments, "--catalog")?));
            }
            "--refresh-seconds" => {
                let value = next_value(&mut arguments, "--refresh-seconds")?;
                let seconds: u64 = value
                    .parse()
                    .map_err(|_| "invalid --refresh-seconds".to_string())?;
                if seconds == 0 || seconds > 86400 {
                    return Err("--refresh-seconds must be 1..86400".to_string());
                }
                refresh_seconds = Some(seconds);
            }
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
            "--review" => {
                review = Some(PathBuf::from(next_value(&mut arguments, "--review")?));
            }
            "--help" | "-h" => return Ok(Parsed::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let profile_path = profile_path.ok_or_else(|| "--profile is required".to_string())?;
    if refresh_seconds.is_some() && (watch || format != ReportFormat::Html) {
        return Err("--refresh-seconds requires HTML and cannot be mixed with --watch".to_string());
    }
    let input = if let Some(path) = catalog_path {
        if journal_path.is_some() || store_path.is_some() || stream.is_some() || review.is_some() {
            return Err(
                "--catalog cannot be mixed with --journal, --store, --stream or --review"
                    .to_string(),
            );
        }
        if format != ReportFormat::Html
            || output.is_none()
            || plain
            || selection.run.is_some()
            || selection.attempt.is_some()
        {
            return Err("--catalog requires --format html --output and does not support --plain, --run or --attempt".to_string());
        }
        Input::Catalog { path }
    } else {
        match (journal_path, store_path) {
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
            (None, None) => return Err("--journal, --store or --catalog is required".to_string()),
        }
    };
    Ok(Parsed::Options(Options {
        input,
        profile_path,
        selection,
        watch,
        plain,
        format,
        output,
        refresh_seconds,
        review,
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
    let refresh = options.refresh_seconds.or_else(|| {
        options
            .watch
            .then(|| profile.profile.watch_interval_ms.div_ceil(1000).max(1))
    });
    if let Input::Catalog { path } = &options.input {
        let output = options
            .output
            .as_ref()
            .ok_or("catalog requires an output path")?;
        protect_output(path, &options.profile_path, output)?;
        let rendered = render_catalog(path, &profile.profile, refresh)?;
        return write_atomic(output, &rendered);
    }
    let journal = admit_input(options)?;
    let rendered = match options.format {
        ReportFormat::Text => build_report(&profile.profile, &journal, &options.selection)
            .map(|report| render_report(&report, style)),
        ReportFormat::Html => {
            // A watched HTML report reloads the atomically replaced document.
            build_html_report(&profile.profile, &journal, &options.selection)
                .map(|report| render_html_with_refresh(&report, refresh))
        }
    }
    .map_err(|error| format!("dashboard projection: {}: {}", error.code, error.message))?;
    match &options.output {
        Some(path) => write_atomic(path, &rendered)?,
        None => print!("{rendered}"),
    }
    Ok(())
}

/// Write a report so a reader never sees a half-written document.
fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    static NEXT_TEMPORARY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT_TEMPORARY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut name = path
        .file_name()
        .ok_or("report output must name a file")?
        .to_os_string();
    name.push(format!(".{}.{}.tmp", std::process::id(), serial));
    let temporary = path.with_file_name(name);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("report temporary create failed: {error}"))?;
    let result = (|| {
        file.write_all(content.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("report write failed: {error}"))?;
        std::fs::rename(&temporary, path).map_err(|error| format!("report replace failed: {error}"))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// Admit the observed input into canonical Kiln state.
///
/// A named review window document is read and parsed after the journal or
/// store is admitted; the report projection admits it against the profile.
pub fn admit_input(options: &Options) -> Result<AdmittedJournal, String> {
    let mut journal = match &options.input {
        Input::Journal { path } => {
            let bytes = read_bounded(path, MAX_JOURNAL_BYTES)?;
            let document = JournalDocument::parse(&bytes)
                .map_err(|error| format!("journal document: {}: {}", error.code, error.message))?;
            admit_document(&document)
                .map_err(|error| format!("journal admission: {}: {}", error.code, error.message))?
        }
        Input::Store { path, stream } => {
            let store_profile = kiln_adapter_sqlite::canonical_profile();
            admit_store(path, &store_profile, stream)?
        }
        Input::Catalog { .. } => {
            return Err("catalog histories must be admitted independently".to_string());
        }
    };
    if let Some(path) = &options.review {
        journal.review = Some(load_review_window(path)?);
    }
    Ok(journal)
}

/// Read and parse one canonical review window document under its byte bound.
pub fn load_review_window(path: &Path) -> Result<ReviewWindow, String> {
    let bytes = read_bounded(path, MAX_REVIEW_WINDOW_BYTES)?;
    parse_review_window(&bytes).map_err(|message| format!("{}: {message}", path.display()))
}

/// Parse one canonical review window document.
///
/// The document is exactly the observation review window: unknown fields,
/// unknown authorship classes, and missing facts are rejected.
pub fn parse_review_window(bytes: &[u8]) -> Result<ReviewWindow, String> {
    serde_json::from_slice(bytes).map_err(|error| {
        format!("review window document is not a canonical review window: {error}")
    })
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
    let file = std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(bound.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "{}: file exceeds the named byte bound of {max_bytes}",
            path.display()
        ));
    }
    Ok(bytes)
}

fn next_value(arguments: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{name} requires a value"))
}
/// Refuse to replace any catalog input, including through symlinked directories.
pub fn protect_output(
    catalog_path: &Path,
    profile_path: &Path,
    output: &Path,
) -> Result<(), String> {
    let catalog = Catalog::parse(&read_bounded(catalog_path, MAX_CATALOG_BYTES)?)?;
    let directory = catalog_path.parent().unwrap_or_else(|| Path::new("."));
    let output = resolved_output(output)?;
    let mut sources = vec![catalog_path.to_path_buf(), profile_path.to_path_buf()];
    for project in &catalog.projects {
        for history in &project.histories {
            let input = directory.join(history.input.path());
            sources.push(input.clone());
            if matches!(history.input, HistoryInput::Store { .. }) {
                for suffix in ["-wal", "-shm", "-journal"] {
                    let mut sidecar = input.as_os_str().to_os_string();
                    sidecar.push(suffix);
                    sources.push(PathBuf::from(sidecar));
                }
            }
        }
    }
    for source in sources {
        if resolved_output(&source)? == output {
            return Err(format!(
                "report output collides with a protected input: {}",
                source.display()
            ));
        }
    }
    Ok(())
}

fn resolved_output(path: &Path) -> Result<PathBuf, String> {
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let parent = std::fs::canonicalize(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
            let name = path.file_name().ok_or("report path must name a file")?;
            Ok(parent.join(name))
        }
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// Admit every catalog history independently, with filesystem effects confined here.
// r[impl kiln.dashboard.repositories.catalog.scenario.navigation]
pub fn render_catalog(
    path: &Path,
    profile: &DashboardProfile,
    refresh: Option<u64>,
) -> Result<String, String> {
    let catalog = Catalog::parse(&read_bounded(path, MAX_CATALOG_BYTES)?)?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let mut inputs = Vec::new();
    let mut total_bytes = 0u64;
    for project in &catalog.projects {
        for history in &project.histories {
            let input_path = directory.join(history.input.path());
            let metadata = std::fs::metadata(&input_path)
                .map_err(|error| format!("{}: {error}", input_path.display()))?;
            if !metadata.is_file() {
                return Err(format!(
                    "{}: history input is not a regular file",
                    input_path.display()
                ));
            }
            if matches!(history.input, HistoryInput::Store { .. }) {
                total_bytes = total_bytes
                    .checked_add(metadata.len())
                    .ok_or("aggregate input size overflow")?;
                for suffix in ["-wal", "-shm", "-journal"] {
                    let mut sidecar = input_path.as_os_str().to_os_string();
                    sidecar.push(suffix);
                    match std::fs::metadata(Path::new(&sidecar)) {
                        Ok(metadata) => {
                            total_bytes = total_bytes
                                .checked_add(metadata.len())
                                .ok_or("aggregate input size overflow")?;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                        Err(error) => return Err(format!("store sidecar metadata: {error}")),
                    }
                }
            }
            if total_bytes > catalog.bounds.max_input_bytes {
                return Err("catalog aggregate input byte bound exceeded".to_string());
            }
            inputs.push(input_path);
        }
    }
    let mut renderer = CatalogRenderer::new(&catalog);
    let mut inputs = inputs.into_iter();
    for project in &catalog.projects {
        for history in &project.histories {
            let path = inputs.next().ok_or("catalog input accounting error")?;
            let journal = match &history.input {
                HistoryInput::Journal { .. } => {
                    let remaining = usize::try_from(catalog.bounds.max_input_bytes - total_bytes)
                        .unwrap_or(usize::MAX)
                        .min(MAX_JOURNAL_BYTES);
                    let bytes = read_bounded(&path, remaining)?;
                    total_bytes += bytes.len() as u64;
                    let document = JournalDocument::parse(&bytes).map_err(|error| {
                        format!("journal document: {}: {}", error.code, error.message)
                    })?;
                    admit_document(&document).map_err(|error| {
                        format!("journal admission: {}: {}", error.code, error.message)
                    })
                }
                HistoryInput::Store { stream, .. } => admit_store(
                    &path,
                    &kiln_adapter_sqlite::canonical_profile(),
                    &parse_reference(stream)?,
                ),
            }
            .map_err(|error| format!("{}/{}: {error}", project.id, history.id))?;
            let report = build_html_report(profile, &journal, &DashboardSelection::default())
                .map_err(|error| {
                    format!(
                        "{}/{}: {}: {}",
                        project.id, history.id, error.code, error.message
                    )
                })?;
            renderer.push(project, history, &render_html(&report), &report.report)?;
        }
    }
    renderer.finish(refresh)
}
