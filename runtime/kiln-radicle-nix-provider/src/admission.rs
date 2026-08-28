use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

use rustix::fs::{Mode, OFlags, open};

use super::model::{
    HOSTED_NIX_PROVIDER_SCHEMA, HostedNixProviderProfile, HostedProviderError,
    HostedProviderRequest, HostedProviderResult, PINNED_BOUNDED_EXEC_REVISION,
    PINNED_DURABLE_FILE_PUBLICATION_REVISION, REQUIRED_PROVIDER_NON_CLAIMS,
};

const PROFILE_MAXIMUM_BYTES: u64 = 1_048_576;
const BLAKE3_HEX_CHARACTERS: usize = 64;
const RADICLE_REVISION_HEX_CHARACTERS: usize = 40;
const REPORT_HEADER_MAXIMUM_BYTES: u64 = 4_096;

pub fn load_hosted_provider_profile(path: &Path) -> HostedProviderResult<HostedNixProviderProfile> {
    let mut file = File::open(path).map_err(|error| {
        HostedProviderError::Infrastructure(format!("open provider profile: {error}"))
    })?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(PROFILE_MAXIMUM_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            HostedProviderError::Infrastructure(format!("read provider profile: {error}"))
        })?;
    let maximum = usize::try_from(PROFILE_MAXIMUM_BYTES).map_err(|_| {
        HostedProviderError::Admission("profile bound does not fit this host".to_string())
    })?;
    if bytes.len() > maximum {
        return Err(HostedProviderError::Admission(
            "provider profile exceeds its named byte bound".to_string(),
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| HostedProviderError::Admission("provider profile JSON is invalid".to_string()))
}

pub(super) fn observe_and_admit(
    profile: &HostedNixProviderProfile,
    request: &HostedProviderRequest,
) -> HostedProviderResult<()> {
    let observed_user_id = std::fs::metadata("/proc/self")
        .map_err(|error| {
            HostedProviderError::Infrastructure(format!("observe provider user: {error}"))
        })?
        .uid();
    let observed_wrapper_blake3 =
        hash_bounded_file(&profile.wrapper_path, profile.limits.wrapper_max_bytes)?;
    admit_profile(profile, observed_user_id, &observed_wrapper_blake3)?;
    admit_request(profile, request)
}

fn admit_profile(
    profile: &HostedNixProviderProfile,
    observed_user_id: u32,
    observed_wrapper_blake3: &str,
) -> HostedProviderResult<()> {
    let limits = profile.limits;
    let output_and_header = limits
        .stdout_max_bytes
        .checked_add(limits.stderr_max_bytes)
        .and_then(|bytes| bytes.checked_add(REPORT_HEADER_MAXIMUM_BYTES))
        .ok_or_else(|| HostedProviderError::Admission("report bound overflow".to_string()))?;
    let is_limits_positive = limits.timeout_milliseconds > 0
        && limits.stdout_max_bytes > 0
        && limits.stderr_max_bytes > 0
        && limits.poll_interval_milliseconds > 0
        && limits.teardown_timeout_milliseconds > 0
        && limits.wrapper_max_bytes > 0
        && limits.report_max_bytes > 0
        && limits.stage_collision_attempts > 0;
    if profile.schema != HOSTED_NIX_PROVIDER_SCHEMA
        || profile.bounded_exec_revision != PINNED_BOUNDED_EXEC_REVISION
        || profile.durable_file_publication_revision != PINNED_DURABLE_FILE_PUBLICATION_REVISION
        || profile.allowed_user_id == 0
        || profile.allowed_user_id != observed_user_id
        || !safe_absolute_path(&profile.source_root)
        || !safe_absolute_path(&profile.wrapper_path)
        || !safe_absolute_path(&profile.working_directory)
        || !safe_absolute_path(&profile.report_root)
        || profile.source_root == profile.report_root
        || profile.wrapper_blake3 != observed_wrapper_blake3
        || !valid_blake3_hex(&profile.wrapper_blake3)
        || !safe_label(&profile.report_namespace)
        || !profile.report_base_url.starts_with("https://")
        || !is_limits_positive
        || output_and_header > limits.report_max_bytes
        || profile.non_claims != REQUIRED_PROVIDER_NON_CLAIMS.map(str::to_string)
    {
        return Err(HostedProviderError::Admission(
            "provider profile differs from the reviewed cohort".to_string(),
        ));
    }
    require_directory(&profile.source_root, "source root")?;
    require_directory(&profile.working_directory, "working directory")?;
    require_directory(&profile.report_root, "report root")?;
    require_directory(
        &profile.report_root.join(&profile.report_namespace),
        "report namespace",
    )?;
    Ok(())
}

fn admit_request(
    profile: &HostedNixProviderProfile,
    request: &HostedProviderRequest,
) -> HostedProviderResult<()> {
    if request.repository != profile.repository
        || request.repository.is_empty()
        || request.revision.len() != RADICLE_REVISION_HEX_CHARACTERS
        || !request
            .revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(HostedProviderError::Admission(
            "provider request differs from the admitted repository or revision".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn hash_bounded_file(path: &Path, maximum_bytes: u64) -> HostedProviderResult<String> {
    if maximum_bytes == 0 || !safe_absolute_path(path) {
        return Err(HostedProviderError::Admission(
            "wrapper identity needs an absolute path and positive bound".to_string(),
        ));
    }
    let descriptor = open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        HostedProviderError::Infrastructure(format!("open provider wrapper: {error}"))
    })?;
    let mut file = File::from(descriptor);
    let metadata = file.metadata().map_err(|error| {
        HostedProviderError::Infrastructure(format!("inspect provider wrapper: {error}"))
    })?;
    if !metadata.is_file() || metadata.len() > maximum_bytes {
        return Err(HostedProviderError::Admission(
            "provider wrapper is not a bounded regular file".to_string(),
        ));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            HostedProviderError::Infrastructure(format!("read provider wrapper: {error}"))
        })?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > maximum_bytes) {
        return Err(HostedProviderError::Admission(
            "provider wrapper exceeds its named byte bound".to_string(),
        ));
    }
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn valid_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_CHARACTERS
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn safe_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path.file_name().is_some()
        && !path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        && !path
            .as_os_str()
            .as_encoded_bytes()
            .iter()
            .any(|byte| matches!(byte, b'?' | b'#' | 0))
}

fn safe_label(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn require_directory(path: &Path, label: &str) -> HostedProviderResult<()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        HostedProviderError::Infrastructure(format!("inspect {label}: {error}"))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(HostedProviderError::Admission(format!(
            "{label} is not an admitted directory"
        )));
    }
    Ok(())
}
