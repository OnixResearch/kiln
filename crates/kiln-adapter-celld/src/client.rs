use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use kiln_core::types::{ContentId, CoreResult, Diagnostic, ExternalRef};
use kiln_provider_api::{
    EventAppendRequest, EventStorePort, EventStoreSnapshot, InfrastructureFailure, PortFailure,
};

use crate::admission::admit_append_batch;
use crate::admission::{
    CURRENT_SCHEMA_IDENTITY, EMPTY_SCHEMA_IDENTITY, MigrationPlan, StoredLease, StoredSchema,
};
use crate::http::HttpCelldRuntime;
use crate::profile::CelldProfile;
use crate::protocol::{
    AuthEnvelope, READ_PATH, RECONCILE_PATH, WorkerResponse, classify_worker_response,
};
use crate::protocol::{LEASE_PATH, MIGRATE_PATH};
use kiln_core::types::EffectId;

// r[impl kiln.celld.ambiguity]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationObservation {
    Absent,
    Committed {
        revision: u64,
        payload_identity: ContentId,
    },
}

pub trait CelldRuntime {
    fn dispatch(&mut self, profile: &CelldProfile, envelope: &AuthEnvelope) -> DispatchOutcome;
    fn reconcile(
        &self,
        profile: &CelldProfile,
        envelope: &AuthEnvelope,
    ) -> Result<OperationObservation, PortFailure>;
    fn load(
        &self,
        profile: &CelldProfile,
        envelope: &AuthEnvelope,
    ) -> Result<EventStoreSnapshot, PortFailure>;
}

impl<R: CelldRuntime> CelldRuntime for &mut R {
    fn dispatch(&mut self, profile: &CelldProfile, envelope: &AuthEnvelope) -> DispatchOutcome {
        (**self).dispatch(profile, envelope)
    }

    fn reconcile(
        &self,
        profile: &CelldProfile,
        envelope: &AuthEnvelope,
    ) -> Result<OperationObservation, PortFailure> {
        (**self).reconcile(profile, envelope)
    }

    fn load(
        &self,
        profile: &CelldProfile,
        envelope: &AuthEnvelope,
    ) -> Result<EventStoreSnapshot, PortFailure> {
        (**self).load(profile, envelope)
    }
}

impl fmt::Debug for &dyn CelldRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("&dyn CelldRuntime")
    }
}

/// Adapter for the versioned Kiln Worker. Composition selects the runtime;
/// core decisions never depend on it.
// r[impl kiln.celld.dependency_boundary]
#[derive(Clone, Debug)]
pub struct CelldEventStore<R> {
    profile: CelldProfile,
    key_reference: ExternalRef,
    runtime: R,
}

impl<R> CelldEventStore<R> {
    #[must_use]
    pub fn new(profile: CelldProfile, key_reference: ExternalRef, runtime: R) -> Self {
        Self {
            profile,
            key_reference,
            runtime,
        }
    }
}

fn external_operation(request: &EventAppendRequest) -> CoreResult<ExternalRef> {
    let event = request.records.first().ok_or_else(|| {
        Diagnostic::new(
            "celld_operation_empty",
            "celld.operation_id",
            "an append needs one event before its operation identity can be derived",
        )
    })?;
    ExternalRef::new(
        "kiln",
        crate::CELLD_OPERATION_KIND,
        event.event_identity.native_id.as_str(),
    )
}

fn fresh_nonce() -> Result<String, PortFailure> {
    let mut entropy = [0u8; 16];
    getrandom::fill(&mut entropy).map_err(|_| {
        PortFailure::Infrastructure(infrastructure(
            "celld_nonce_entropy",
            "operating system entropy unavailable; no authenticated request may be sent",
        ))
    })?;
    Ok(format!("{:032x}", u128::from_be_bytes(entropy)))
}

fn timestamp() -> CoreResult<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| {
            Diagnostic::new(
                "celld_clock",
                "celld.transport.timestamp",
                "system clock predates the protocol epoch",
            )
        })
}

#[derive(serde::Serialize)]
struct StreamQuery<'a> {
    stream: &'a ExternalRef,
}

#[derive(serde::Serialize)]
struct ReadQuery<'a> {
    stream: &'a ExternalRef,
    max_records: usize,
}

#[derive(serde::Serialize)]
struct LeaseQuery<'a> {
    stream: &'a ExternalRef,
    mode: &'static str,
    effect_id: &'a EffectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected: Option<&'a StoredLease>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<&'a StoredLease>,
}

