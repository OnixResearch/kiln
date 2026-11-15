//! Retained exact preview observations and baseline/candidate comparison.
//!
//! A browser report is untrusted preview evidence. Every identity, binding,
//! engine and invocation field of a retained record comes from the admitted
//! descriptor; the browser supplies only bounded phases and a bounded result.
//! Records never change CI, review, merge or completion state.
// r[impl kiln.preview.comparison]
use kiln_core::preview::{PreviewBinding, PreviewDescriptor};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const OBSERVATION_SCHEMA: &str = "kiln.preview.observation.v1";
pub const COMPARISON_RULES: &str = "kiln.preview.comparison-rules.v1";
const OBSERVATION_DOMAIN: &[u8] = b"kiln.preview.observation.v1\0";
const COMPARISON_DOMAIN: &[u8] = b"kiln.preview.comparison.v1\0";
pub const MAX_REPORT_BYTES: usize = 64 * 1024;
const MAX_RECORD_BYTES: u64 = 192 * 1024;
pub const MAX_RETAINED_OBSERVATIONS: usize = 256;
pub const MAX_PROJECTED_OBSERVATIONS: usize = 32;
pub const OBSERVATION_RETENTION_SECONDS: u64 = 30 * 24 * 60 * 60;
const MAX_DETAIL_BYTES: usize = 512;
const MILLIS_PER_SECOND: u64 = 1000;
const PHASES: [&str; 4] = [
    "artifact discovery",
    "payload hydration",
    "guest boot",
    "program readiness",
];
const OUTCOMES: [&str; 2] = ["ready", "failed"];
pub const PROVENANCE: &str =
    "browser-reported preview observation; not CI, review, merge or completion evidence";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserReport {
    phases: Vec<BrowserPhase>,
    result: BrowserResult,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserPhase {
    phase: String,
    outcome: String,
    elapsed_ms: f64,
    #[serde(default)]
    detail: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserResult {
    status: Option<i64>,
    output: String,
    error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedPhase {
    pub phase: String,
    pub outcome: String,
    pub elapsed_ms: u64,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedResult {
    pub status: Option<i64>,
    pub output: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invocation {
    pub root_path: String,
    pub executable: String,
    pub arguments: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationBody {
    pub schema: String,
    pub descriptor_identity: String,
    pub binding: PreviewBinding,
    pub engine_identity: String,
    pub platform: String,
    pub invocation: Invocation,
    pub observed_at: u64,
    pub expires_at: u64,
    pub phases: Vec<ObservedPhase>,
    pub result: ObservedResult,
    pub provenance: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewObservation {
    pub identity: String,
    pub observation: ObservationBody,
}

/// One projected record for a review change; `current` only for the current
/// revision commit backed by the selected successful CI run.
#[derive(Clone, Debug, Serialize)]
pub struct ProjectedObservation {
    pub identity: String,
    pub current: bool,
    pub review_run_id: Option<String>,
    pub observation: ObservationBody,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Comparison {
    pub identity: String,
    pub rules: &'static str,
    pub baseline: String,
    pub candidate: String,
    pub compatible: bool,
    pub reasons: Vec<&'static str>,
    pub exit_status_equal: bool,
    pub output_equal: bool,
}

fn hash_with_domain(domain: &[u8], value: &impl Serialize) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&bytes);
    Ok(hasher.finalize().to_hex().to_string())
}

fn hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Select one projected baseline and the current candidate by exact identity.
/// Refusals carry an HTTP status and code; no other record is ever substituted.
pub fn select_pair<'a>(
    observations: &'a [ProjectedObservation],
    baseline: &str,
    candidate: &str,
) -> Result<(&'a ProjectedObservation, &'a ProjectedObservation), (u16, &'static str)> {
    if !hex64(baseline) || !hex64(candidate) {
        return Err((400, "preview_comparison_subject_invalid"));
    }
    let find = |identity: &str| observations.iter().find(|row| row.identity == identity);
    let baseline = find(baseline).ok_or((404, "preview_baseline_unavailable"))?;
    let candidate = find(candidate).ok_or((404, "preview_candidate_unavailable"))?;
    if !candidate.current {
        return Err((409, "preview_candidate_not_current"));
    }
    Ok((baseline, candidate))
}

/// Compose a retained record from an admitted descriptor and a bounded browser report.
pub fn compose(
    report: &[u8],
    descriptor_identity: &str,
    descriptor: &PreviewDescriptor,
    observed_at: u64,
) -> Result<PreviewObservation, String> {
    if report.len() > MAX_REPORT_BYTES {
        return Err("preview_observation_exhausted".into());
    }
    if !hex64(descriptor_identity) {
        return Err("preview_observation_descriptor_invalid".into());
    }
    let report: BrowserReport =
        serde_json::from_slice(report).map_err(|_| "preview_observation_malformed")?;
    let maximum_elapsed = descriptor
        .limits
        .run_seconds
        .saturating_mul(MILLIS_PER_SECOND) as f64;
    if report.phases.len() > PHASES.len() {
        return Err("preview_observation_phase_invalid".into());
    }
    let mut phases = Vec::with_capacity(report.phases.len());
    for (expected, phase) in PHASES.iter().zip(report.phases) {
        if phase.phase != *expected
            || !OUTCOMES.contains(&phase.outcome.as_str())
            || !phase.elapsed_ms.is_finite()
            || phase.elapsed_ms < 0.0
            || phase.elapsed_ms > maximum_elapsed
            || phase
                .detail
                .as_ref()
                .is_some_and(|detail| detail.len() > MAX_DETAIL_BYTES)
        {
            return Err("preview_observation_phase_invalid".into());
        }
        phases.push(ObservedPhase {
            phase: phase.phase,
            outcome: phase.outcome,
            elapsed_ms: phase.elapsed_ms.round() as u64,
            detail: phase.detail,
        });
    }
    // Only the final reported phase may have failed; later phases cannot follow a failure.
    if phases
        .iter()
        .rev()
        .skip(1)
        .any(|phase| phase.outcome != "ready")
    {
        return Err("preview_observation_phase_invalid".into());
    }
    let output_limit = usize::try_from(descriptor.limits.output_bytes).unwrap_or(usize::MAX);
    if report.result.output.len() > output_limit
        || report
            .result
            .error
            .as_ref()
            .is_some_and(|error| error.len() > MAX_DETAIL_BYTES)
    {
        return Err("preview_observation_result_invalid".into());
    }
    let observation = ObservationBody {
        schema: OBSERVATION_SCHEMA.into(),
        descriptor_identity: descriptor_identity.into(),
        binding: descriptor.binding.clone(),
        engine_identity: descriptor.engine.identity.as_str().into(),
        platform: descriptor.platform.as_str().into(),
        invocation: Invocation {
            root_path: descriptor.root_path.clone(),
            executable: descriptor.executable.clone(),
            arguments: descriptor.arguments.clone(),
        },
        observed_at,
        expires_at: observed_at.saturating_add(OBSERVATION_RETENTION_SECONDS),
        phases,
        result: ObservedResult {
            status: report.result.status,
            output: report.result.output,
            error: report.result.error,
        },
        provenance: PROVENANCE.into(),
    };
    Ok(PreviewObservation {
        identity: hash_with_domain(OBSERVATION_DOMAIN, &observation)?,
        observation,
    })
}

/// Recompute the identity and fixed provenance of a retained record.
pub fn verify(record: &PreviewObservation) -> Result<(), String> {
    if record.observation.schema != OBSERVATION_SCHEMA
        || record.observation.provenance != PROVENANCE
        || !hex64(&record.identity)
        || !hex64(&record.observation.descriptor_identity)
        || hash_with_domain(OBSERVATION_DOMAIN, &record.observation)? != record.identity
    {
        return Err("preview_observation_identity_mismatch".into());
    }
    Ok(())
}

pub fn validate_directory(path: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path).map_err(|_| "preview_observations_unavailable")?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != ::nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o022 != 0
    {
        return Err("preview_observations_not_owner_controlled".into());
    }
    path.canonicalize()
        .map_err(|_| "preview_observations_unavailable".into())
}

fn record_names(directory: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).map_err(|_| "preview_observations_unavailable")? {
        let entry = entry.map_err(|_| "preview_observations_unavailable")?;
        let name = entry.file_name();
        let Some(identity) = name.to_str().and_then(|name| name.strip_suffix(".json")) else {
            continue;
        };
        names.push((identity.to_owned(), entry.path()));
        if names.len() > MAX_RETAINED_OBSERVATIONS {
            return Err("preview_observations_exhausted".into());
        }
    }
    Ok(names)
}

/// Retain one verified record under its identity. A full store refuses rather than evicting.
pub fn retain(directory: &Path, record: &PreviewObservation) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let directory = validate_directory(directory)?;
    verify(record)?;
    let target = directory.join(format!("{}.json", record.identity));
    if target.exists() {
        return Ok(());
    }
    if record_names(&directory)?.len() >= MAX_RETAINED_OBSERVATIONS {
        return Err("preview_observations_full".into());
    }
    let staged = directory.join(format!(".{}.staged", record.identity));
    let bytes = serde_json::to_vec(record).map_err(|e| e.to_string())?;
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .map_err(|_| "preview_observation_staging_unavailable")?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    fs::rename(&staged, &target).map_err(|e| e.to_string())?;
    fs::File::open(&directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|e| e.to_string())
}

/// Read every verified, unexpired record, newest first. Any tampered record fails closed.
pub fn read_retained(directory: &Path, now: u64) -> Result<Vec<PreviewObservation>, String> {
    use std::os::unix::fs::MetadataExt;
    let directory = validate_directory(directory)?;
    let mut records = Vec::new();
    for (identity, path) in record_names(&directory)? {
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| "preview_observation_unavailable")?;
        if !metadata.is_file()
            || metadata.uid() != ::nix::unistd::geteuid().as_raw()
            || metadata.mode() & 0o022 != 0
        {
            return Err("preview_observation_not_owner_controlled".into());
        }
        let record: PreviewObservation = super::read_json(&path, MAX_RECORD_BYTES)?;
        verify(&record)?;
        if record.identity != identity {
            return Err("preview_observation_identity_mismatch".into());
        }
        if record.observation.expires_at > now {
            records.push(record);
        }
    }
    records.sort_by(|left, right| {
        right
            .observation
            .observed_at
            .cmp(&left.observation.observed_at)
            .then_with(|| left.identity.cmp(&right.identity))
    });
    Ok(records)
}

/// Remove expired verified records; returns how many were removed.
pub fn cleanup_expired(directory: &Path, now: u64) -> Result<usize, String> {
    let directory = validate_directory(directory)?;
    let mut removed = 0;
    for (_, path) in record_names(&directory)? {
        let record: PreviewObservation = super::read_json(&path, MAX_RECORD_BYTES)?;
        verify(&record)?;
        if record.observation.expires_at <= now {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Project records for one change. Unrelated repositories, projects and revisions are omitted.
pub fn project(
    records: Vec<PreviewObservation>,
    project: &str,
    repository: &str,
    commits: &BTreeSet<&str>,
    current_commit: &str,
    selected_runs: &[String],
    canonical_builds: &BTreeMap<String, crate::engine::PreviewBuildEvidence>,
) -> Vec<ProjectedObservation> {
    records
        .into_iter()
        .filter(|record| {
            let binding = &record.observation.binding;
            binding.project == project
                && binding.repository == repository
                && commits.contains(binding.revision.as_str())
        })
        .take(MAX_PROJECTED_OBSERVATIONS)
        .map(|record| {
            let binding = &record.observation.binding;
            let build = canonical_builds
                .get(binding.run_id.as_str())
                .filter(|build| {
                    super::service::canonical_build_matches(
                        binding,
                        &record.observation.invocation.executable,
                        build,
                    )
                });
            let review_run_id = build.map(|build| build.source_run_id.clone());
            let current = binding.revision == current_commit
                && review_run_id
                    .as_ref()
                    .is_some_and(|run| selected_runs.iter().any(|selected| selected == run));
            ProjectedObservation {
                identity: record.identity,
                current,
                review_run_id,
                observation: record.observation,
            }
        })
        .collect()
}

/// A borrowed comparison subject: one verified record identity and its body.
#[derive(Clone, Copy, Debug)]
pub struct Subject<'a> {
    pub identity: &'a str,
    pub observation: &'a ObservationBody,
}

impl PreviewObservation {
    pub fn subject(&self) -> Subject<'_> {
        Subject {
            identity: &self.identity,
            observation: &self.observation,
        }
    }
}

impl ProjectedObservation {
    pub fn subject(&self) -> Subject<'_> {
        Subject {
            identity: &self.identity,
            observation: &self.observation,
        }
    }
}

fn relative_executable(body: &ObservationBody) -> Option<&str> {
    body.invocation
        .executable
        .strip_prefix(body.invocation.root_path.as_str())
}

/// Deterministic comparison; every incompatibility is reported, never normalized away.
pub fn compare(baseline: Subject<'_>, candidate: Subject<'_>) -> Result<Comparison, String> {
    let (left, right) = (baseline.observation, candidate.observation);
    let mut reasons = Vec::new();
    if baseline.identity == candidate.identity {
        reasons.push("same_observation");
    }
    if left.engine_identity != right.engine_identity {
        reasons.push("engine_differs");
    }
    if left.platform != right.platform {
        reasons.push("platform_differs");
    }
    if left.invocation.arguments != right.invocation.arguments {
        reasons.push("arguments_differ");
    }
    if relative_executable(left).is_none()
        || relative_executable(left) != relative_executable(right)
    {
        reasons.push("executable_differs");
    }
    Ok(Comparison {
        identity: hash_with_domain(
            COMPARISON_DOMAIN,
            &(COMPARISON_RULES, baseline.identity, candidate.identity),
        )?,
        rules: COMPARISON_RULES,
        baseline: baseline.identity.to_owned(),
        candidate: candidate.identity.to_owned(),
        compatible: reasons.is_empty(),
        reasons,
        exit_status_equal: left.result.status == right.result.status,
        output_equal: left.result.output == right.result.output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> PreviewDescriptor {
        let root = "/nix/store/00000000000000000000000000000000-cli";
        let id = "a".repeat(64);
        serde_json::from_value(serde_json::json!({
            "version":1,"binding":{"project":"p","repository":"r","revision":"c".repeat(40),"run_id":"build-run","attempt_id":"attempt","candidate_id":"candidate","build_receipt":id,"product":"out","artifact_identity":id,"store_identity":id},
            "platform":"x86_64-linux","root_path":root,"executable":format!("{root}/bin/cli"),"arguments":["noop"],
            "closure":[{"path":root,"nar_sha256":id,"nar_size":16,"references":[],"signatures":["cache-1:signature"]}],"closure_complete":true,
            "engine":{"protocol":"trynix-startvm-v1","identity":"b".repeat(64),"platform":"x86_64-linux","guest_memory_bytes":536870912,"network_access":false,"supports_exact_closure":true,"supports_disposable_session":true},
            "trust_identity":id,"public_keys":["operator-key"],"issued_at":100,
            "limits":{"runtime_bytes":1024,"run_seconds":60,"output_bytes":64,"expires_at":200,"retain_after_session":false}
        }))
        .unwrap_or_else(|error| unreachable!("fixture descriptor: {error}"))
    }

    fn report(value: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&value).unwrap_or_else(|error| unreachable!("fixture report: {error}"))
    }

    fn complete() -> serde_json::Value {
        serde_json::json!({
            "phases":[
                {"phase":"artifact discovery","outcome":"ready","elapsed_ms":12.4},
                {"phase":"payload hydration","outcome":"ready","elapsed_ms":2000.6},
                {"phase":"guest boot","outcome":"ready","elapsed_ms":1500.0},
                {"phase":"program readiness","outcome":"ready","elapsed_ms":300.0}
            ],
            "result":{"status":0,"output":"emubench: done","error":null}
        })
    }

    fn descriptor_identity() -> String {
        "d".repeat(64)
    }

    fn composed() -> PreviewObservation {
        compose(
            &report(complete()),
            &descriptor_identity(),
            &descriptor(),
            1_000,
        )
        .unwrap_or_else(|error| unreachable!("fixture compose: {error}"))
    }

    // r[verify kiln.preview.comparison]
    #[test]
    fn record_binds_descriptor_facts_not_browser_claims() {
        let record = composed();
        assert!(verify(&record).is_ok());
        assert_eq!(record.observation.binding, descriptor().binding);
        assert_eq!(record.observation.engine_identity, "b".repeat(64));
        assert_eq!(
            record.observation.invocation.arguments,
            vec!["noop".to_owned()]
        );
        assert_eq!(
            record.observation.expires_at,
            1_000 + OBSERVATION_RETENTION_SECONDS
        );
        assert_eq!(
            record
                .observation
                .phases
                .iter()
                .map(|phase| phase.elapsed_ms)
                .collect::<Vec<_>>(),
            vec![12, 2001, 1500, 300]
        );
        let mut tampered = record.clone();
        tampered.observation.result.output = "forged".into();
        assert!(verify(&tampered).is_err());
        let mut relabelled = record;
        relabelled.observation.binding.revision = "e".repeat(40);
        assert!(verify(&relabelled).is_err());
    }

    fn set_phase(
        report: &mut serde_json::Value,
        index: usize,
        key: &str,
        value: serde_json::Value,
    ) {
        report
            .get_mut("phases")
            .and_then(|phases| phases.as_array_mut())
            .and_then(|phases| phases.get_mut(index))
            .and_then(|phase| phase.as_object_mut())
            .unwrap_or_else(|| unreachable!("fixture phase {index}"))
            .insert(key.into(), value);
    }

    // r[verify kiln.preview.comparison]
    #[test]
    fn malformed_or_out_of_bounds_reports_are_refused() {
        let descriptor = descriptor();
        let identity = descriptor_identity();
        let mut reordered = complete();
        if let Some(phases) = reordered.get_mut("phases").and_then(|v| v.as_array_mut()) {
            phases.swap(0, 1);
        }
        let mut unknown_phase = complete();
        set_phase(
            &mut unknown_phase,
            0,
            "phase",
            serde_json::json!("cache warmup"),
        );
        let mut after_failure = complete();
        set_phase(
            &mut after_failure,
            0,
            "outcome",
            serde_json::json!("failed"),
        );
        let mut negative = complete();
        set_phase(&mut negative, 0, "elapsed_ms", serde_json::json!(-1));
        let mut long_output = complete();
        long_output
            .get_mut("result")
            .and_then(|result| result.as_object_mut())
            .unwrap_or_else(|| unreachable!("fixture result"))
            .insert("output".into(), serde_json::json!("x".repeat(65)));
        let mut extra = complete();
        extra
            .as_object_mut()
            .unwrap_or_else(|| unreachable!("fixture report"))
            .insert("identity".into(), serde_json::json!("forged"));
        for rejected in [
            reordered,
            unknown_phase,
            after_failure,
            long_output,
            extra,
            negative,
        ] {
            assert!(compose(&report(rejected), &identity, &descriptor, 1).is_err());
        }
        assert!(compose(&vec![b' '; MAX_REPORT_BYTES + 1], &identity, &descriptor, 1).is_err());
        assert!(compose(&report(complete()), "not-an-identity", &descriptor, 1).is_err());
        let failed_boot = serde_json::json!({
            "phases":[
                {"phase":"artifact discovery","outcome":"ready","elapsed_ms":1},
                {"phase":"payload hydration","outcome":"failed","elapsed_ms":2,"detail":"member unavailable"}
            ],
            "result":{"status":null,"output":"","error":"preview_member_unavailable"}
        });
        assert!(compose(&report(failed_boot), &identity, &descriptor, 1).is_ok());
    }

    // r[verify kiln.preview.comparison]
    #[test]
    fn retention_is_owner_controlled_bounded_and_verified() -> Result<(), Box<dyn std::error::Error>>
    {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir()?;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))?;
        let record = composed();
        retain(directory.path(), &record)?;
        retain(directory.path(), &record)?;
        assert_eq!(
            read_retained(directory.path(), 1_001)?,
            vec![record.clone()]
        );
        assert!(read_retained(directory.path(), record.observation.expires_at)?.is_empty());
        let path = directory.path().join(format!("{}.json", record.identity));
        let mut forged = record.clone();
        forged.observation.result.status = Some(1);
        fs::write(&path, serde_json::to_vec(&forged)?)?;
        assert!(read_retained(directory.path(), 1_001).is_err());
        fs::write(&path, serde_json::to_vec(&record)?)?;
        assert_eq!(
            cleanup_expired(directory.path(), record.observation.expires_at)?,
            1
        );
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o777))?;
        assert!(retain(directory.path(), &record).is_err());
        Ok(())
    }

    // r[verify kiln.preview.comparison]
    #[test]
    fn full_store_refuses_instead_of_evicting() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir()?;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))?;
        for index in 0..MAX_RETAINED_OBSERVATIONS {
            fs::write(directory.path().join(format!("{index:064x}.json")), b"{}")?;
        }
        assert_eq!(
            retain(directory.path(), &composed()),
            Err("preview_observations_full".into())
        );
        Ok(())
    }

    // r[verify kiln.preview.comparison]
    #[test]
    fn comparison_reports_every_incompatibility_deterministically() {
        let baseline = composed();
        let mut later = descriptor();
        later.binding.revision = "e".repeat(40);
        later.root_path = "/nix/store/11111111111111111111111111111111-cli".into();
        later.executable = format!("{}/bin/cli", later.root_path);
        let candidate = compose(&report(complete()), &"f".repeat(64), &later, 2_000)
            .unwrap_or_else(|error| unreachable!("candidate: {error}"));
        let first = compare(baseline.subject(), candidate.subject())
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(first.compatible, "{:?}", first.reasons);
        assert!(first.exit_status_equal && first.output_equal);
        assert_eq!(
            compare(baseline.subject(), candidate.subject()).ok(),
            Some(first.clone())
        );
        let reversed = compare(candidate.subject(), baseline.subject())
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_ne!(reversed.identity, first.identity);

        let mut different = later.clone();
        different.arguments = vec!["--bench".into()];
        different.engine.identity = kiln_core::types::ContentId::from_blake3_hex("c".repeat(64))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        different.executable = format!("{}/bin/other", different.root_path);
        let unlike = compose(&report(complete()), &"f".repeat(64), &different, 2_000)
            .unwrap_or_else(|error| unreachable!("unlike: {error}"));
        let refused =
            compare(baseline.subject(), unlike.subject()).unwrap_or_else(|e| unreachable!("{e}"));
        assert!(!refused.compatible);
        assert_eq!(
            refused.reasons,
            vec!["engine_differs", "arguments_differ", "executable_differs"]
        );
        let itself =
            compare(baseline.subject(), baseline.subject()).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(itself.reasons, vec!["same_observation"]);
    }

    // r[verify kiln.preview.comparison]
    #[test]
    fn pair_selection_refuses_without_substituting() {
        let row = |record: PreviewObservation, current: bool| ProjectedObservation {
            identity: record.identity,
            current,
            review_run_id: current.then(|| "source-run".into()),
            observation: record.observation,
        };
        let historical = row(composed(), false);
        let mut later = descriptor();
        later.binding.revision = "e".repeat(40);
        let current = row(
            compose(&report(complete()), &"f".repeat(64), &later, 2_000)
                .unwrap_or_else(|error| unreachable!("current: {error}")),
            true,
        );
        let rows = [historical.clone(), current.clone()];
        let (old, new) = (historical.identity.as_str(), current.identity.as_str());
        let selected = select_pair(&rows, old, new).map(|(b, c)| (&b.identity, &c.identity));
        assert_eq!(selected, Ok((&historical.identity, &current.identity)));
        let (unknown, upper) = ("0".repeat(64), new.to_uppercase());
        for (baseline, candidate, refusal) in [
            ("short", new, (400, "preview_comparison_subject_invalid")),
            (
                old,
                upper.as_str(),
                (400, "preview_comparison_subject_invalid"),
            ),
            (unknown.as_str(), new, (404, "preview_baseline_unavailable")),
            (
                old,
                unknown.as_str(),
                (404, "preview_candidate_unavailable"),
            ),
            (new, old, (409, "preview_candidate_not_current")),
        ] {
            assert_eq!(
                select_pair(&rows, baseline, candidate).map(|_| ()),
                Err(refusal),
                "{baseline} ~ {candidate}"
            );
        }
    }

    // r[verify kiln.preview.comparison]
    #[test]
    fn projection_keeps_only_the_change_and_marks_currency() {
        let record = composed();
        let commit = "c".repeat(40);
        let commits: BTreeSet<&str> = [commit.as_str()].into_iter().collect();
        let builds = BTreeMap::new();
        let projected = project(
            vec![record.clone()],
            "p",
            "r",
            &commits,
            &commit,
            &["run".into()],
            &builds,
        );
        assert_eq!(projected.len(), 1);
        // Without the canonical build record the observation stays historical.
        assert!(
            projected
                .iter()
                .all(|row| !row.current && row.review_run_id.is_none())
        );
        let other_project = project(
            vec![record.clone()],
            "other",
            "r",
            &commits,
            &commit,
            &["run".into()],
            &builds,
        );
        assert!(other_project.is_empty());
        let zero = "0".repeat(40);
        let other_revision: BTreeSet<&str> = [zero.as_str()].into_iter().collect();
        assert!(
            project(
                vec![record],
                "p",
                "r",
                &other_revision,
                &commit,
                &[],
                &builds
            )
            .is_empty()
        );
    }
}
