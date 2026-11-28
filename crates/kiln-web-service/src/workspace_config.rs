//! Workspace manifest (`kiln.factory-workspace.v1`) and shared `serve`/`check` CLI loader.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::server;

/// Versioned schema identifier for the single workspace manifest.
pub const WORKSPACE_CONFIG_SCHEMA: &str = "kiln.factory-workspace.v1";

/// Maximum admitted byte size for a single workspace manifest JSON file.
pub const MAX_WORKSPACE_CONFIG_BYTES: usize = 65_536;

/// Maximum number of local repository roots admitted per workspace manifest or CLI invocation.
pub const MAX_REPOSITORY_ROOTS: usize = 64;

/// Maximum admitted byte length for a single configured path value.
pub const MAX_PATH_BYTES: usize = 4_096;

pub const SERVICE_USAGE: &str = "\
usage: kiln-web-service <serve|check> [arguments]

minimal local example (after `kiln init --repo PATH --out DIR`):
  kiln-web-service check --config DIR/kiln.json
  kiln-web-service serve --config DIR/kiln.json

options:
  --config <path>                    Single kiln.factory-workspace.v1 JSON manifest
  --profile <path>                   Explicit service profile JSON (overrides --config profile)
  --access <path>                    Access policy JSON
  --sources <path>                   Repository sources JSON
  --fleet <path>                     Fleet coordinator profile JSON
  --automation <path>                Automation triggers profile JSON
  --workers <path>                   Worker status profile JSON
  --cache <path>                     Cache profile JSON
  --runbooks <path>                  Runbooks registry JSON
  --runbooks-dir <path>              Runbooks bundle directory
  --execution-artifacts <path>       Execution artifacts profile JSON
  --reviews <path>                   Patchgate receipts profile JSON
  --code-review <path>               Code review workspace profile JSON
  --work <path>                      Work & issues profile JSON
  --cobs <path>                      Read-only COB profile JSON
  --artifact-previews <path>         Artifact preview catalog JSON
  --preview-observations <path>      Preview observations directory
  --cairn-authoring <path>           Native Cairn authoring profile JSON
  --lattice-authoring <path>         Lattice workflow sources JSON
  --lattice-drafts <path>            Lattice workflow drafts profile JSON
  --inventory <path>                 Inventory snapshot input JSON
  --implementation-work <path>       Implementation work database path
  --campaign-controls <path>         Campaign controls profile JSON
  --dispatch-recovery <path>         Dispatch recovery profile JSON
  --native-runtime-evidence <path>   Native runtime evidence profile JSON
  --delivery-observations <path>     Delivery observations JSON
  --repository-root <dir>            Local checkout root directory (repeatable)";

