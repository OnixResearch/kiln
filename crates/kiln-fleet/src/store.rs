//! The durable implementation store and its error classes.
//!
//! `Implementer` holds a non-`Send` mirror observer slot, so it cannot be
//! shared behind a mutex across threads. Each API connection and the
//! dispatch thread open their own connection to the same SQLite database;
//! every store method runs in one immediate transaction, so SQLite
//! serializes them exactly as a shared mutex would, and no connection is held
//! across a child process.

use std::path::{Path, PathBuf};

use kiln_core::implementation::Enrollment;
use kiln_implementer::Implementer;
use kiln_implementer::authority_config::ImplementationAuthorityConfig;

use crate::config::read_bounded;

/// Largest enrollment document read.
pub const MAX_ENROLLMENT_BYTES: u64 = 256 * 1_024;

/// How to open the store.
#[derive(Clone, Debug)]
pub struct Store {
    pub database: PathBuf,
    pub enrollment: Enrollment,
    /// Authority profile selected for protected effects.
    pub authority: ImplementationAuthorityConfig,
}

impl Store {
    /// Load the enrollment file, resolve the authority selection and remember
    /// the database path.
    ///
    /// # Errors
    /// Returns a diagnostic for an unreadable or malformed enrollment, or an
    /// invalid authority selection.
    pub fn load(
        database: &Path,
        enrollment: &Path,
        authority_operator: Option<&str>,
        authority_config: Option<&Path>,
    ) -> Result<Self, String> {
        let bytes = read_bounded(enrollment, MAX_ENROLLMENT_BYTES)?;
        let enrollment: Enrollment = serde_json::from_slice(&bytes)
            .map_err(|error| format!("enrollment is malformed: {error}"))?;
        Ok(Self {
            database: database.to_path_buf(),
            enrollment,
            authority: ImplementationAuthorityConfig::resolve(
                authority_operator,
                authority_config,
            )?,
        })
    }

    /// Open one connection.
    ///
    /// The fleet claims, reserves, places, cancels and accepts candidate
    /// handoffs; verification and publication run as separate configured
    /// programs. It loads the coordinator holder seed and no other.
    ///
    /// # Errors
    /// Returns the store diagnostic.
    pub fn open(&self) -> Result<Implementer, String> {
        let mut implementer = Implementer::open(&self.database, self.enrollment.clone())?;
        implementer.select_authority(
            &self.authority,
            &std::collections::BTreeSet::from([
                kiln_core::implementation::grant::GrantRole::Coding,
            ]),
        )?;
        Ok(implementer)
    }
}

/// How the API reports one store refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorClass {
    Invalid,
    Forbidden,
    NotFound,
    Conflict,
    Internal,
}

impl ErrorClass {
    #[must_use]
    pub const fn status(self) -> u16 {
        match self {
            Self::Invalid => 400,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::Internal => 500,
        }
    }
}

const NOT_FOUND: [&str; 5] = [
    "fleet_claim_unknown",
    "fleet_session_unknown",
    "fleet_decision_unknown",
    "implementation_attempt_missing",
    "fleet_attempt_unknown",
];
const CONFLICT: [&str; 14] = [
    "fleet_claim_stale",
    "fleet_claim_limit",
    "fleet_decision_answered",
    "fleet_decision_closed",
    "fleet_decision_open",
    "fleet_decision_session",
    "fleet_session_transition",
    "fleet_session_bind",
    "fleet_session_limit",
    "implementation_fence_stale",
    "implementation_attempt_phase",
    "implementation_frozen_work_stale",
    "implementation_candidate_handoff",
    "fleet_handoff_session",
];
const FORBIDDEN: [&str; 1] = ["fleet_decision_operator"];

/// The diagnostic code of a `"code: message"` store error, if it has one.
#[must_use]
pub fn error_code(error: &str) -> Option<&str> {
    let (code, _) = error.split_once(": ")?;
    (!code.is_empty()
        && code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_'))
    .then_some(code)
}

/// Classify one store error: unknown identities are 404, stale or
/// conflicting state 409, other diagnostics 400, and errors without a
/// diagnostic code (SQLite, I/O) 500.
#[must_use]
pub fn classify(error: &str) -> ErrorClass {
    match error_code(error) {
        Some(code) if NOT_FOUND.contains(&code) => ErrorClass::NotFound,
        Some(code) if CONFLICT.contains(&code) => ErrorClass::Conflict,
        Some(code) if FORBIDDEN.contains(&code) => ErrorClass::Forbidden,
        Some(code) if code.starts_with("fleet_") || code.starts_with("implementation_") => {
            ErrorClass::Invalid
        }
        _ => ErrorClass::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // r[verify kiln.fleet.api]
    fn store_errors_map_to_consistent_statuses() {
        for (error, status) in [
            ("fleet_claim_unknown: no such fleet claim", 404),
            ("fleet_claim_stale: stale fence", 409),
            ("fleet_decision_operator: only an operator", 403),
            ("fleet_scope_path: bad path", 400),
            ("implementation_fence_stale: attempt does not own", 409),
            ("implementation_dispatch_bounds: x", 400),
            ("database is locked", 500),
            ("Weird: text", 500),
        ] {
            assert_eq!(classify(error).status(), status, "{error}");
        }
    }
}
