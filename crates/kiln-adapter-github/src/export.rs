use std::io::{self, Write};
use std::path::{Path, PathBuf};

use kiln_codec::encode_and_identify;
use kiln_core::domain::{Outcome, State};
use kiln_core::operations::{JournalRecord, replay};
use kiln_core::types::{CONTRACT_VERSION_V1, ContentId, ExternalRef, RunId};
use kiln_core::workflow::WorkflowSourceContext;
use kiln_provider_api::PortResult;
use serde::Serialize;

use crate::profile::AdmittedGithubConfig;
use crate::replay::load_read_only;
use crate::service::RunPhase;
use crate::{domain, infrastructure};

pub const OBSERVATION_SCHEMA: &str = "kiln.github-observation.v1";
pub const MAXIMUM_EXPORT_BYTES: usize = 16 * 1024 * 1024;
pub const MAXIMUM_EVENT_IDENTITY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Serialize)]
struct Observation<'a> {
    schema: &'static str,
    identity: &'a ContentId,
    run_identity: &'a ContentId,
    profile_identity: &'a ContentId,
    run_id: &'a RunId,
    phase: RunPhase,
    outcome: Outcome,
    declared_source: &'a Option<WorkflowSourceContext>,
    records: Vec<JournalRecord>,
}

// r[impl kiln.dashboard.repositories.github_export]
// The profile is admitted, but its secret references are never resolved here.
pub fn encode_observation(
    admitted: &AdmittedGithubConfig,
    run: &ContentId,
    database_snapshot: Option<&Path>,
) -> PortResult<Vec<u8>> {
    let record = load_read_only(
        database_snapshot.unwrap_or(&admitted.config.replay_database_path),
        &admitted.identity,
        run,
        admitted
            .config
            .maximum_record_bytes
            .min(MAXIMUM_EXPORT_BYTES),
    )?;
    let outcome = record.outcome()?;
    let stream = ExternalRef::new("kiln", "github-run", run.as_str())?;
    let mut records = Vec::with_capacity(record.events.len());
    for (revision, event) in record.events.into_iter().enumerate() {
        let (_, identity) =
            encode_and_identify(&event, MAXIMUM_EVENT_IDENTITY_BYTES).map_err(|_| {
                domain(
                    "github_export_event_bound",
                    "github.export",
                    "GitHub event exceeds its canonical identity encoding bound",
                )
            })?;
        records.push(JournalRecord {
            schema_version: CONTRACT_VERSION_V1,
            stream: stream.clone(),
            expected_revision: u64::try_from(revision).map_err(|_| export_bound())?,
            event_identity: ExternalRef::new("kiln", "ci-event-blake3", identity.as_str())?,
            event,
        });
    }
    let replayed = replay(&State::default(), &records)?;
    if replayed != record.ci_state
        || replayed.runs.get(&record.run_id).map(|run| run.outcome) != Some(outcome)
    {
        return Err(domain(
            "github_export_replay",
            "github.export",
            "GitHub canonical journal replay differs from its validated durable state",
        ));
    }
    let document = Observation {
        schema: OBSERVATION_SCHEMA,
        identity: &record.run_identity,
        run_identity: &record.run_identity,
        profile_identity: &record.profile_identity,
        run_id: &record.run_id,
        phase: record.phase,
        outcome,
        declared_source: &record.plan.source,
        records,
    };
    let mut writer = BoundedOutput(Vec::new());
    serde_json::to_writer(&mut writer, &document).map_err(|_| export_bound())?;
    writer.write_all(b"\n").map_err(|_| export_bound())?;
    Ok(writer.0)
}

// r[impl kiln.dashboard.repositories.github_export]
pub fn export_observation(
    admitted: &AdmittedGithubConfig,
    profile_path: &Path,
    run: &ContentId,
    output: &Path,
    database_snapshot: Option<&Path>,
) -> PortResult<()> {
    let output = checked_output(admitted, profile_path, output, database_snapshot)?;
    let bytes = encode_observation(admitted, run, database_snapshot)?;
    let parent = output.parent().ok_or_else(output_path_error)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|_| output_error())?;
    temporary.write_all(&bytes).map_err(|_| output_error())?;
    temporary.as_file().sync_all().map_err(|_| output_error())?;
    // Recheck aliases after encoding; never truncate a source, even on failure.
    checked_output(admitted, profile_path, &output, database_snapshot)?;
    temporary.persist(&output).map_err(|_| output_error())?;
    Ok(())
}

fn checked_output(
    admitted: &AdmittedGithubConfig,
    profile_path: &Path,
    output: &Path,
    database_snapshot: Option<&Path>,
) -> PortResult<PathBuf> {
    let name = output.file_name().ok_or_else(output_path_error)?;
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let output = std::fs::canonicalize(parent)
        .map_err(|_| output_path_error())?
        .join(name);
    match std::fs::symlink_metadata(&output) {
        Ok(metadata) if !metadata.file_type().is_file() => return Err(output_path_error()),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(output_path_error()),
    }
    let database = &admitted.config.replay_database_path;
    let mut protected = vec![database.clone(), profile_path.to_path_buf()];
    if let Some(snapshot) = database_snapshot {
        protected.push(snapshot.to_path_buf());
    }
    let databases = [Some(database.as_path()), database_snapshot];
    for database in databases.into_iter().flatten() {
        let paths = [
            Some(database.to_path_buf()),
            std::fs::canonicalize(database).ok(),
        ];
        for path in paths.into_iter().flatten() {
            for suffix in ["-wal", "-shm", "-journal"] {
                let mut sidecar = path.as_os_str().to_os_string();
                sidecar.push(suffix);
                protected.push(PathBuf::from(sidecar));
            }
        }
    }
    for source in protected {
        let resolved = match std::fs::canonicalize(&source) {
            Ok(path) => path,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                ) =>
            {
                if let Some(name) = source.file_name() {
                    let parent = source
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new("."));
                    std::fs::canonicalize(parent)
                        .map(|parent| parent.join(name))
                        .unwrap_or(source)
                } else {
                    source
                }
            }
            Err(_) => return Err(output_path_error()),
        };
        if resolved == output || same_file(&resolved, &output)? {
            return Err(output_path_error());
        }
    }
    Ok(output)
}

#[cfg(unix)]
fn same_file(left: &Path, right: &Path) -> PortResult<bool> {
    use std::os::unix::fs::MetadataExt;
    let metadata = |path: &Path| match std::fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        // A snapshot may be exported where the original service namespace is inaccessible.
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            Ok(None)
        }
        Err(_) => Err(output_path_error()),
    };
    Ok(match (metadata(left)?, metadata(right)?) {
        (Some(left), Some(right)) => left.dev() == right.dev() && left.ino() == right.ino(),
        _ => false,
    })
}

#[cfg(not(unix))]
fn same_file(_left: &Path, _right: &Path) -> PortResult<bool> {
    Err(output_path_error())
}

struct BoundedOutput(Vec<u8>);

impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAXIMUM_EXPORT_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other(
                "GitHub observation exceeds its byte bound",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn export_bound() -> kiln_provider_api::PortFailure {
    domain(
        "github_export_bound",
        "github.export",
        "GitHub observation exceeds its bounded JSON encoding",
    )
}

fn output_path_error() -> kiln_provider_api::PortFailure {
    domain(
        "github_export_output",
        "github.export.output",
        "GitHub export requires a regular output distinct from its profile, database and database sidecars",
    )
}

fn output_error() -> kiln_provider_api::PortFailure {
    infrastructure(
        "github_export_write",
        "GitHub observation could not be atomically installed",
    )
}