#[derive(serde::Serialize)]
struct SchemaQuery<'a> {
    stream: &'a ExternalRef,
    mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_version: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_version: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_identity: Option<&'a ContentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_identity: Option<&'a ContentId>,
}

impl CelldEventStore<HttpCelldRuntime> {
    fn control_envelope<T: serde::Serialize>(
        &self,
        stream: &ExternalRef,
        path: &str,
        kind: &str,
        body: &T,
    ) -> Result<AuthEnvelope, PortFailure> {
        let now = timestamp()?;
        crate::profile::admit_profile_at(&self.profile, now).map_err(PortFailure::Domain)?;
        stream.validate().map_err(PortFailure::Domain)?;
        let body = serde_json::to_vec(body).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "celld_control_encoding",
                "celld.control",
                "signed control payload cannot be encoded",
            ))
        })?;
        let payload = kiln_codec::identity_from_bytes(&body).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "celld_control_identity",
                "celld.control",
                "signed control identity cannot be derived",
            ))
        })?;
        // Deriving the operation from the entire canonical payload binds exact
        // reconciliation to this stream, effect, and conditional transition.
        let operation = ExternalRef::new("kiln", kind, payload.as_str())?;
        Ok(crate::protocol::build_envelope(
            path,
            &self.key_reference,
            now,
            &fresh_nonce()?,
            &operation,
            &payload,
            body,
        )?)
    }

    fn control_committed(
        &self,
        stream: &ExternalRef,
        envelope: &AuthEnvelope,
    ) -> Result<bool, PortFailure> {
        let now = timestamp()?;
        crate::profile::admit_profile_at(&self.profile, now).map_err(PortFailure::Domain)?;
        let body = serde_json::to_vec(&StreamQuery { stream }).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "celld_control_encoding",
                "celld.control",
                "reconciliation cannot be encoded",
            ))
        })?;
        let lookup = crate::protocol::build_envelope(
            RECONCILE_PATH,
            &self.key_reference,
            now,
            &fresh_nonce()?,
            &envelope.operation_id,
            &envelope.payload_identity,
            body,
        )?;
        match self.runtime.reconcile(&self.profile, &lookup)? {
            OperationObservation::Absent => Ok(false),
            OperationObservation::Committed {
                payload_identity, ..
            } if payload_identity == envelope.payload_identity => Ok(true),
            OperationObservation::Committed { .. } => Err(PortFailure::Domain(Diagnostic::new(
                "celld_conflict",
                "celld.control",
                "stored control operation has another payload",
            ))),
        }
    }

    /// Authenticated read of the complete durable lease and its provider evidence.
    pub fn read_lease(
        &self,
        stream: &ExternalRef,
        effect_id: &EffectId,
    ) -> Result<StoredLease, PortFailure> {
        let envelope = self.control_envelope(
            stream,
            LEASE_PATH,
            "lease-read",
            &LeaseQuery {
                stream,
                mode: "read",
                effect_id,
                expected: None,
                next: None,
            },
        )?;
        let stored = self
            .runtime
            .lease_control(&self.profile, &envelope, "lease")?;
        if stored.lease.effect_id != *effect_id {
            return Err(control_damage("signed lease belongs to a different effect"));
        }
        Ok(stored)
    }

    /// Apply only a caller-supplied pure-core lease plan against the exact signed prior state.
    pub fn apply_lease(
        &self,
        stream: &ExternalRef,
        effect_id: &EffectId,
        expected: &StoredLease,
        next: &StoredLease,
        now: u64,
        lease_ticks: u64,
    ) -> Result<StoredLease, PortFailure> {
        if lease_ticks == 0
            || lease_ticks > self.profile.bounds.max_lease_ticks
            || expected.lease.effect_id != *effect_id
            || next.lease.effect_id != *effect_id
            || expected.lease.provider_profile != next.lease.provider_profile
            || expected.idempotency_key != next.idempotency_key
            || expected.completion != next.completion
            || expected.reconciliation != next.reconciliation
        {
            return Err(PortFailure::Domain(Diagnostic::new(
                "celld_lease_plan",
                "celld.lease",
                "lease plan changes identity or evidence or exceeds its bound",
            )));
        }
        let planned = crate::admission::plan_lease_claim(
            &expected.lease,
            next.lease.owner.clone().ok_or_else(|| {
                PortFailure::Domain(Diagnostic::new(
                    "celld_lease_plan",
                    "celld.lease",
                    "claimed lease needs an owner",
                ))
            })?,
            now,
            lease_ticks,
        )
        .map_err(PortFailure::Domain)?;
        if planned != next.lease {
            return Err(PortFailure::Domain(Diagnostic::new(
                "celld_lease_plan",
                "celld.lease",
                "claimed lease differs from the pure-core plan",
            )));
        }
        self.write_lease_plan(stream, effect_id, expected, next)
    }
    /// Store a shell-observed terminal outcome without provider redispatch.
    pub fn apply_reconciled_completion(
        &self,
        stream: &ExternalRef,
        effect_id: &EffectId,
        expected: &StoredLease,
        next: &StoredLease,
    ) -> Result<StoredLease, PortFailure> {
        if expected.lease.effect_id != *effect_id || next.lease.effect_id != *effect_id {
            return Err(PortFailure::Domain(Diagnostic::new(
                "celld_lease_plan",
                "celld.lease",
                "terminal evidence must belong to this effect",
            )));
        }
        let planned = crate::admission::plan_lease_reconciled_completion(
            expected,
            next.completion.clone().ok_or_else(|| {
                PortFailure::Domain(Diagnostic::new(
                    "celld_lease_plan",
                    "celld.lease",
                    "terminal completion evidence is required",
                ))
            })?,
            next.reconciliation.clone().ok_or_else(|| {
                PortFailure::Domain(Diagnostic::new(
                    "celld_lease_plan",
                    "celld.lease",
                    "provider reconciliation evidence is required",
                ))
            })?,
        )
        .map_err(PortFailure::Domain)?;
        if planned != *next {
            return Err(PortFailure::Domain(Diagnostic::new(
                "celld_lease_plan",
                "celld.lease",
                "terminal evidence differs from the pure-core plan",
            )));
        }
        self.write_lease_plan(stream, effect_id, expected, next)
    }

    fn write_lease_plan(
        &self,
        stream: &ExternalRef,
        effect_id: &EffectId,
        expected: &StoredLease,
        next: &StoredLease,
    ) -> Result<StoredLease, PortFailure> {
        let envelope = self.control_envelope(
            stream,
            LEASE_PATH,
            "lease-apply",
            &LeaseQuery {
                stream,
                mode: "apply",
                effect_id,
                expected: Some(expected),
                next: Some(next),
            },
        )?;
        if !self.control_committed(stream, &envelope)? {
            let reply = self
                .runtime
                .lease_control(&self.profile, &envelope, "updated")?;
            if reply != *next {
                return Err(control_damage(
                    "signed lease application differs from the requested plan",
                ));
            }
        }
        if !self.control_committed(stream, &envelope)? {
            return Err(PortFailure::Unknown(infrastructure(
                "celld_lease_unknown",
                "lease commit lacks exact operation evidence",
            )));
        }
        let stored = self.read_lease(stream, effect_id)?;
        if stored != *next {
            return Err(control_damage(
                "lease postflight does not match the committed plan",
            ));
        }
        Ok(stored)
    }

    /// Returns None only when the signed Worker confirms no schema on this stream.
    pub fn read_schema(&self, stream: &ExternalRef) -> Result<Option<StoredSchema>, PortFailure> {
        let envelope = self.control_envelope(
            stream,
            MIGRATE_PATH,
            "schema-read",
            &SchemaQuery {
                stream,
                mode: "read",
                from_version: None,
                to_version: None,
                input_identity: None,
                output_identity: None,
            },
        )?;
        let schema = self
            .runtime
            .schema_control(&self.profile, &envelope, "schema", true)?;
        if let Some(value) = &schema {
            let expected = match value.version {
                crate::INITIAL_SCHEMA_VERSION => EMPTY_SCHEMA_IDENTITY,
                crate::STORE_SCHEMA_VERSION => CURRENT_SCHEMA_IDENTITY,
                _ => return Err(control_damage("signed schema version is not admitted")),
            };
            if value.identity.as_str() != expected {
                return Err(control_damage(
                    "signed schema identity differs from the admitted cohort",
                ));
            }
            if value.version == crate::INITIAL_SCHEMA_VERSION && value.revision != 0 {
                return Err(control_damage("prior schema zero is not an empty stream"));
            }
        }
        Ok(schema)
    }

    /// Initialize the admitted prior schema only on a virgin empty stream.
    pub fn initialize_schema(&self, stream: &ExternalRef) -> Result<StoredSchema, PortFailure> {
        let envelope = self.control_envelope(
            stream,
            MIGRATE_PATH,
            "schema-initialize",
            &SchemaQuery {
                stream,
                mode: "initialize",
                from_version: None,
                to_version: None,
                input_identity: None,
                output_identity: None,
            },
        )?;
        if !self.control_committed(stream, &envelope)? {
            if self.read_schema(stream)?.is_some() {
                return Err(PortFailure::Domain(Diagnostic::new(
                    "celld_conflict",
                    "celld.schema",
                    "only a virgin stream can initialize schema zero",
                )));
            }
            let reply = self
                .runtime
                .schema_control(&self.profile, &envelope, "initialized", false)?
                .ok_or_else(|| control_damage("initialization returned no schema"))?;
            if reply.version != crate::INITIAL_SCHEMA_VERSION
                || reply.identity.as_str() != EMPTY_SCHEMA_IDENTITY
                || reply.revision != 0
            {
                return Err(control_damage(
                    "initialization did not commit schema zero on an empty stream",
                ));
            }
        }
        if !self.control_committed(stream, &envelope)? {
            return Err(PortFailure::Unknown(infrastructure(
                "celld_schema_unknown",
                "schema initialization lacks exact operation evidence",
            )));
        }
        let stored = self
            .read_schema(stream)?
            .ok_or_else(|| control_damage("initialization vanished"))?;
        if stored.version != crate::INITIAL_SCHEMA_VERSION
            || stored.identity.as_str() != EMPTY_SCHEMA_IDENTITY
            || stored.revision != 0
        {
            return Err(control_damage(
                "initialized schema postflight differs from schema zero",
            ));
        }
        Ok(stored)
    }

    /// Apply the sole admitted empty-schema-zero to schema-one edge, then verify durability.
    pub fn apply_migration(
        &self,
        stream: &ExternalRef,
        prior: &StoredSchema,
        plan: &MigrationPlan,
    ) -> Result<StoredSchema, PortFailure> {
        let admitted = crate::admission::plan_migration(
            prior.version,
            crate::STORE_SCHEMA_VERSION,
            &[(crate::INITIAL_SCHEMA_VERSION, crate::STORE_SCHEMA_VERSION)],
            self.profile.bounds.max_migration_steps,
        )
        .map_err(PortFailure::Domain)?;
        if plan != &admitted
            || plan.steps != [(0, 1)]
            || prior.identity.as_str() != EMPTY_SCHEMA_IDENTITY
            || prior.revision != 0
        {
            return Err(PortFailure::Domain(Diagnostic::new(
                "celld_migration_plan",
                "celld.schema",
                "only empty prior schema zero can migrate to schema one",
            )));
        }
        let input = ContentId::from_blake3_hex(EMPTY_SCHEMA_IDENTITY)?;
        let output = ContentId::from_blake3_hex(CURRENT_SCHEMA_IDENTITY)?;
        let envelope = self.control_envelope(
            stream,
            MIGRATE_PATH,
            "schema-migrate",
            &SchemaQuery {
                stream,
                mode: "apply",
                from_version: Some(0),
                to_version: Some(1),
                input_identity: Some(&input),
                output_identity: Some(&output),
            },
        )?;
        if !self.control_committed(stream, &envelope)? {
            if self.read_schema(stream)?.as_ref() != Some(prior) {
                return Err(PortFailure::Domain(Diagnostic::new(
                    "celld_conflict",
                    "celld.schema",
                    "schema changed before conditional migration",
                )));
            }
            let reply = self
                .runtime
                .schema_control(&self.profile, &envelope, "migrated", false)?
                .ok_or_else(|| control_damage("migration returned no schema"))?;
            if reply.version != 1 || reply.identity != output || reply.revision != 0 {
                return Err(control_damage(
                    "migration response differs from the admitted empty edge",
                ));
            }
        }
        if !self.control_committed(stream, &envelope)? {
            return Err(PortFailure::Unknown(infrastructure(
                "celld_migration_unknown",
                "schema migration lacks exact operation evidence",
            )));
        }
        let stored = self
            .read_schema(stream)?
            .ok_or_else(|| control_damage("migration vanished"))?;
        if stored.version != 1 || stored.identity != output || stored.revision != 0 {
            return Err(control_damage(
                "migration postflight differs from committed identity",
            ));
        }
        Ok(stored)
    }
}

