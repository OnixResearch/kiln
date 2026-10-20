use std::collections::BTreeSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use ucan::integration_profile::IntegrationFixtureManifest;
use ucan::integration_profile::IntegrationSupportProfile;
use ucan::integration_profile::SupportCatalog;
use ucan::integration_profile::admit_integration_profile;
use ucan::integration_profile::fixture_value_has_secret_like;
use ucan::integration_profile::validate_fixture_manifest;

use crate::error::ShellError;
use crate::nickel::Evaluation;
use crate::nickel::export;

const SUITE_PATH: &str = "contracts/integration-profiles/fixtures/suite.ncl";
const REQUIRED_CASE_IDS: &[&str] = &[
    "positive-controlled",
    "positive-preview",
    "positive-public-manifest",
    "negative-secret-manifest",
    "negative-proof-transport",
    "negative-unsupported-did",
    "negative-unsupported-signature",
    "negative-unknown-protocol-profile",
    "negative-implicit-preview",
    "negative-stale-catalog",
    "negative-production-overclaim",
    "negative-invalid-limit",
    "negative-duplicate-surface",
    "negative-missing-key-resolution",
    "negative-missing-revocation",
    "negative-missing-replay",
    "negative-missing-caveat-policy",
    "negative-missing-signer-custody",
];

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureCase {
    id: String,
    path: String,
    profile_path: String,
    kind: String,
    expected: String,
    expected_stage: String,
    expected_code: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SuiteStats {
    pub accepted_profiles: usize,
    pub rejected_profiles: usize,
    pub accepted_manifests: usize,
    pub rejected_manifests: usize,
    pub nickel_rejections: usize,
}

pub fn run(root: &Path, nickel: &Path, catalog: &SupportCatalog) -> Result<SuiteStats, ShellError> {
    let suite_evaluation = export(nickel, &root.join(SUITE_PATH))?;
    require_nickel_success(&suite_evaluation, SUITE_PATH)?;
    let cases: Vec<FixtureCase> = serde_json::from_slice(&suite_evaluation.stdout)
        .map_err(|error| ShellError::new("fixture-suite-schema", error.to_string()))?;
    validate_suite_coverage(&cases)?;
    let mut stats = SuiteStats::default();
    for case in &cases {
        run_case(root, nickel, catalog, case, &mut stats)?;
    }
    Ok(stats)
}

fn validate_suite_coverage(cases: &[FixtureCase]) -> Result<(), ShellError> {
    let ids = cases.iter().map(|case| case.id.as_str()).collect::<BTreeSet<_>>();
    if ids.len() != cases.len() {
        return Err(ShellError::new("fixture-suite-duplicate", "fixture suite contains duplicate ids"));
    }
    for required in REQUIRED_CASE_IDS {
        if !ids.contains(required) {
            return Err(ShellError::new(
                "fixture-suite-coverage",
                format!("fixture suite is missing required case {required}"),
            ));
        }
    }
    Ok(())
}

fn run_case(
    root: &Path,
    nickel: &Path,
    catalog: &SupportCatalog,
    case: &FixtureCase,
    stats: &mut SuiteStats,
) -> Result<(), ShellError> {
    let source = checked_path(root, &case.path)?;
    let evaluation = export(nickel, &source)?;
    if case.expected_stage == "nickel" {
        if evaluation.success {
            return Err(case_error(case, "Nickel accepted a fixture expected to fail its contract"));
        }
        stats.nickel_rejections += 1;
        return Ok(());
    }
    require_nickel_success(&evaluation, &case.path)?;
    match case.kind.as_str() {
        "profile" => run_profile_case(catalog, case, &evaluation, stats),
        "manifest" => run_manifest_case(root, nickel, catalog, case, &evaluation, stats),
        _ => Err(case_error(case, "fixture suite contains an unknown case kind")),
    }
}

fn run_profile_case(
    catalog: &SupportCatalog,
    case: &FixtureCase,
    evaluation: &Evaluation,
    stats: &mut SuiteStats,
) -> Result<(), ShellError> {
    let profile: IntegrationSupportProfile = serde_json::from_slice(&evaluation.stdout)
        .map_err(|error| case_error(case, &format!("profile JSON schema failed: {error}")))?;
    let report = admit_integration_profile(catalog, &profile);
    match case.expected.as_str() {
        "accept" if report.valid => {
            stats.accepted_profiles += 1;
            Ok(())
        }
        "reject" if !report.valid && report.issues.iter().any(|issue| issue.code == case.expected_code) => {
            stats.rejected_profiles += 1;
            Ok(())
        }
        "accept" => Err(case_error(case, &format!("profile was rejected: {:#?}", report.issues))),
        "reject" => Err(case_error(
            case,
            &format!("profile did not reject with expected code {}: {:#?}", case.expected_code, report.issues),
        )),
        _ => Err(case_error(case, "fixture suite contains an unknown expected outcome")),
    }
}

fn run_manifest_case(
    root: &Path,
    nickel: &Path,
    catalog: &SupportCatalog,
    case: &FixtureCase,
    evaluation: &Evaluation,
    stats: &mut SuiteStats,
) -> Result<(), ShellError> {
    if case.profile_path.is_empty() {
        return Err(case_error(case, "manifest case must bind a profile path"));
    }
    let profile_source = checked_path(root, &case.profile_path)?;
    let profile_evaluation = export(nickel, &profile_source)?;
    require_nickel_success(&profile_evaluation, &case.profile_path)?;
    let profile: IntegrationSupportProfile = serde_json::from_slice(&profile_evaluation.stdout)
        .map_err(|error| case_error(case, &format!("bound profile JSON schema failed: {error}")))?;
    let profile_report = admit_integration_profile(catalog, &profile);
    if !profile_report.valid {
        return Err(case_error(case, &format!("bound profile is not admitted: {:#?}", profile_report.issues)));
    }
    let fixture_value: serde_json::Value = serde_json::from_slice(&evaluation.stdout)
        .map_err(|error| case_error(case, &format!("fixture JSON parse failed: {error}")))?;
    if fixture_value_has_secret_like(&fixture_value) {
        if case.expected == "reject" && case.expected_code == "fixture-secret" {
            stats.rejected_manifests += 1;
            return Ok(());
        }
        return Err(case_error(case, "fixture contains secret-like fields or values"));
    }
    let manifest: IntegrationFixtureManifest = serde_json::from_value(fixture_value)
        .map_err(|error| case_error(case, &format!("fixture JSON schema failed: {error}")))?;
    let report = validate_fixture_manifest(catalog, &profile, &manifest);
    match case.expected.as_str() {
        "accept" if report.valid => {
            stats.accepted_manifests += 1;
            Ok(())
        }
        "reject" if !report.valid && report.issues.iter().any(|issue| issue.code == case.expected_code) => {
            stats.rejected_manifests += 1;
            Ok(())
        }
        "accept" => Err(case_error(case, &format!("fixture was rejected: {:#?}", report.issues))),
        "reject" => Err(case_error(
            case,
            &format!("fixture did not reject with expected code {}: {:#?}", case.expected_code, report.issues),
        )),
        _ => Err(case_error(case, "fixture suite contains an unknown expected outcome")),
    }
}

fn checked_path(root: &Path, relative: &str) -> Result<PathBuf, ShellError> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err(ShellError::new(
            "fixture-path",
            format!("fixture path {relative:?} is not safe and repository-relative"),
        ));
    }
    Ok(root.join(path))
}

