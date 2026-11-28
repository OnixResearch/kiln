//! Workspace and host diagnostics (`kiln doctor`).
//!
//! Argument parsing, `kiln.json` (`kiln.factory-workspace.v1`) schema and path
//! collection, and report evaluation/rendering live in pure functions; file
//! reads, profile loading, and tool lookups stay in the imperative shell.

use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dispatch::resolve_sibling_candidates;

pub const DOCTOR_USAGE: &str = "\
usage: kiln doctor [--config PATH] [--json]

options:
  --config PATH   path to kiln.json (kiln.factory-workspace.v1) to validate
  --json          emit machine-readable JSON report on stdout
  --help, -h      print this text";

pub const WORKSPACE_SCHEMA_V1: &str = "kiln.factory-workspace.v1";
pub const DOCTOR_REPORT_SCHEMA_V1: &str = "kiln.doctor-report.v1";
pub const MAX_CONFIG_BYTES: usize = 1_048_576;
const MAX_REFERENCED_PATHS: usize = 256;

const EXIT_OK: i32 = 0;
const EXIT_FAILURE: i32 = 1;
const EXIT_USAGE: i32 = 2;

const PATH_TOOLS: [&str; 3] = ["git", "jj", "nickel"];
const SIBLING_TOOLS: [&str; 2] = ["kiln-fleet", "kiln-web-service"];
const TOP_LEVEL_OPTIONAL_PATHS: [&str; 3] = ["access", "sources", "fleet"];
const NESTED_PATH_SECTIONS: [&str; 4] = ["ci", "review", "cairn", "observations"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DoctorAction {
    Help,
    Run { config: Option<PathBuf>, json: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DoctorError {
    Usage(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckSeverity {
    Ok,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub severity: CheckSeverity,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub schema: String,
    pub ok: bool,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferencedPath {
    pub field: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedWorkspaceConfig {
    pub profile: PathBuf,
    pub referenced_paths: Vec<ReferencedPath>,
}

/// Parse `kiln doctor` arguments without touching the filesystem.
pub fn parse_doctor_args(
    arguments: impl Iterator<Item = String>,
) -> Result<DoctorAction, DoctorError> {
    let mut args: Vec<String> = arguments.collect();
    if args.len() == 1
        && args
            .first()
            .is_some_and(|arg| matches!(arg.as_str(), "--help" | "-h" | "help"))
    {
        return Ok(DoctorAction::Help);
    }

    let mut config: Option<PathBuf> = None;
    let mut json = false;
    let mut iter = args.drain(..);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--config" => {
                if config.is_some() {
                    return Err(DoctorError::Usage(DOCTOR_USAGE.to_owned()));
                }
                let value = iter
                    .next()
                    .filter(|value| !value.is_empty() && !value.starts_with("--"))
                    .ok_or_else(|| DoctorError::Usage(DOCTOR_USAGE.to_owned()))?;
                config = Some(PathBuf::from(value));
            }
            "--json" => {
                if json {
                    return Err(DoctorError::Usage(DOCTOR_USAGE.to_owned()));
                }
                json = true;
            }
            "--help" | "-h" => return Err(DoctorError::Usage(DOCTOR_USAGE.to_owned())),
            _ => return Err(DoctorError::Usage(DOCTOR_USAGE.to_owned())),
        }
    }

    Ok(DoctorAction::Run { config, json })
}

/// Purely validate a `kiln.json` (`kiln.factory-workspace.v1`) payload and
/// collect every referenced path resolved relative to `config_dir`.
pub fn validate_workspace_config(
    raw: &str,
    config_dir: &Path,
) -> Result<ValidatedWorkspaceConfig, String> {
    let value: Value =
        serde_json::from_str(raw).map_err(|error| format!("invalid kiln.json JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "kiln.json must be a JSON object".to_owned())?;

    let schema = object
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| "kiln.json missing string 'schema' field".to_owned())?;
    if schema != WORKSPACE_SCHEMA_V1 {
        return Err(format!(
            "unsupported kiln.json schema '{schema}' (expected '{WORKSPACE_SCHEMA_V1}')"
        ));
    }

    let profile_raw = object
        .get("profile")
        .and_then(Value::as_str)
        .ok_or_else(|| "kiln.json missing required string 'profile' field".to_owned())?;
    let profile = resolve_config_path(config_dir, profile_raw, "profile")?;

    let mut referenced_paths = vec![ReferencedPath {
        field: "profile".to_owned(),
        path: profile.clone(),
    }];

    for key in TOP_LEVEL_OPTIONAL_PATHS {
        if let Some(entry) = object.get(key)
            && !entry.is_null()
        {
            let text = entry
                .as_str()
                .ok_or_else(|| format!("kiln.json '{key}' must be a path string"))?;
            referenced_paths.push(ReferencedPath {
                field: key.to_owned(),
                path: resolve_config_path(config_dir, text, key)?,
            });
        }
    }

    for section in NESTED_PATH_SECTIONS {
        if let Some(entry) = object.get(section)
            && !entry.is_null()
        {
            let sub = entry
                .as_object()
                .ok_or_else(|| format!("kiln.json '{section}' must be an object"))?;
            for (key, value) in sub {
                if value.is_null() {
                    continue;
                }
                let field_name = format!("{section}.{key}");
                collect_field_paths(&field_name, value, config_dir, &mut referenced_paths)?;
            }
        }
    }

    if let Some(roots) = object.get("repository_roots")
        && !roots.is_null()
    {
        let items = roots.as_array().ok_or_else(|| {
            "kiln.json 'repository_roots' must be an array of path strings".to_owned()
        })?;
        for (index, item) in items.iter().enumerate() {
            let field_name = format!("repository_roots[{index}]");
            let text = item
                .as_str()
                .ok_or_else(|| format!("kiln.json '{field_name}' must be a path string"))?;
            referenced_paths.push(ReferencedPath {
                field: field_name.clone(),
                path: resolve_config_path(config_dir, text, &field_name)?,
            });
        }
    }

    if referenced_paths.len() > MAX_REFERENCED_PATHS {
        return Err(format!(
            "kiln.json references more than {MAX_REFERENCED_PATHS} paths"
        ));
    }

    Ok(ValidatedWorkspaceConfig {
        profile,
        referenced_paths,
    })
}

fn collect_field_paths(
    field_name: &str,
    value: &Value,
    config_dir: &Path,
    out: &mut Vec<ReferencedPath>,
) -> Result<(), String> {
    if let Some(text) = value.as_str() {
        out.push(ReferencedPath {
            field: field_name.to_owned(),
            path: resolve_config_path(config_dir, text, field_name)?,
        });
        return Ok(());
    }
    if let Some(items) = value.as_array() {
        for (index, item) in items.iter().enumerate() {
            let indexed_field = format!("{field_name}[{index}]");
            let text = item
                .as_str()
                .ok_or_else(|| format!("kiln.json '{indexed_field}' must be a path string"))?;
            out.push(ReferencedPath {
                field: indexed_field.clone(),
                path: resolve_config_path(config_dir, text, &indexed_field)?,
            });
        }
        return Ok(());
    }
    Err(format!(
        "kiln.json '{field_name}' must be a path string or array of path strings"
    ))
}

fn resolve_config_path(config_dir: &Path, raw: &str, field: &str) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.trim().is_empty() {
        return Err(format!("kiln.json '{field}' path cannot be empty"));
    }
    if raw.chars().any(char::is_control) {
        return Err(format!(
            "kiln.json '{field}' path must not contain control characters"
        ));
    }
    let path = Path::new(raw);
    Ok(if path.is_absolute() || config_dir.as_os_str().is_empty() {
        path.to_path_buf()
    } else {
        config_dir.join(path)
    })
}

/// Evaluate diagnostic checks into a summary report (`ok` iff no `Error` checks).
pub fn evaluate_report(checks: Vec<DoctorCheck>) -> DoctorReport {
    let ok = !checks
        .iter()
        .any(|check| check.severity == CheckSeverity::Error);
    DoctorReport {
        schema: DOCTOR_REPORT_SCHEMA_V1.to_owned(),
        ok,
        checks,
    }
}

/// Render a [`DoctorReport`] as human-readable text.
pub fn render_report_text(report: &DoctorReport) -> String {
    let mut lines = Vec::with_capacity(report.checks.len() + 2);
    for check in &report.checks {
        let tag = match check.severity {
            CheckSeverity::Ok => "ok",
            CheckSeverity::Warning => "warn",
            CheckSeverity::Error => "error",
        };
        lines.push(format!("[{tag}] {}: {}", check.name, check.detail));
    }
    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines.push(format!(
        "status: {}",
        if report.ok { "ok" } else { "failed" }
    ));
    lines.join("\n")
}

/// Render a [`DoctorReport`] as stable pretty JSON.
pub fn render_report_json(report: &DoctorReport) -> Result<String, String> {
    serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize doctor report: {error}"))
}

/// Imperative shell entry point for `kiln doctor`.
pub fn run(arguments: impl Iterator<Item = String>) -> i32 {
    let action = match parse_doctor_args(arguments) {
        Ok(action) => action,
        Err(DoctorError::Usage(usage)) => {
            eprintln!("{usage}");
            return EXIT_USAGE;
        }
    };
    let DoctorAction::Run { config, json } = action else {
        println!("{DOCTOR_USAGE}");
        return EXIT_OK;
    };

    let mut checks = Vec::new();
    if let Some(config_path) = config.as_deref() {
        checks.extend(check_workspace_config(config_path));
    }
    checks.extend(check_tool_availability());

    let report = evaluate_report(checks);
    let rendered = if json {
        match render_report_json(&report) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("kiln doctor: {error}");
                return EXIT_FAILURE;
            }
        }
    } else {
        render_report_text(&report)
    };
    println!("{rendered}");

    if report.ok { EXIT_OK } else { EXIT_FAILURE }
}

fn check_workspace_config(config_path: &Path) -> Vec<DoctorCheck> {
    let raw = match read_bounded_config(config_path) {
        Ok(text) => text,
        Err(error) => {
            return vec![DoctorCheck {
                name: "config.read".to_owned(),
                severity: CheckSeverity::Error,
                detail: format!("{}: {error}", config_path.display()),
            }];
        }
    };

    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    let validated = match validate_workspace_config(&raw, config_dir) {
        Ok(validated) => validated,
        Err(error) => {
            return vec![DoctorCheck {
                name: "config.schema".to_owned(),
                severity: CheckSeverity::Error,
                detail: error,
            }];
        }
    };

    let mut checks = vec![DoctorCheck {
        name: "config.schema".to_owned(),
        severity: CheckSeverity::Ok,
        detail: format!("{} ({WORKSPACE_SCHEMA_V1})", config_path.display()),
    }];

    for item in &validated.referenced_paths {
        let check = match fs::metadata(&item.path) {
            Ok(_) => DoctorCheck {
                name: format!("config.path.{}", item.field),
                severity: CheckSeverity::Ok,
                detail: item.path.display().to_string(),
            },
            Err(error) => DoctorCheck {
                name: format!("config.path.{}", item.field),
                severity: CheckSeverity::Error,
                detail: format!("missing {} ({error})", item.path.display()),
            },
        };
        checks.push(check);
    }

    let profile_check = match kiln_web_service::profile::ServiceProfile::load(&validated.profile) {
        Ok(profile) => DoctorCheck {
            name: "config.profile.load".to_owned(),
            severity: CheckSeverity::Ok,
            detail: format!(
                "service '{}' ({} projects)",
                profile.service_id,
                profile.projects.len()
            ),
        },
        Err(error) => DoctorCheck {
            name: "config.profile.load".to_owned(),
            severity: CheckSeverity::Error,
            detail: format!("{}: {error}", validated.profile.display()),
        },
    };
    checks.push(profile_check);
    checks
}

fn read_bounded_config(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|error| format!("cannot open config: {error}"))?;
    let mut bytes = Vec::new();
    file.take((MAX_CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read config: {error}"))?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(format!("config exceeds {MAX_CONFIG_BYTES} byte limit"));
    }
    String::from_utf8(bytes).map_err(|_| "config must be valid UTF-8".to_owned())
}

fn check_tool_availability() -> Vec<DoctorCheck> {
    let mut checks = Vec::with_capacity(PATH_TOOLS.len() + SIBLING_TOOLS.len());
    for tool in PATH_TOOLS {
        let check = match find_executable_on_path(tool) {
            Some(path) => DoctorCheck {
                name: format!("tool.{tool}"),
                severity: CheckSeverity::Ok,
                detail: format!("found at {}", path.display()),
            },
            None => DoctorCheck {
                name: format!("tool.{tool}"),
                severity: CheckSeverity::Warning,
                detail: "not found on PATH (optional)".to_owned(),
            },
        };
        checks.push(check);
    }

    let current_exe = std::env::current_exe().ok();
    for tool in SIBLING_TOOLS {
        let check = match find_sibling_or_path(current_exe.as_deref(), tool) {
            Some(path) => DoctorCheck {
                name: format!("tool.{tool}"),
                severity: CheckSeverity::Ok,
                detail: format!("found at {}", path.display()),
            },
            None => DoctorCheck {
                name: format!("tool.{tool}"),
                severity: CheckSeverity::Warning,
                detail: "not found beside kiln or on PATH (optional)".to_owned(),
            },
        };
        checks.push(check);
    }
    checks
}

fn find_sibling_or_path(current_exe: Option<&Path>, binary_name: &str) -> Option<PathBuf> {
    for candidate in resolve_sibling_candidates(current_exe, binary_name) {
        if candidate.components().count() > 1 {
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        } else if let Some(found) = find_executable_on_path(binary_name) {
            return Some(found);
        }
    }
    None
}

fn find_executable_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_doctor_args_supports_valid_combinations() {
        assert_eq!(
            parse_doctor_args(std::iter::empty()),
            Ok(DoctorAction::Run {
                config: None,
                json: false,
            })
        );
        assert_eq!(
            parse_doctor_args(["--help"].into_iter().map(str::to_owned)),
            Ok(DoctorAction::Help)
        );
        assert_eq!(
            parse_doctor_args(["-h"].into_iter().map(str::to_owned)),
            Ok(DoctorAction::Help)
        );
        assert_eq!(
            parse_doctor_args(
                ["--config", "/tmp/ws/kiln.json", "--json"]
                    .into_iter()
                    .map(str::to_owned)
            ),
            Ok(DoctorAction::Run {
                config: Some(PathBuf::from("/tmp/ws/kiln.json")),
                json: true,
            })
        );
    }

    #[test]
    fn parse_doctor_args_rejects_unknown_duplicate_and_missing_values() {
        for bad in [
            vec!["--unknown"],
            vec!["--config"],
            vec!["--config", "--json"],
            vec!["--config", "a.json", "--config", "b.json"],
            vec!["--json", "--json"],
            vec!["--json", "--help"],
            vec!["positional"],
        ] {
            assert!(
                matches!(
                    parse_doctor_args(bad.into_iter().map(str::to_owned)),
                    Err(DoctorError::Usage(_))
                ),
                "expected usage error"
            );
        }
    }

    #[test]
    fn validate_workspace_config_collects_and_resolves_all_paths() {
        let raw = r#"{
            "schema": "kiln.factory-workspace.v1",
            "profile": "service.json",
            "access": "access.json",
            "sources": "sources.json",
            "fleet": "/etc/kiln/fleet.json",
            "ci": {
                "automation": "ci/automation.json",
                "runbooks_dir": "ci/runbooks"
            },
            "review": {
                "reviews": "reviews.json"
            },
            "cairn": {
                "authoring": "cairn.json"
            },
            "observations": {
                "inventory": "obs/inventory.json"
            },
            "repository_roots": ["repos", "/srv/repos"]
        }"#;
        let validated = validate_workspace_config(raw, Path::new("/tmp/factory"));
        let Ok(validated) = validated else {
            unreachable!("expected valid config, got {validated:?}");
        };
        assert_eq!(
            validated.profile,
            PathBuf::from("/tmp/factory/service.json")
        );
        assert!(validated.referenced_paths.contains(&ReferencedPath {
            field: "profile".to_owned(),
            path: PathBuf::from("/tmp/factory/service.json"),
        }));
        assert!(validated.referenced_paths.contains(&ReferencedPath {
            field: "fleet".to_owned(),
            path: PathBuf::from("/etc/kiln/fleet.json"),
        }));
        assert!(validated.referenced_paths.contains(&ReferencedPath {
            field: "ci.automation".to_owned(),
            path: PathBuf::from("/tmp/factory/ci/automation.json"),
        }));
        assert!(validated.referenced_paths.contains(&ReferencedPath {
            field: "review.reviews".to_owned(),
            path: PathBuf::from("/tmp/factory/reviews.json"),
        }));
        assert!(validated.referenced_paths.contains(&ReferencedPath {
            field: "cairn.authoring".to_owned(),
            path: PathBuf::from("/tmp/factory/cairn.json"),
        }));
        assert!(validated.referenced_paths.contains(&ReferencedPath {
            field: "observations.inventory".to_owned(),
            path: PathBuf::from("/tmp/factory/obs/inventory.json"),
        }));
        assert!(validated.referenced_paths.contains(&ReferencedPath {
            field: "repository_roots[0]".to_owned(),
            path: PathBuf::from("/tmp/factory/repos"),
        }));
        assert!(validated.referenced_paths.contains(&ReferencedPath {
            field: "repository_roots[1]".to_owned(),
            path: PathBuf::from("/srv/repos"),
        }));
    }

    #[test]
    fn validate_workspace_config_rejects_bad_schema_missing_profile_and_invalid_paths() {
        let wrong_schema = r#"{"schema": "kiln.other.v1", "profile": "service.json"}"#;
        assert!(validate_workspace_config(wrong_schema, Path::new("/tmp")).is_err());

        let missing_profile = r#"{"schema": "kiln.factory-workspace.v1"}"#;
        assert!(validate_workspace_config(missing_profile, Path::new("/tmp")).is_err());

        let empty_profile = r#"{"schema": "kiln.factory-workspace.v1", "profile": "   "}"#;
        assert!(validate_workspace_config(empty_profile, Path::new("/tmp")).is_err());

        let bad_roots = r#"{
            "schema": "kiln.factory-workspace.v1",
            "profile": "service.json",
            "repository_roots": "not-an-array"
        }"#;
        assert!(validate_workspace_config(bad_roots, Path::new("/tmp")).is_err());

        let not_json = "{broken";
        assert!(validate_workspace_config(not_json, Path::new("/tmp")).is_err());
    }

    #[test]
    fn evaluate_report_and_renderers_distinguish_ok_warning_and_error() {
        let warn_only = evaluate_report(vec![
            DoctorCheck {
                name: "tool.git".to_owned(),
                severity: CheckSeverity::Ok,
                detail: "/usr/bin/git".to_owned(),
            },
            DoctorCheck {
                name: "tool.nickel".to_owned(),
                severity: CheckSeverity::Warning,
                detail: "not found".to_owned(),
            },
        ]);
        assert!(warn_only.ok);
        let text = render_report_text(&warn_only);
        assert!(text.contains("[ok] tool.git: /usr/bin/git"), "{text}");
        assert!(text.contains("[warn] tool.nickel: not found"), "{text}");
        assert!(text.contains("status: ok"), "{text}");

        let with_error = evaluate_report(vec![DoctorCheck {
            name: "config.path.profile".to_owned(),
            severity: CheckSeverity::Error,
            detail: "missing".to_owned(),
        }]);
        assert!(!with_error.ok);
        let err_text = render_report_text(&with_error);
        assert!(err_text.contains("[error] config.path.profile: missing"));
        assert!(err_text.contains("status: failed"));

        let Ok(json_text) = render_report_json(&with_error) else {
            unreachable!("json render should succeed");
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&json_text) else {
            unreachable!("rendered json should parse");
        };
        assert_eq!(parsed.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            parsed.get("schema").and_then(Value::as_str),
            Some(DOCTOR_REPORT_SCHEMA_V1)
        );
    }
}
