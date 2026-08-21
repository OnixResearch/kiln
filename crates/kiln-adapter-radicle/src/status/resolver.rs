use std::ffi::OsString;
use std::path::PathBuf;

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope, run,
};
use kiln_core::types::Diagnostic;
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use zeroize::Zeroize as _;

use super::profile::SecretSpecDeclaration;
use super::secret::ResolvedSigner;

const SUCCESS_EXIT_CODE: i32 = 0;
const MAX_STDIN_BYTES: usize = 1;
const MAX_STDOUT_BYTES: usize = 4_096;
const MAX_STDERR_BYTES: usize = 4_096;
const RESOLVE_TIMEOUT_MS: u64 = 10_000;
const POLL_INTERVAL_MS: u64 = 20;
const TEARDOWN_TIMEOUT_MS: u64 = 2_000;

/// Resolves the declared SecretSpec secret into signer material.
///
/// The resolver runs the pinned `secretspec` executable through Bounded Exec
/// with a cleared environment and bounded output. It is fail-closed: identity
/// drift, provider failure, or a malformed value denies the effect before
/// signing. Resolved values live only in memory.
pub struct SignerResolver {
    declaration: SecretSpecDeclaration,
    executable: PathBuf,
    expected_blake3: String,
    limits: ExecutionLimits,
}

impl std::fmt::Debug for SignerResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SignerResolver")
            .field("declaration", &self.declaration)
            .field("executable", &self.executable)
            .field("expected_blake3", &self.expected_blake3)
            .finish_non_exhaustive()
    }
}

impl SignerResolver {
    pub fn new(
        declaration: SecretSpecDeclaration,
        executable: PathBuf,
        expected_blake3: String,
    ) -> Self {
        Self {
            declaration,
            executable,
            expected_blake3,
            limits: ExecutionLimits {
                timeout_ms: RESOLVE_TIMEOUT_MS,
                stdin_max_bytes: MAX_STDIN_BYTES,
                stdout_max_bytes: MAX_STDOUT_BYTES,
                stderr_max_bytes: MAX_STDERR_BYTES,
                poll_interval_ms: POLL_INTERVAL_MS,
                teardown_timeout_ms: TEARDOWN_TIMEOUT_MS,
            },
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> &SecretSpecDeclaration {
        &self.declaration
    }

    // r[impl kiln.radicle.status_signer.scenario.resolved]
    // r[impl kiln.radicle.status_signer.scenario.missing]
    /// Resolve the declared secret through the pinned SecretSpec executable.
    /// `reason` is passed to `--reason` for provider audit trails.
    pub fn resolve(&self, reason: &str) -> PortResult<ResolvedSigner> {
        if reason.trim().is_empty() {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_status_signer_reason",
                "radicle_status.signer",
                "SecretSpec access requires a non-empty reason",
            )));
        }
        self.verify_executable_identity()?;
        let mut value = self.run_get(reason)?;
        let signer = ResolvedSigner::from_resolved_secret(&value).map_err(PortFailure::Domain);
        value.zeroize();
        signer
    }

    fn verify_executable_identity(&self) -> PortResult<()> {
        let observed = hash_file(&self.executable)?;
        if observed != self.expected_blake3 {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_status_executable_drift",
                "radicle_status.executable",
                "SecretSpec executable identity differs from the pinned BLAKE3",
            )));
        }
        Ok(())
    }

    fn run_get(&self, reason: &str) -> PortResult<String> {
        let mut args = vec![
            OsString::from("get"),
            OsString::from(&self.declaration.secret_key),
            OsString::from("--profile"),
            OsString::from(&self.declaration.profile),
        ];
        if let Some(provider) = &self.declaration.provider {
            args.push(OsString::from("--provider"));
            args.push(OsString::from(provider));
        }
        args.push(OsString::from("--reason"));
        args.push(OsString::from(reason));
        let request = RunRequest {
            command: CommandSpec {
                program: self.executable.clone(),
                args,
                current_dir: self.declaration.project_root.clone(),
                environment_mode: EnvironmentMode::Clear,
                environment: Vec::new(),
                input: Input::Null,
            },
            limits: self.limits,
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: OutcomePolicy::new(vec![SUCCESS_EXIT_CODE], true, true).map_err(
                |_| {
                    PortFailure::Domain(Diagnostic::new(
                        "radicle_status_process_policy",
                        "radicle_status.signer",
                        "SecretSpec resolution outcome policy was invalid",
                    ))
                },
            )?,
        };
        let output = run(request).map_err(|error| {
            PortFailure::Infrastructure(InfrastructureFailure {
                code: "radicle_status_process_start".to_string(),
                message: format!("SecretSpec resolution could not start: {error}"),
            })
        })?;
        match output.disposition {
            Disposition::Succeeded => {
                let value = String::from_utf8(output.stdout.bytes).map_err(|_| {
                    PortFailure::Domain(Diagnostic::new(
                        "radicle_status_secret_encoding",
                        "radicle_status.signer",
                        "SecretSpec output was not valid UTF-8",
                    ))
                })?;
                Ok(value.trim().to_string())
            }
            Disposition::ExitFailed => Err(PortFailure::Domain(Diagnostic::new(
                "radicle_status_signer_unresolved",
                "radicle_status.signer",
                "SecretSpec did not resolve the declared signer secret",
            ))),
            Disposition::TimedOut => Err(PortFailure::Domain(Diagnostic::new(
                "radicle_status_signer_timeout",
                "radicle_status.signer",
                "SecretSpec resolution exceeded its named deadline",
            ))),
            Disposition::Cancelled | Disposition::OutputLimitExceeded(_) => {
                Err(PortFailure::Domain(Diagnostic::new(
                    "radicle_status_signer_bounded",
                    "radicle_status.signer",
                    "SecretSpec resolution exceeded its bounded output or was cancelled",
                )))
            }
        }
    }
}

fn hash_file(path: &PathBuf) -> PortResult<String> {
    let mut hasher = blake3::Hasher::new();
    let mut file = std::fs::File::open(path).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "radicle_status_executable_missing",
            "radicle_status.executable",
            "The pinned SecretSpec executable could not be opened",
        ))
    })?;
    std::io::copy(&mut file, &mut hasher).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "radicle_status_executable_unreadable",
            "radicle_status.executable",
            "The pinned SecretSpec executable could not be read",
        ))
    })?;
    Ok(hasher.finalize().to_string())
}