const OPTION_FLAGS: [&str; 24] = [
    "--access",
    "--automation",
    "--cobs",
    "--sources",
    "--reviews",
    "--cache",
    "--workers",
    "--inventory",
    "--implementation-work",
    "--campaign-controls",
    "--dispatch-recovery",
    "--native-runtime-evidence",
    "--execution-artifacts",
    "--delivery-observations",
    "--artifact-previews",
    "--preview-observations",
    "--work",
    "--code-review",
    "--fleet",
    "--runbooks",
    "--runbooks-dir",
    "--lattice-authoring",
    "--lattice-drafts",
    "--cairn-authoring",
];

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WorkspaceConfigFile {
    schema: Option<String>,
    profile: Option<String>,
    access: Option<String>,
    sources: Option<String>,
    fleet: Option<String>,
    ci: Option<CiSection>,
    review: Option<ReviewSection>,
    cairn: Option<CairnSection>,
    observations: Option<ObservationsSection>,
    repository_roots: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct CiSection {
    automation: Option<String>,
    workers: Option<String>,
    cache: Option<String>,
    runbooks: Option<String>,
    runbooks_dir: Option<String>,
    execution_artifacts: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ReviewSection {
    reviews: Option<String>,
    code_review: Option<String>,
    work: Option<String>,
    cobs: Option<String>,
    artifact_previews: Option<String>,
    preview_observations: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct CairnSection {
    authoring: Option<String>,
    lattice_authoring: Option<String>,
    lattice_drafts: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ObservationsSection {
    inventory: Option<String>,
    implementation_work: Option<String>,
    campaign_controls: Option<String>,
    dispatch_recovery: Option<String>,
    native_runtime_evidence: Option<String>,
    delivery_observations: Option<String>,
}

/// Admitted CLI configuration ready for `server::check_with_options` or `server::run_with_options`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceCliConfig {
    pub profile: PathBuf,
    pub options: server::Options,
}

/// Pure parsing or configuration error produced by `parse_service_cli_args`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceCliError {
    Help,
    Usage(String),
    Config(String),
}

#[derive(Debug, Default)]
struct ParsedCliFlags {
    config: Option<PathBuf>,
    profile: Option<PathBuf>,
    options: server::Options,
}

fn validate_path_str(raw: &str, label: &str) -> Result<(), String> {
    if raw.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if raw.len() > MAX_PATH_BYTES {
        return Err(format!("{label} exceeds {MAX_PATH_BYTES} bytes"));
    }
    if raw.chars().any(char::is_control) {
        return Err(format!("{label} must not contain control characters"));
    }
    Ok(())
}

fn resolve_path(raw: &str, field: &str, base_dir: &Path) -> Result<PathBuf, String> {
    validate_path_str(raw, &format!("workspace config field '{field}'"))?;
    let path = Path::new(raw);
    if path.is_absolute() || base_dir.as_os_str().is_empty() {
        Ok(path.to_path_buf())
    } else {
        Ok(base_dir.join(path))
    }
}

fn resolve_optional_path(
    raw: Option<&str>,
    field: &str,
    base_dir: &Path,
) -> Result<Option<PathBuf>, String> {
    raw.map(|value| resolve_path(value, field, base_dir))
        .transpose()
}

fn option_slot<'a>(
    options: &'a mut server::Options,
    flag: &str,
) -> Option<&'a mut Option<PathBuf>> {
    match flag {
        "--access" => Some(&mut options.access),
        "--automation" => Some(&mut options.automation),
        "--cobs" => Some(&mut options.cobs),
        "--sources" => Some(&mut options.sources),
        "--reviews" => Some(&mut options.reviews),
        "--cache" => Some(&mut options.cache),
        "--workers" => Some(&mut options.workers),
        "--inventory" => Some(&mut options.inventory),
        "--implementation-work" => Some(&mut options.implementation_work),
        "--campaign-controls" => Some(&mut options.campaign_controls),
        "--dispatch-recovery" => Some(&mut options.dispatch_recovery),
        "--native-runtime-evidence" => Some(&mut options.native_runtime_evidence),
        "--execution-artifacts" => Some(&mut options.execution_artifacts),
        "--delivery-observations" => Some(&mut options.delivery_observations),
        "--artifact-previews" => Some(&mut options.artifact_previews),
        "--preview-observations" => Some(&mut options.preview_observations),
        "--work" => Some(&mut options.work),
        "--code-review" => Some(&mut options.code_review),
        "--fleet" => Some(&mut options.fleet),
        "--runbooks" => Some(&mut options.runbooks),
        "--runbooks-dir" => Some(&mut options.runbooks_dir),
        "--lattice-authoring" => Some(&mut options.lattice_authoring),
        "--lattice-drafts" => Some(&mut options.lattice_drafts),
        "--cairn-authoring" => Some(&mut options.cairn_authoring),
        _ => None,
    }
}

