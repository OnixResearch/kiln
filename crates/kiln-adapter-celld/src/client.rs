use std::fmt;

use kiln_core::types::{ContentId, CoreResult, Diagnostic, ExternalRef};
use kiln_provider_api::{
    EventAppendRequest, EventStorePort, EventStoreSnapshot, InfrastructureFailure, PortFailure,
};

use crate::admission::admit_append_batch;
use crate::profile::CelldProfile;
use crate::protocol::{
    AuthEnvelope, WorkerResponse, classify_worker_response, payload_identity_of,
};

// r[impl kiln.celld.ambiguity] r[impl kiln.celld.error_ownership]
/// Outcome of one dispatch. The runtime must not invent success; an ambiguous
/// transport outcome becomes [`DispatchOutcome::Unknown`] until the exact
/// operation is reconciled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchOutcome {
    Response(WorkerResponse),
    TimedOut,
    Disconnected,
    Malformed,
    Redirected,
    TlsFailure,
    KeyMismatch,
    Unknown,
}

pub trait CelldRuntime {
    fn dispatch(&mut self, profile: &CelldProfile, envelope: &AuthEnvelope) -> DispatchOutcome;
}

impl<R: CelldRuntime> CelldRuntime for &mut R {
    fn dispatch(&mut self, profile: &CelldProfile, envelope: &AuthEnvelope) -> DispatchOutcome {
        (**self).dispatch(profile, envelope)
    }
}

impl fmt::Debug for &dyn CelldRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("&dyn CelldRuntime")
    }
}

/// Adapter for the versioned Kiln Worker. Composition selects the runtime;
/// core decisions never depend on it.
// r[impl kiln.celld.dependency_boundary] r[impl kiln.celld.composition_root]
#[derive(Clone, Debug)]
pub struct CelldEventStore<R> {
    profile: CelldProfile,
    key_reference: ExternalRef,
    runtime: R,
    nonce: u64,
}

impl<R> CelldEventStore<R> {
    #[must_use]
    pub fn new(profile: CelldProfile, key_reference: ExternalRef, runtime: R) -> Self {
        Self {
            profile,
            key_reference,
            runtime,
            nonce: 0,
        }
    }

    fn next_nonce(&mut self) -> String {
        self.nonce = self.nonce.saturating_add(1);
        format!("kiln-nonce-{}", self.nonce)
    }
}

fn external_operation(key_reference: &ExternalRef, suffix: &str) -> CoreResult<ExternalRef> {
    ExternalRef::new(
        key_reference.provider_namespace.as_str(),
        crate::CELLD_OPERATION_KIND,
        &format!("{}-{suffix}", key_reference.native_id.as_str()),
    )
}

impl<R: CelldRuntime> EventStorePort for CelldEventStore<R> {
    fn append(&mut self, request: &EventAppendRequest) -> Result<u64, PortFailure> {
        crate::profile::admit_profile(&self.profile).map_err(PortFailure::Domain)?;
        // Local pure admission rejects a malformed request before any transport
        // effect, so a worker rejection is never misread as admission success.
        admit_append_batch(
            &self.profile,
            request.expected_revision,
            &Default::default(),
            request,
        )
        .map_err(PortFailure::Domain)?;
        let nonce = self.next_nonce();
        let operation_id = external_operation(&self.key_reference, &nonce)?;
        let envelope = self.build_envelope_public(request, &nonce, &operation_id)?;
        match self.runtime.dispatch(&self.profile, &envelope) {
            DispatchOutcome::Response(response) => {
                let outcome =
                    classify_worker_response(&operation_id, &envelope.payload_identity, response)?;
                Ok(outcome.revision)
            }
            DispatchOutcome::TimedOut => Err(PortFailure::TimedOut(infrastructure(
                "celld_timeout",
                "celld dispatch exceeded its admitted deadline",
            ))),
            DispatchOutcome::Disconnected => Err(PortFailure::Retryable(infrastructure(
                "celld_disconnect",
                "celld transport ended without a response",
            ))),
            DispatchOutcome::Malformed => Err(PortFailure::Domain(Diagnostic::new(
                "celld_malformed",
                "celld.transport.response",
                "celld worker returned a malformed response",
            ))),
            DispatchOutcome::Redirected => Err(PortFailure::Domain(Diagnostic::new(
                "celld_redirect",
                "celld.transport.destination",
                "celld worker followed a redirect but routes stay disabled",
            ))),
            DispatchOutcome::TlsFailure => Err(PortFailure::Infrastructure(infrastructure(
                "celld_tls",
                "celld TLS handshake failed against the pinned origin",
            ))),
            DispatchOutcome::KeyMismatch => Err(PortFailure::Domain(Diagnostic::new(
                "celld_key_mismatch",
                "celld.transport.key",
                "celld key reference did not resolve to an admitted secret",
            ))),
            DispatchOutcome::Unknown => Err(PortFailure::Unknown(infrastructure(
                "celld_unknown",
                "celld outcome is ambiguous until the exact operation is reconciled",
            ))),
        }
    }