fn control_damage(message: &str) -> PortFailure {
    PortFailure::Infrastructure(infrastructure("celld_store_damage", message))
}

impl<R: CelldRuntime> EventStorePort for CelldEventStore<R> {
    fn append(&mut self, request: &EventAppendRequest) -> Result<u64, PortFailure> {
        let now = timestamp()?;
        crate::profile::admit_profile_at(&self.profile, now).map_err(PortFailure::Domain)?;
        // Local pure admission rejects a malformed request before any transport
        // effect, so a worker rejection is never misread as admission success.
        let plan = admit_append_batch(
            &self.profile,
            request.expected_revision,
            &Default::default(),
            request,
        )
        .map_err(PortFailure::Domain)?;
        let operation_id = external_operation(request)?;
        let lookup_body = serde_json::to_vec(&StreamQuery {
            stream: &request.stream,
        })
        .map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "celld_lookup_encoding",
                "celld.reconcile",
                "exact operation lookup could not be encoded",
            ))
        })?;
        let lookup = crate::protocol::build_envelope(
            RECONCILE_PATH,
            &self.key_reference,
            now,
            &fresh_nonce()?,
            &operation_id,
            &plan.payload_identity,
            lookup_body,
        )?;
        match self.runtime.reconcile(&self.profile, &lookup)? {
            OperationObservation::Committed {
                revision,
                payload_identity,
            } if payload_identity == plan.payload_identity => return Ok(revision),
            OperationObservation::Committed { .. } => {
                return Err(PortFailure::Domain(Diagnostic::new(
                    "celld_conflict",
                    "celld.operation_id",
                    "the stored operation has a different payload identity",
                )));
            }
            OperationObservation::Absent => {}
        }
        let dispatch_at = timestamp()?;
        crate::profile::admit_profile_at(&self.profile, dispatch_at)
            .map_err(PortFailure::Domain)?;
        let envelope = crate::protocol::build_envelope(
            &self.profile.path,
            &self.key_reference,
            dispatch_at,
            &fresh_nonce()?,
            &operation_id,
            &plan.payload_identity,
            plan.encoded_payload,
        )?;
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
            DispatchOutcome::Disconnected => Err(PortFailure::Unknown(infrastructure(
                "celld_disconnect",
                "celld transport ended after possible acceptance; reconcile the exact operation before another dispatch",
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
        max_records: usize,
    ) -> Result<EventStoreSnapshot, PortFailure> {
        let now = timestamp()?;
        crate::profile::admit_profile_at(&self.profile, now).map_err(PortFailure::Domain)?;
        stream.validate().map_err(PortFailure::Domain)?;
        if max_records == 0 || max_records > 1024 {
            return Err(PortFailure::Domain(Diagnostic::new(
                "celld_replay_bound",
                "celld.replay.max_records",
                "read record count exceeds the Worker replay bound",
            )));
        }
        let body = serde_json::to_vec(&ReadQuery {
            stream,
            max_records,
        })
        .map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "celld_read_encoding",
                "celld.replay",
                "authenticated read request could not be encoded",
            ))
        })?;
        let identity = kiln_codec::identity_from_bytes(&body).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "celld_read_identity",
                "celld.replay",
                "authenticated read identity could not be derived",
            ))
        })?;
        let operation_id = ExternalRef::new("kiln", "read", stream.native_id.as_str())?;
        let envelope = crate::protocol::build_envelope(
            READ_PATH,
            &self.key_reference,
            now,
            &fresh_nonce()?,
            &operation_id,
            &identity,
            body,
        )?;
        let snapshot = self.runtime.load(&self.profile, &envelope)?;
        if snapshot.records.len() > max_records
            || snapshot.outbox.len() > self.profile.bounds.max_outbox_records
        {
            return Err(PortFailure::Infrastructure(infrastructure(
                "celld_read_bound",
                "Worker returned more rows than the admitted read bounds",
            )));
        }
        for (index, record) in snapshot.records.iter().enumerate() {
            if record.stream != *stream || record.expected_revision != index as u64 {
                return Err(PortFailure::Infrastructure(infrastructure(
                    "celld_store_damage",
                    "signed Worker history has a cross-stream or non-contiguous record",
                )));
            }
            crate::admission::admit_event_identity(&self.profile, record)
                .map_err(PortFailure::Domain)?;
        }
        Ok(snapshot)
    }
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}

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

        fn reconcile(
            &self,
            _profile: &CelldProfile,
            _envelope: &AuthEnvelope,
        ) -> Result<OperationObservation, PortFailure> {
            Ok(OperationObservation::Absent)
        }

        fn load(
            &self,
            _profile: &CelldProfile,
            _envelope: &AuthEnvelope,
        ) -> Result<EventStoreSnapshot, PortFailure> {
            Err(PortFailure::Infrastructure(infrastructure(
                "celld_read_unavailable",
                "fixture has no authenticated remote Worker read",
            )))
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
    fn a_missing_authenticated_read_never_fabricates_an_empty_journal() {
        let runtime = FixtureRuntime {
            outcome: DispatchOutcome::Response(WorkerResponse::Accepted { revision: 1 }),
            observed: 0,
        };
        let store = CelldEventStore::new(profile_fixture(), key_reference(), runtime);
        let error = store
            .load(&request().stream, 1)
            .err()
            .unwrap_or_else(|| unreachable!("unverified read must fail"));
        match error {
            PortFailure::Infrastructure(failure) => {
                assert_eq!(failure.code, "celld_read_unavailable");
            }
            _ => unreachable!("an unavailable read is infrastructure failure"),
        }
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

    // r[verify kiln.celld.partitioning.scenario.cross_partition]
    #[test]
    fn append_rejects_a_second_partition_before_transport() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::Response(WorkerResponse::Accepted { revision: 1 }),
            observed: 0,
        };
        let mut request = request();
        let partition_count = profile_fixture().bounds.max_partition_count;
        let own = crate::partition::map_partition(&request.stream, partition_count)
            .unwrap_or_else(|_| unreachable!("own partition"));
        let foreign = (0..32)
            .map(|index| {
                ExternalRef::new("kiln", "stream", &format!("foreign-{index}"))
                    .unwrap_or_else(|_| unreachable!("foreign stream"))
            })
            .find(|stream| {
                crate::partition::map_partition(stream, partition_count)
                    .map(|assignment| assignment.partition != own.partition)
                    .unwrap_or(false)
            })
            .unwrap_or_else(|| unreachable!("fixture needs a distinct partition"));
        request
            .records
            .first_mut()
            .unwrap_or_else(|| unreachable!("fixture record"))
            .stream = foreign;
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let error = store
            .append(&request)
            .err()
            .unwrap_or_else(|| unreachable!("two partitions must fail"));
        assert!(matches!(error, PortFailure::Domain(d) if d.code == "celld_event_sequence"));
        assert_eq!(runtime.observed, 0);
    }

    #[test]
    fn append_classifies_a_disconnection() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::Disconnected,
            observed: 0,
        };
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let error = store
            .append(&request())
            .err()
            .unwrap_or_else(|| unreachable!("disconnect"));
        assert!(matches!(error, PortFailure::Unknown(_)));
        assert_eq!(runtime.observed, 1);
    }

    #[test]
    fn append_classifies_a_malformed_response() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::Malformed,
            observed: 0,
        };
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let error = store
            .append(&request())
            .err()
            .unwrap_or_else(|| unreachable!("malformed"));
        assert!(matches!(error, PortFailure::Domain(_)));
    }

    #[test]
    fn append_classifies_a_redirect() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::Redirected,
            observed: 0,
        };
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let error = store
            .append(&request())
            .err()
            .unwrap_or_else(|| unreachable!("redirect"));
        let message = match error {
            PortFailure::Domain(diagnostic) => diagnostic.message,
            _ => unreachable!("redirect is a domain rejection"),
        };
        assert!(message.contains("redirect"));
    }

    #[test]
    fn append_classifies_a_tls_failure() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::TlsFailure,
            observed: 0,
        };
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let error = store
            .append(&request())
            .err()
            .unwrap_or_else(|| unreachable!("tls"));
        assert!(matches!(error, PortFailure::Infrastructure(_)));
    }

    #[test]
    fn append_classifies_a_key_mismatch() {
        let mut runtime = FixtureRuntime {
            outcome: DispatchOutcome::KeyMismatch,
            observed: 0,
        };
        let mut store = CelldEventStore::new(profile_fixture(), key_reference(), &mut runtime);
        let error = store
            .append(&request())
            .err()
            .unwrap_or_else(|| unreachable!("key mismatch"));
        assert!(matches!(error, PortFailure::Domain(_)));
    }
}