/// Pure core parser for a `kiln.factory-workspace.v1` JSON manifest.
pub fn parse_workspace_config(
    raw: &str,
    base_dir: &Path,
) -> Result<(PathBuf, server::Options), String> {
    if raw.len() > MAX_WORKSPACE_CONFIG_BYTES {
        return Err(format!(
            "workspace config exceeds {MAX_WORKSPACE_CONFIG_BYTES} bytes"
        ));
    }
    let parsed: WorkspaceConfigFile =
        serde_json::from_str(raw).map_err(|err| format!("invalid workspace config JSON: {err}"))?;
    if parsed.schema.as_deref() != Some(WORKSPACE_CONFIG_SCHEMA) {
        return Err(format!(
            "workspace config schema must be '{WORKSPACE_CONFIG_SCHEMA}'"
        ));
    }
    let profile_raw = parsed
        .profile
        .as_deref()
        .ok_or_else(|| "workspace config requires a 'profile' path".to_owned())?;
    let profile = resolve_path(profile_raw, "profile", base_dir)?;

    let raw_roots = parsed.repository_roots.unwrap_or_default();
    if raw_roots.len() > MAX_REPOSITORY_ROOTS {
        return Err(format!(
            "workspace config repository_roots exceeds {MAX_REPOSITORY_ROOTS} entries"
        ));
    }
    let mut repository_roots = Vec::with_capacity(raw_roots.len());
    for root in &raw_roots {
        repository_roots.push(resolve_path(root, "repository_roots", base_dir)?);
    }

    let ci = parsed.ci.unwrap_or_default();
    let review = parsed.review.unwrap_or_default();
    let cairn = parsed.cairn.unwrap_or_default();
    let observations = parsed.observations.unwrap_or_default();

    let options = server::Options {
        access: resolve_optional_path(parsed.access.as_deref(), "access", base_dir)?,
        sources: resolve_optional_path(parsed.sources.as_deref(), "sources", base_dir)?,
        fleet: resolve_optional_path(parsed.fleet.as_deref(), "fleet", base_dir)?,
        automation: resolve_optional_path(ci.automation.as_deref(), "ci.automation", base_dir)?,
        workers: resolve_optional_path(ci.workers.as_deref(), "ci.workers", base_dir)?,
        cache: resolve_optional_path(ci.cache.as_deref(), "ci.cache", base_dir)?,
        runbooks: resolve_optional_path(ci.runbooks.as_deref(), "ci.runbooks", base_dir)?,
        runbooks_dir: resolve_optional_path(
            ci.runbooks_dir.as_deref(),
            "ci.runbooks_dir",
            base_dir,
        )?,
        execution_artifacts: resolve_optional_path(
            ci.execution_artifacts.as_deref(),
            "ci.execution_artifacts",
            base_dir,
        )?,
        reviews: resolve_optional_path(review.reviews.as_deref(), "review.reviews", base_dir)?,
        code_review: resolve_optional_path(
            review.code_review.as_deref(),
            "review.code_review",
            base_dir,
        )?,
        work: resolve_optional_path(review.work.as_deref(), "review.work", base_dir)?,
        cobs: resolve_optional_path(review.cobs.as_deref(), "review.cobs", base_dir)?,
        artifact_previews: resolve_optional_path(
            review.artifact_previews.as_deref(),
            "review.artifact_previews",
            base_dir,
        )?,
        preview_observations: resolve_optional_path(
            review.preview_observations.as_deref(),
            "review.preview_observations",
            base_dir,
        )?,
        cairn_authoring: resolve_optional_path(
            cairn.authoring.as_deref(),
            "cairn.authoring",
            base_dir,
        )?,
        lattice_authoring: resolve_optional_path(
            cairn.lattice_authoring.as_deref(),
            "cairn.lattice_authoring",
            base_dir,
        )?,
        lattice_drafts: resolve_optional_path(
            cairn.lattice_drafts.as_deref(),
            "cairn.lattice_drafts",
            base_dir,
        )?,
        inventory: resolve_optional_path(
            observations.inventory.as_deref(),
            "observations.inventory",
            base_dir,
        )?,
        implementation_work: resolve_optional_path(
            observations.implementation_work.as_deref(),
            "observations.implementation_work",
            base_dir,
        )?,
        campaign_controls: resolve_optional_path(
            observations.campaign_controls.as_deref(),
            "observations.campaign_controls",
            base_dir,
        )?,
        dispatch_recovery: resolve_optional_path(
            observations.dispatch_recovery.as_deref(),
            "observations.dispatch_recovery",
            base_dir,
        )?,
        native_runtime_evidence: resolve_optional_path(
            observations.native_runtime_evidence.as_deref(),
            "observations.native_runtime_evidence",
            base_dir,
        )?,
        delivery_observations: resolve_optional_path(
            observations.delivery_observations.as_deref(),
            "observations.delivery_observations",
            base_dir,
        )?,
        repository_roots,
    };
    Ok((profile, options))
}