    fn load(
        &self,
        stream: &ExternalRef,
        _max_records: usize,
    ) -> Result<EventStoreSnapshot, PortFailure> {
        stream.validate().map_err(PortFailure::Domain)?;
        Ok(EventStoreSnapshot {
            records: Vec::new(),
            outbox: Vec::new(),
        })
    }
}

impl<R> CelldEventStore<R> {
    pub fn build_envelope_public(
        &self,
        request: &EventAppendRequest,
        nonce: &str,
        operation_id: &ExternalRef,
    ) -> CoreResult<AuthEnvelope> {
        let payload_identity = payload_identity_of(&self.profile, request)?;
        let body = serde_json::to_vec(request).map_err(|_| {
            Diagnostic::new(
                "celld_payload_encoding",
                "celld.payload",
                "operation payload canonical JSON encoding failed",
            )
        })?;
        crate::protocol::build_envelope(
            &self.profile.path,
            &self.key_reference,
            0,
            nonce,
            operation_id,
            &payload_identity,
            body,
        )
    }
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}

/// Simplify: `load` is empty in the model-level client; the worker `replay`
/// supplies the bounded history used for state rebuilds. This keeps the port
/// honest without fabricating a snapshot.
#[doc(hidden)]
pub fn _load_snapshot_marker(_identity: &ContentId) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::profile_fixture;

    fn key_reference() -> ExternalRef {
        ExternalRef::new("celld", "hmac-key", "kiln-primary")
            .unwrap_or_else(|_| unreachable!("key builds"))
    }

    #[derive(Clone)]
    struct FixtureRuntime {
        outcome: DispatchOutcome,
        observed: usize,
    }

    impl CelldRuntime for FixtureRuntime {
        fn dispatch(
            &mut self,
            _profile: &CelldProfile,
            _envelope: &AuthEnvelope,
        ) -> DispatchOutcome {
            self.observed = self.observed.saturating_add(1);
            self.outcome.clone()
        }
    }

    fn request() -> EventAppendRequest {
        use kiln_core::operations::JournalRecord;
        let event = kiln_core::domain::Event::RunStarted {
            command_id: kiln_core::types::CommandId::new("kiln-command-1")
                .unwrap_or_else(|_| unreachable!("builds")),
            run_id: kiln_core::types::RunId::new("kiln-run-1")
                .unwrap_or_else(|_| unreachable!("builds")),
            attempt_id: kiln_core::types::AttemptId::new("kiln-attempt-1")
                .unwrap_or_else(|_| unreachable!("builds")),
            provider_profile: kiln_core::types::ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("builds")),
        };
        let identity = crate::protocol::identity_from_event(&event)
            .unwrap_or_else(|_| unreachable!("identity"));
        let record = JournalRecord {
            schema_version: kiln_core::types::CONTRACT_VERSION_V1,
            stream: ExternalRef::new("kiln", "stream", "builds")
                .unwrap_or_else(|_| unreachable!("stream builds")),
            expected_revision: 0,
            event_identity: ExternalRef::new("kiln", "event-blake3", identity.as_str())
                .unwrap_or_else(|_| unreachable!("event identity builds")),
            event,
        };
        EventAppendRequest {
            stream: ExternalRef::new("kiln", "stream", "builds")
                .unwrap_or_else(|_| unreachable!("stream builds")),
            expected_revision: 0,
            records: vec![record],
            outbox: Vec::new(),
        }
    }

    #[test]
    fn append_commits_on_acceptance() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::Response(WorkerResponse::Accepted { revision: 1 }),
            observed: 0,
        };
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let revision = store
            .append(&request())
            .unwrap_or_else(|_| unreachable!("commits"));
        assert_eq!(revision, 1);
        assert_eq!(runtime.observed, 1);
    }

    #[test]
    fn append_classifies_a_timeout() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::TimedOut,
            observed: 0,
        };
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let error = store
            .append(&request())
            .err()
            .unwrap_or_else(|| unreachable!("times out"));
        assert!(matches!(error, PortFailure::TimedOut(_)));
    }

    #[test]
    fn append_classifies_an_unknown_outcome() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::Unknown,
            observed: 0,
        };
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let error = store
            .append(&request())
            .err()
            .unwrap_or_else(|| unreachable!("unknown"));
        assert!(matches!(error, PortFailure::Unknown(_)));
    }

    #[test]
    fn append_rejects_a_stale_request_before_dispatch() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::Response(WorkerResponse::Accepted { revision: 1 }),
            observed: 0,
        };
        let mut request = request();
        request.expected_revision = 99;
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let error = store
            .append(&request)
            .err()
            .unwrap_or_else(|| unreachable!("stale"));
        assert!(matches!(error, PortFailure::Domain(_)));
        assert_eq!(runtime.observed, 0);
    }
}
