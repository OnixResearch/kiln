use std::ffi::OsString;
use std::path::PathBuf;

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope, run,
};
use kiln_provider_api::{PortFailure, PortResult};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::{domain, infrastructure};

const MAX_SECRET_BYTES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretReference {
    pub program_path: PathBuf,
    pub program_blake3: String,
    pub project_root: PathBuf,
    pub profile: String,
    pub provider: Option<String>,
    pub secret_key: String,
}

/// Owned, redacted material; deliberately neither Clone nor serializable.
pub struct ResolvedSecret(Zeroizing<Vec<u8>>);

impl std::fmt::Debug for ResolvedSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ResolvedSecret([REDACTED])")
    }
}

impl ResolvedSecret {
    pub fn from_bytes(bytes: Vec<u8>) -> PortResult<Self> {
        let bytes = Zeroizing::new(bytes);
        if bytes.is_empty()
            || bytes.len() > MAX_SECRET_BYTES
            || std::str::from_utf8(&bytes).is_err()
            || bytes
                .iter()
                .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        {
            return Err(domain(
                "github_secret_value",
                "secret",
                "Secret material is invalid or exceeds its bound",
            ));
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_str(&self) -> PortResult<&str> {
        std::str::from_utf8(&self.0).map_err(|_| {
            domain(
                "github_secret_encoding",
                "secret",
                "Secret material is not UTF-8",
            )
        })
    }
}

// r[impl kiln.github.secrets]
pub fn admit_secret_reference(reference: &SecretReference) -> PortResult<()> {
    let name = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
            && !value.starts_with('-')
    };
    let path = |value: &std::path::Path| {
        value.is_absolute()
            && value.as_os_str().len() <= 4096
            && !value
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
            && value
                .as_os_str()
                .as_encoded_bytes()
                .iter()
                .all(|byte| !byte.is_ascii_control())
    };
    if !path(&reference.program_path)
        || !path(&reference.project_root)
        || reference.program_blake3.len() != 64
        || !reference
            .program_blake3
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !name(&reference.profile)
        || !name(&reference.secret_key)
        || reference
            .provider
            .as_ref()
            .is_some_and(|provider| !name(provider))
    {
        return Err(domain(
            "github_secret_reference",
            "secret_reference",
            "SecretSpec reference is invalid",
        ));
    }
    Ok(())
}

// r[impl kiln.github.secrets]
pub fn resolve_secret(reference: &SecretReference, reason: &str) -> PortResult<ResolvedSecret> {
    admit_secret_reference(reference)?;
    if reason.trim().is_empty()
        || reason.len() > 256
        || reason.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(domain(
            "github_secret_reason",
            "secret_reference",
            "SecretSpec requires a bounded access reason",
        ));
    }
    let mut file = std::fs::File::open(&reference.program_path).map_err(|_| {
        infrastructure(
            "github_secret_executable",
            "Pinned SecretSpec executable cannot be opened",
        )
    })?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher).map_err(|_| {
        infrastructure(
            "github_secret_executable",
            "Pinned SecretSpec executable cannot be read",
        )
    })?;
    if hasher.finalize().to_hex().as_str() != reference.program_blake3 {
        return Err(domain(
            "github_secret_drift",
            "secret_reference",
            "SecretSpec executable differs from its pinned identity",
        ));
    }
    let mut args = vec![
        OsString::from("get"),
        OsString::from(&reference.secret_key),
        OsString::from("--profile"),
        OsString::from(&reference.profile),
    ];
    if let Some(provider) = &reference.provider {
        args.extend([OsString::from("--provider"), OsString::from(provider)]);
    }
    args.extend([OsString::from("--reason"), OsString::from(reason)]);
    let mut output = run(RunRequest {
        command: CommandSpec {
            program: reference.program_path.clone(),
            args,
            current_dir: reference.project_root.clone(),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: 10_000,
            stdin_max_bytes: 1,
            stdout_max_bytes: MAX_SECRET_BYTES + 2,
            stderr_max_bytes: MAX_SECRET_BYTES,
            poll_interval_ms: 20,
            teardown_timeout_ms: 2000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true).map_err(|_| {
            infrastructure(
                "github_secret_policy",
                "SecretSpec execution policy is invalid",
            )
        })?,
    })
    .map_err(|_| {
        infrastructure(
            "github_secret_process",
            "SecretSpec resolution could not complete",
        )
    })?;
    // Both streams may contain credentials, including on failed/overflowed runs.
    output.stderr.bytes.zeroize();
    let mut bytes = Zeroizing::new(std::mem::take(&mut output.stdout.bytes));
    match output.disposition {
        Disposition::Succeeded => {
            let length = bytes.len();
            let end = if bytes.ends_with(b"\r\n") {
                length - 2
            } else if bytes.ends_with(b"\n") {
                length - 1
            } else {
                length
            };
            bytes
                .get_mut(end..)
                .ok_or_else(|| {
                    infrastructure(
                        "github_secret_output",
                        "SecretSpec output bounds were invalid",
                    )
                })?
                .zeroize();
            bytes.truncate(end);
            ResolvedSecret::from_bytes(std::mem::take(&mut *bytes))
        }
        Disposition::TimedOut => Err(PortFailure::TimedOut(
            kiln_provider_api::InfrastructureFailure {
                code: "github_secret_timeout".into(),
                message: "SecretSpec resolution exceeded its deadline".into(),
            },
        )),
        _ => Err(domain(
            "github_secret_unresolved",
            "secret",
            "SecretSpec resolution failed or exceeded its output bound",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolver(script: &str) -> (tempfile::TempDir, SecretReference) {
        let root = tempfile::tempdir()
            .unwrap_or_else(|_| unreachable!("fixture temporary directory is available"));
        // Execute an immutable interpreter. The owned protocol script is read,
        // never exec'd, so concurrent fork inheritance of a writer cannot
        // make this fixture transiently fail with Linux ETXTBSY.
        let program_path = PathBuf::from("/bin/sh");
        std::fs::write(root.path().join("get"), script)
            .unwrap_or_else(|_| unreachable!("fixture SecretSpec protocol script is written"));
        let executable = std::fs::read(&program_path)
            .unwrap_or_else(|_| unreachable!("fixture immutable interpreter is readable"));
        let reference = SecretReference {
            program_path,
            program_blake3: blake3::hash(&executable).to_hex().to_string(),
            project_root: root.path().to_owned(),
            profile: "ci".into(),
            provider: Some("fixture".into()),
            secret_key: "GITHUB_TOKEN".into(),
        };
        (root, reference)
    }

    // r[verify kiln.github.secrets]
    #[test]
    fn declared_resolution_checks_arguments_and_clears_environment() {
        let (_root, reference) = resolver(
            "#!/bin/sh\n[ -z \"$HOME\" ] || exit 1\n[ \"$0 $*\" = 'get GITHUB_TOKEN --profile ci --provider fixture --reason fixture access' ] || exit 2\nprintf 'fixture-token\\n'\n",
        );
        assert_eq!(
            resolve_secret(&reference, "fixture access")
                .unwrap_or_else(|_| unreachable!(
                    "pinned fixture resolver returns its declared secret"
                ))
                .as_bytes(),
            b"fixture-token"
        );
    }

    // r[verify kiln.github.secrets]
    #[test]
    fn drift_failures_and_malformed_output_are_redacted() {
        let (_root, mut reference) = resolver("#!/bin/sh\nprintf 'private-value\\n' >&2\nexit 1\n");
        let error = resolve_secret(&reference, "fixture")
            .err()
            .unwrap_or_else(|| unreachable!("fixture operation must fail"));
        assert!(!format!("{error:?}").contains("private-value"));
        // Exercise pin drift without ever modifying the shared interpreter.
        reference.program_blake3 = "0".repeat(64);
        assert!(matches!(
            resolve_secret(&reference, "fixture"),
            Err(PortFailure::Domain(_))
        ));
        let (_root, reference) = resolver("#!/bin/sh\nprintf 'bad token\\n'\n");
        assert!(resolve_secret(&reference, "fixture").is_err());
        let (_root, reference) =
            resolver("#!/bin/sh\ni=0; while [ $i -lt 5000 ]; do printf x; i=$((i+1)); done\n");
        assert!(resolve_secret(&reference, "fixture").is_err());
    }

    #[test]
    fn secret_values_reject_header_injection_and_invalid_encoding() {
        for bytes in [
            vec![],
            vec![0xff],
            b"token\r\nAuthorization: other".to_vec(),
            vec![b'x'; 4097],
        ] {
            assert!(ResolvedSecret::from_bytes(bytes).is_err());
        }
    }

    // r[verify kiln.github.secrets]
    #[test]
    fn debug_discloses_neither_secret_material_nor_secret_length() {
        let short = ResolvedSecret::from_bytes(b"private-short-token".to_vec())
            .unwrap_or_else(|_| unreachable!("fixture secret is admitted"));
        let long = ResolvedSecret::from_bytes(b"private-much-longer-token-material".to_vec())
            .unwrap_or_else(|_| unreachable!("fixture secret is admitted"));
        let short_debug = format!("{short:?}");
        assert!(
            !short_debug.contains(
                short
                    .as_str()
                    .unwrap_or_else(|_| unreachable!("admitted fixture secret is UTF-8"))
            )
        );
        assert_eq!(short_debug, format!("{long:?}"));
    }
}
