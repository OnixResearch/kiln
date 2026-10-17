use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[derive(Debug)]
pub(super) struct Harness {
    root: tempfile::TempDir,
    pub(super) profile: kiln_radicle_nix_provider::HostedNixProviderProfile,
    pub(super) clock: fn() -> std::time::Instant,
}

impl Harness {
    pub(super) fn new(
        operation: &str,
        timeout_ms: u64,
        clock: fn() -> std::time::Instant,
    ) -> super::TestResult<Self> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source.git");
        let work = root.path().join("work");
        let reports = root.path().join("reports");
        for directory in [&source, &work, &reports.join("process-control")] {
            std::fs::create_dir_all(directory)?;
        }
        // All PID and release files are relative to the admitted working directory.
        // Only two owned processes exist: this wrapper and its shell descendant.
        let body = format!(
            r#"#!/bin/sh
[ "$#" = 4 ] || exit 41
[ "$1" = flake ] || exit 42
[ "$2" = check ] || exit 43
[ "$3" = --no-update-lock-file ] || exit 44
[ "$4" = 'git+file://{source}?rev={revision}' ] || exit 45
[ "${{KILN_PROVIDER_AMBIENT_SECRET+x}}" != x ] || exit 46
printf '%s\n' "$$" > wrapper.pid
/bin/sh -c 'printf "%s\n" "$$" > descendant.pid; while [ ! -f release-child ]; do :; done' &
descendant=$!
while [ ! -f proceed ]; do :; done
{operation}
"#,
            source = source.display(),
            revision = super::REVISION,
        );
        let wrapper = root.path().join("controlled-wrapper");
        std::fs::write(&wrapper, &body)?;
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))?;
        let uid = std::fs::metadata("/proc/self")?.uid();
        assert_ne!(uid, 0, "the provider intentionally refuses root authority");
        let profile = kiln_radicle_nix_provider::HostedNixProviderProfile {
            schema: kiln_radicle_nix_provider::HOSTED_NIX_PROVIDER_SCHEMA.to_string(),
            bounded_exec_revision: kiln_radicle_nix_provider::PINNED_BOUNDED_EXEC_REVISION
                .to_string(),
            durable_file_publication_revision:
                kiln_radicle_nix_provider::PINNED_DURABLE_FILE_PUBLICATION_REVISION.to_string(),
            allowed_user_id: uid,
            repository: super::REPOSITORY.to_string(),
            source_root: source,
            wrapper_path: wrapper,
            wrapper_blake3: blake3::hash(body.as_bytes()).to_hex().to_string(),
            working_directory: work,
            report_root: reports,
            report_namespace: "process-control".to_string(),
            report_base_url: "https://ci.invalid/process-control".to_string(),
            limits: kiln_radicle_nix_provider::HostedProviderLimits {
                timeout_milliseconds: timeout_ms,
                stdin_max_bytes: 1,
                stdout_max_bytes: super::OUTPUT_BOUND_BYTES,
                stderr_max_bytes: super::OUTPUT_BOUND_BYTES,
                poll_interval_milliseconds: super::POLL_INTERVAL_MS,
                teardown_timeout_milliseconds: super::TEARDOWN_BOUND_MS,
                wrapper_max_bytes: super::WRAPPER_BOUND_BYTES,
                report_max_bytes: super::REPORT_BOUND_BYTES,
                stage_collision_attempts: 4,
            },
            non_claims: kiln_radicle_nix_provider::REQUIRED_PROVIDER_NON_CLAIMS
                .map(str::to_string)
                .to_vec(),
        };
        Ok(Self {
            root,
            profile,
            clock,
        })
    }

    pub(super) fn start(&self) -> super::TestResult<super::running::OwnedChild> {
        let profile_path = self.root.path().join("profile.json");
        std::fs::write(&profile_path, serde_json::to_vec(&self.profile)?)?;
        let stdout_path = self.root.path().join("provider.stdout");
        let stderr_path = self.root.path().join("provider.stderr");
        let started = (self.clock)();
        let child = std::process::Command::new(env!("CARGO_BIN_EXE_kiln-radicle-nix-provider"))
            .args([
                "--profile",
                profile_path.to_str().ok_or("non-UTF8 fixture path")?,
            ])
            .args([
                "--repository",
                super::REPOSITORY,
                "--revision",
                super::REVISION,
            ])
            .env("KILN_PROVIDER_AMBIENT_SECRET", "must-not-reach-wrapper")
            .stdin(std::process::Stdio::null())
            .stdout(std::fs::File::create(&stdout_path)?)
            .stderr(std::fs::File::create(&stderr_path)?)
            .spawn()?;
        Ok(super::running::OwnedChild {
            child,
            started,
            work: self.profile.working_directory.clone(),
            stdout_path,
            stderr_path,
            owned: Vec::with_capacity(super::OWNED_PROCESS_COUNT),
            clock: self.clock,
        })
    }
}