fn require_nickel_success(evaluation: &Evaluation, source: &str) -> Result<(), ShellError> {
    if evaluation.success {
        return Ok(());
    }
    Err(ShellError::new(
        "nickel-contract",
        format!("Nickel rejected {source}: {}", evaluation.stderr.trim()),
    ))
}

fn case_error(case: &FixtureCase, message: &str) -> ShellError {
    ShellError::new("fixture-case", format!("{}: {message}", case.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_fixture_paths_stay_beneath_the_root() {
        let root = Path::new("/tmp/root");
        assert!(checked_path(root, "contracts/profile.ncl").is_ok());
        assert!(checked_path(root, "../secret").is_err());
        assert!(checked_path(root, "/absolute").is_err());
    }

    #[test]
    fn suite_coverage_rejects_missing_and_duplicate_cases() {
        assert!(validate_suite_coverage(&[]).is_err());
        let duplicate = FixtureCase {
            id: REQUIRED_CASE_IDS[0].to_owned(),
            path: "fixture.ncl".to_owned(),
            profile_path: String::new(),
            kind: "profile".to_owned(),
            expected: "accept".to_owned(),
            expected_stage: "admission".to_owned(),
            expected_code: "accepted".to_owned(),
        };
        assert!(validate_suite_coverage(&[duplicate.clone(), duplicate]).is_err());
    }
}