/// Thin imperative shell reading and validating a workspace config JSON file from disk.
pub fn load_workspace_config(path: &Path) -> Result<(PathBuf, server::Options), String> {
    use std::io::Read as _;

    let file = std::fs::File::open(path).map_err(|err| {
        format!(
            "workspace config {} cannot be opened: {err}",
            path.display()
        )
    })?;
    if let Ok(metadata) = file.metadata()
        && metadata.len() > MAX_WORKSPACE_CONFIG_BYTES as u64
    {
        return Err(format!(
            "workspace config exceeds {MAX_WORKSPACE_CONFIG_BYTES} bytes"
        ));
    }
    let mut bytes = Vec::new();
    file.take((MAX_WORKSPACE_CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("workspace config {} cannot be read: {err}", path.display()))?;
    if bytes.len() > MAX_WORKSPACE_CONFIG_BYTES {
        return Err(format!(
            "workspace config exceeds {MAX_WORKSPACE_CONFIG_BYTES} bytes"
        ));
    }
    let raw = std::str::from_utf8(&bytes)
        .map_err(|_| "workspace config must be valid UTF-8".to_owned())?;
    let base_dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parse_workspace_config(raw, base_dir)
}

fn next_flag_value(
    flag: &str,
    arguments: &mut impl Iterator<Item = String>,
) -> Result<PathBuf, ServiceCliError> {
    let Some(value) = arguments.next().filter(|value| !value.starts_with("--")) else {
        return Err(ServiceCliError::Usage(format!("missing value for {flag}")));
    };
    validate_path_str(&value, flag).map_err(ServiceCliError::Usage)?;
    Ok(PathBuf::from(value))
}

fn parse_service_cli_flags(
    mut arguments: impl Iterator<Item = String>,
) -> Result<ParsedCliFlags, ServiceCliError> {
    let mut parsed = ParsedCliFlags::default();
    while let Some(flag) = arguments.next() {
        if flag == "--help" || flag == "-h" {
            return Err(ServiceCliError::Help);
        }
        if flag == "--repository-root" {
            let value = next_flag_value(&flag, &mut arguments)?;
            if parsed.options.repository_roots.len() >= MAX_REPOSITORY_ROOTS {
                return Err(ServiceCliError::Usage(format!(
                    "--repository-root exceeds {MAX_REPOSITORY_ROOTS} entries"
                )));
            }
            parsed.options.repository_roots.push(value);
            continue;
        }
        let slot = match flag.as_str() {
            "--config" => &mut parsed.config,
            "--profile" => &mut parsed.profile,
            _ => option_slot(&mut parsed.options, &flag)
                .ok_or_else(|| ServiceCliError::Usage(format!("unknown flag {flag}")))?,
        };
        if slot.is_some() {
            return Err(ServiceCliError::Usage(format!("duplicate {flag} flag")));
        }
        *slot = Some(next_flag_value(&flag, &mut arguments)?);
    }
    if parsed.config.is_none() && parsed.profile.is_none() {
        return Err(ServiceCliError::Usage(
            "expected --config <path> or --profile <path>".to_owned(),
        ));
    }
    Ok(parsed)
}

fn merge_cli_overrides(
    base_profile: Option<PathBuf>,
    mut base_options: server::Options,
    mut cli: ParsedCliFlags,
) -> Result<ServiceCliConfig, ServiceCliError> {
    let profile = cli.profile.or(base_profile).ok_or_else(|| {
        ServiceCliError::Usage("expected --config <path> or --profile <path>".to_owned())
    })?;
    for flag in OPTION_FLAGS {
        if let Some(value) = option_slot(&mut cli.options, flag).and_then(Option::take)
            && let Some(slot) = option_slot(&mut base_options, flag)
        {
            *slot = Some(value);
        }
    }
    if base_options.repository_roots.len() + cli.options.repository_roots.len()
        > MAX_REPOSITORY_ROOTS
    {
        return Err(ServiceCliError::Config(format!(
            "combined repository_roots exceeds {MAX_REPOSITORY_ROOTS} entries"
        )));
    }
    base_options
        .repository_roots
        .extend(cli.options.repository_roots);
    Ok(ServiceCliConfig {
        profile,
        options: base_options,
    })
}

/// Parse `serve`/`check` CLI arguments, loading `--config` if present and merging explicit overrides.
pub fn parse_service_cli_args(
    arguments: impl Iterator<Item = String>,
) -> Result<ServiceCliConfig, ServiceCliError> {
    let flags = parse_service_cli_flags(arguments)?;
    let (base_profile, base_options) = match flags.config.as_deref() {
        Some(path) => {
            let (profile, options) =
                load_workspace_config(path).map_err(ServiceCliError::Config)?;
            (Some(profile), options)
        }
        None => (None, server::Options::default()),
    };
    merge_cli_overrides(base_profile, base_options, flags)
}

/// Shared CLI entrypoint for `kiln-web-service` and `kiln-cli` (`check` / `serve`).
pub fn run_service_cli(command: &str, arguments: impl Iterator<Item = String>) -> i32 {
    if !matches!(command, "check" | "serve") {
        eprintln!("{SERVICE_USAGE}");
        return 2;
    }
    let config = match parse_service_cli_args(arguments) {
        Ok(config) => config,
        Err(ServiceCliError::Help) => {
            println!("{SERVICE_USAGE}");
            return 0;
        }
        Err(ServiceCliError::Usage(message)) => {
            eprintln!("kiln-web-service: {message}\n{SERVICE_USAGE}");
            return 2;
        }
        Err(ServiceCliError::Config(message)) => {
            eprintln!("kiln-web-service: {message}");
            return 1;
        }
    };
    let result = if command == "check" {
        server::check_with_options(&config.profile, &config.options)
    } else {
        server::run_with_options(&config.profile, &config.options)
    };
    match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("kiln-web-service: {message}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn args(items: &[&str]) -> std::vec::IntoIter<String> {
        items
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn minimal_config_parses_and_resolves_relative_to_base_dir() -> TestResult {
        let raw = r#"{
            "schema": "kiln.factory-workspace.v1",
            "profile": "service.json"
        }"#;
        let (profile, options) = parse_workspace_config(raw, Path::new("/etc/kiln"))?;
        assert_eq!(profile, PathBuf::from("/etc/kiln/service.json"));
        assert_eq!(options, server::Options::default());
        Ok(())
    }

    #[test]
    fn full_config_resolves_relative_and_preserves_absolute_paths() -> TestResult {
        let raw = r#"{
            "schema": "kiln.factory-workspace.v1",
            "profile": "service.json",
            "access": "/abs/access.json",
            "sources": "sources.json",
            "fleet": "fleet.json",
            "ci": {
                "automation": "ci/automation.json",
                "workers": "/abs/workers.json",
                "cache": "ci/cache.json",
                "runbooks": "ci/runbooks.json",
                "runbooks_dir": "ci/runbooks-dir",
                "execution_artifacts": "ci/artifacts.json"
            },
            "review": {
                "reviews": "review/reviews.json",
                "code_review": "review/code-review.json",
                "work": "review/work.json",
                "cobs": "review/cobs.json",
                "artifact_previews": "review/previews.json",
                "preview_observations": "/abs/preview-obs"
            },
            "cairn": {
                "authoring": "cairn/authoring.json",
                "lattice_authoring": "cairn/lattice-authoring.json",
                "lattice_drafts": "cairn/lattice-drafts.json"
            },
            "observations": {
                "inventory": "obs/inventory.json",
                "implementation_work": "obs/impl.db",
                "campaign_controls": "obs/controls.json",
                "dispatch_recovery": "obs/recovery.json",
                "native_runtime_evidence": "obs/runtime.json",
                "delivery_observations": "obs/delivery.json"
            },
            "repository_roots": ["repos/a", "/srv/repos/b"]
        }"#;
        let base = Path::new("/var/lib/kiln");
        let (profile, options) = parse_workspace_config(raw, base)?;
        assert_eq!(profile, PathBuf::from("/var/lib/kiln/service.json"));
        assert_eq!(options.access, Some(PathBuf::from("/abs/access.json")));
        assert_eq!(
            options.sources,
            Some(PathBuf::from("/var/lib/kiln/sources.json"))
        );
        assert_eq!(
            options.fleet,
            Some(PathBuf::from("/var/lib/kiln/fleet.json"))
        );
        assert_eq!(
            options.automation,
            Some(PathBuf::from("/var/lib/kiln/ci/automation.json"))
        );
        assert_eq!(options.workers, Some(PathBuf::from("/abs/workers.json")));
        assert_eq!(
            options.cache,
            Some(PathBuf::from("/var/lib/kiln/ci/cache.json"))
        );
        assert_eq!(
            options.runbooks,
            Some(PathBuf::from("/var/lib/kiln/ci/runbooks.json"))
        );
        assert_eq!(
            options.runbooks_dir,
            Some(PathBuf::from("/var/lib/kiln/ci/runbooks-dir"))
        );
        assert_eq!(
            options.execution_artifacts,
            Some(PathBuf::from("/var/lib/kiln/ci/artifacts.json"))
        );
        assert_eq!(
            options.reviews,
            Some(PathBuf::from("/var/lib/kiln/review/reviews.json"))
        );
        assert_eq!(
            options.code_review,
            Some(PathBuf::from("/var/lib/kiln/review/code-review.json"))
        );
        assert_eq!(
            options.work,
            Some(PathBuf::from("/var/lib/kiln/review/work.json"))
        );
        assert_eq!(
            options.cobs,
            Some(PathBuf::from("/var/lib/kiln/review/cobs.json"))
        );
        assert_eq!(
            options.artifact_previews,
            Some(PathBuf::from("/var/lib/kiln/review/previews.json"))
        );
        assert_eq!(
            options.preview_observations,
            Some(PathBuf::from("/abs/preview-obs"))
        );
        assert_eq!(
            options.cairn_authoring,
            Some(PathBuf::from("/var/lib/kiln/cairn/authoring.json"))
        );
        assert_eq!(
            options.lattice_authoring,
            Some(PathBuf::from("/var/lib/kiln/cairn/lattice-authoring.json"))
        );
        assert_eq!(
            options.lattice_drafts,
            Some(PathBuf::from("/var/lib/kiln/cairn/lattice-drafts.json"))
        );
        assert_eq!(
            options.inventory,
            Some(PathBuf::from("/var/lib/kiln/obs/inventory.json"))
        );
        assert_eq!(
            options.implementation_work,
            Some(PathBuf::from("/var/lib/kiln/obs/impl.db"))
        );
        assert_eq!(
            options.campaign_controls,
            Some(PathBuf::from("/var/lib/kiln/obs/controls.json"))
        );
        assert_eq!(
            options.dispatch_recovery,
            Some(PathBuf::from("/var/lib/kiln/obs/recovery.json"))
        );
        assert_eq!(
            options.native_runtime_evidence,
            Some(PathBuf::from("/var/lib/kiln/obs/runtime.json"))
        );
        assert_eq!(
            options.delivery_observations,
            Some(PathBuf::from("/var/lib/kiln/obs/delivery.json"))
        );
        assert_eq!(
            options.repository_roots,
            vec![
                PathBuf::from("/var/lib/kiln/repos/a"),
                PathBuf::from("/srv/repos/b"),
            ]
        );
        Ok(())
    }

    #[test]
    fn cli_overrides_and_appends_over_loaded_config() -> TestResult {
        let dir = tempfile::tempdir()?;
        let cfg_path = dir.path().join("kiln.json");
        std::fs::write(
            &cfg_path,
            r#"{
                "schema": "kiln.factory-workspace.v1",
                "profile": "service.json",
                "access": "access.json",
                "fleet": "fleet.json",
                "repository_roots": ["repos/one"]
            }"#,
        )?;
        let cfg_str = cfg_path.to_string_lossy().into_owned();
        let parsed = parse_service_cli_args(args(&[
            "--config",
            &cfg_str,
            "--access",
            "/override/access.json",
            "--repository-root",
            "/override/repos/two",
        ]))
        .map_err(|err| format!("{err:?}"))?;

        assert_eq!(parsed.profile, dir.path().join("service.json"));
        assert_eq!(
            parsed.options.access,
            Some(PathBuf::from("/override/access.json"))
        );
        assert_eq!(parsed.options.fleet, Some(dir.path().join("fleet.json")));
        assert_eq!(
            parsed.options.repository_roots,
            vec![
                dir.path().join("repos/one"),
                PathBuf::from("/override/repos/two"),
            ]
        );
        Ok(())
    }

    #[test]
    fn rejects_unknown_field_wrong_schema_control_char_and_oversized_roots() {
        let base = Path::new("/etc/kiln");
        assert!(
            parse_workspace_config(
                r#"{"schema":"kiln.factory-workspace.v1","profile":"service.json","extra":true}"#,
                base,
            )
            .is_err()
        );

        assert!(
            parse_workspace_config(
                r#"{"schema":"kiln.factory-workspace.v99","profile":"service.json"}"#,
                base,
            )
            .is_err()
        );

        assert!(
            parse_workspace_config(
                r#"{"schema":"kiln.factory-workspace.v1","profile":"bad\npath.json"}"#,
                base,
            )
            .is_err()
        );

        assert!(
            parse_workspace_config(
                r#"{"schema":"kiln.factory-workspace.v1","profile":""}"#,
                base,
            )
            .is_err()
        );

        let too_many_roots: Vec<String> = (0..=MAX_REPOSITORY_ROOTS)
            .map(|i| format!("\"root-{i}\""))
            .collect();
        let oversized = format!(
            r#"{{"schema":"kiln.factory-workspace.v1","profile":"service.json","repository_roots":[{}]}}"#,
            too_many_roots.join(",")
        );
        assert!(parse_workspace_config(&oversized, base).is_err());
    }

    #[test]
    fn rejects_missing_config_and_profile_or_duplicate_flags() {
        assert_eq!(
            parse_service_cli_args(args(&["--access", "access.json"])),
            Err(ServiceCliError::Usage(
                "expected --config <path> or --profile <path>".to_owned()
            ))
        );
        assert_eq!(
            parse_service_cli_args(args(&["--help"])),
            Err(ServiceCliError::Help)
        );
        assert!(matches!(
            parse_service_cli_args(args(&["--profile", "a.json", "--profile", "b.json"])),
            Err(ServiceCliError::Usage(_))
        ));
        assert!(matches!(
            parse_service_cli_args(args(&["--config", "a.json", "--config", "b.json"])),
            Err(ServiceCliError::Usage(_))
        ));
        assert!(matches!(
            parse_service_cli_args(args(&[
                "--profile",
                "a.json",
                "--access",
                "x.json",
                "--access",
                "y.json"
            ])),
            Err(ServiceCliError::Usage(_))
        ));
    }

    #[test]
    fn load_workspace_config_rejects_oversized_file() -> TestResult {
        let dir = tempfile::tempdir()?;
        let cfg_path = dir.path().join("huge.json");
        std::fs::write(&cfg_path, "a".repeat(MAX_WORKSPACE_CONFIG_BYTES + 1))?;
        assert!(load_workspace_config(&cfg_path).is_err());
        Ok(())
    }
}
