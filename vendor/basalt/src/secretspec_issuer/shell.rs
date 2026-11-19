//! Imperative shell that runs the pinned `SecretSpec` SDK inside a bounded worker.
//!
//! This shell owns provider access, process/timeout bounds, and failure
//! plumbing. It never adds decision logic to the pure core; it only converts a
//! bounded SDK response (or a bound violation) into a typed
//! [`ShellResolution`] that [`crate::secretspec_issuer::classify_resolution`]
//! accepts.

use super::{
    DEFAULT_MAX_RESOLVED_VALUE_BYTES, DEFAULT_RESOLUTION_TIMEOUT, IssuerKeyDeclaration,
    build_resolve_request, classify_resolution,
};
use crate::secretspec_issuer::IssuerResolutionResult;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Explicit resource bounds applied by the bounded worker.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResolutionBoundaries {
    /// Maximum wall time the worker may spend in the SDK.
    pub timeout: Duration,
    /// Maximum accepted byte length of the resolution response.
    pub max_output_bytes: usize,
    /// Write operations must stay disabled for issuer-key resolution.
    pub disable_writes: bool,
    /// Plaintext value caching must stay disabled for issuer-key resolution.
    pub disable_plaintext_cache: bool,
}

impl Default for ResolutionBoundaries {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_RESOLUTION_TIMEOUT,
            max_output_bytes: DEFAULT_MAX_RESOLVED_VALUE_BYTES,
            disable_writes: true,
            disable_plaintext_cache: true,
        }
    }
}

impl ResolutionBoundaries {
    /// Return true only when the boundary keeps writes and plaintext caches disabled.
    #[must_use]
    pub const fn is_safely_disabled(&self) -> bool {
        self.disable_writes && self.disable_plaintext_cache
    }
}

/// Typed outcome of one bounded `SecretSpec` resolution.
// r[impl ucan_token_flow.oidc.secretspec_issuer_key]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ShellResolution {
    /// Pure classification of the bounded response.
    pub result: IssuerResolutionResult,
    /// Number of response bytes admitted within the output bound.
    pub output_bytes: usize,
}

impl ShellResolution {
    /// Return true only when the declared key resolved cleanly.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.result.allowed
    }
}

/// Stable failure reported when a bound is exceeded or the SDK is unavailable.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IssuerResolutionFailure {
    /// The worker exceeded the explicit timeout bound.
    Timeout,
    /// Write operations or plaintext caching were not disabled.
    WriteOrCacheEnabled,
    /// The SDK could not be started.
    SpawnFailed,
    /// The worker panicked inside the SDK boundary.
    WorkerPanic,
    /// The internal channel failed unexpectedly.
    ChannelFailure,
}

impl IssuerResolutionFailure {
    /// Return the stable serialized failure identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::WriteOrCacheEnabled => "write-or-cache-enabled",
            Self::SpawnFailed => "spawn-failed",
            Self::WorkerPanic => "worker-panic",
            Self::ChannelFailure => "channel-failure",
        }
    }
}

/// Resolve the declared issuer key through the pinned `SecretSpec` SDK.
///
/// The SDK `resolve_json` call runs inside an owned worker thread. The caller
/// supplies an explicit timeout and output-size bound; any violation disables
/// issuance and never falls back to a static key.
///
/// # Errors
///
/// Returns [`IssuerResolutionFailure::Timeout`] when the worker exceeds the
/// bound, or a related failure when the worker, SDK, or channel is unavailable.
pub fn resolve_issuer_key(
    declaration: &IssuerKeyDeclaration,
    config_path: &str,
    boundaries: ResolutionBoundaries,
) -> Result<ShellResolution, IssuerResolutionFailure> {
    if !boundaries.is_safely_disabled() {
        return Err(IssuerResolutionFailure::WriteOrCacheEnabled);
    }
    let request = build_resolve_request(declaration, config_path);
    let (sender, receiver) = mpsc::channel();

    let worker = thread::Builder::new()
        .name("basalt-secretspec-resolve".to_owned())
        .spawn(move || {
            let response = secretspec::resolve_json(request.as_str());
            let admitted = response.len().min(boundaries.max_output_bytes);
            let _ = sender.send((response, admitted));
        })
        .map_err(|_| IssuerResolutionFailure::SpawnFailed)?;

    let outcome = receiver
        .recv_timeout(boundaries.timeout)
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => IssuerResolutionFailure::Timeout,
            mpsc::RecvTimeoutError::Disconnected => IssuerResolutionFailure::ChannelFailure,
        })?;
    let (response, admitted) = outcome;
    worker
        .join()
        .map_err(|_| IssuerResolutionFailure::WorkerPanic)?;

    let result = classify_resolution(declaration, response.as_str(), admitted);
    Ok(ShellResolution {
        result,
        output_bytes: admitted,
    })
}
