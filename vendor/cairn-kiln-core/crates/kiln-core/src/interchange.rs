use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{
    ContentId, CoreResult, Diagnostic, ExternalRef, NativeEventId, ProviderProfileId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum InterchangeKind {
    Slsa,
    InToto,
    CdEvents,
    CloudEvents,
    OpenTelemetry,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InterchangeProfile {
    pub kind: InterchangeKind,
    pub specification_version: ExternalRef,
    pub supported_event_types: BTreeSet<ExternalRef>,
    pub required_fields: BTreeSet<ExternalRef>,
    pub allow_unknown_fields: bool,
    pub allow_lossy_mapping: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IdentityRoles {
    pub builder: ExternalRef,
    pub signer: Option<ExternalRef>,
    pub provider: ProviderProfileId,
    pub worker: Option<ExternalRef>,
    pub kiln_control_plane: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceProjection {
    pub profile: ExternalRef,
    pub pipeline_revision: ContentId,
    pub source_or_candidate: ContentId,
    pub producing_attempt: ContentId,
    pub products: Vec<ContentId>,
    pub roles: IdentityRoles,
    pub native_digests: Vec<ExternalRef>,
}

pub fn validate_provenance(
    projection: &ProvenanceProjection,
    expected_attempt: &ContentId,
) -> CoreResult<()> {
    if &projection.producing_attempt != expected_attempt {
        return Err(Diagnostic::new(
            "provenance_attempt_mismatch",
            "provenance.producing_attempt",
            "provenance is linked to another producing attempt",
        ));
    }
    projection.roles.builder.validate()?;
    for digest in &projection.native_digests {
        digest.validate()?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalEvent {
    pub profile_version: ExternalRef,
    pub event_type: ExternalRef,
    pub native_event_id: NativeEventId,
    pub source: ExternalRef,
    pub subject: ExternalRef,
    pub correlation: Option<ExternalRef>,
    pub causation: Option<ExternalRef>,
    pub fields: BTreeSet<ExternalRef>,
}

pub fn admit_external_event(profile: &InterchangeProfile, event: &ExternalEvent) -> CoreResult<()> {
    if profile.specification_version != event.profile_version {
        return Err(Diagnostic::new(
            "interchange_version",
            "event.profile_version",
            "interchange profile version is unsupported",
        ));
    }
    if !profile.supported_event_types.contains(&event.event_type) {
        return Err(Diagnostic::new(
            "interchange_event_type",
            "event.event_type",
            "interchange event type is unsupported",
        ));
    }
    if !profile.required_fields.is_subset(&event.fields) {
        return Err(Diagnostic::new(
            "interchange_missing_field",
            "event.fields",
            "interchange event omits a required field",
        ));
    }
    if !profile.allow_unknown_fields && !event.fields.is_subset(&profile.required_fields) {
        return Err(Diagnostic::new(
            "interchange_unknown_field",
            "event.fields",
            "interchange event includes an unknown field",
        ));
    }
    Ok(())
}

#[must_use]
pub fn duplicate_event(first: &ExternalEvent, second: &ExternalEvent) -> bool {
    first.native_event_id == second.native_event_id && first.source == second.source
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExternalEventDecision {
    Accepted,
    Duplicate,
}

pub fn decide_external_event(
    profile: &InterchangeProfile,
    event: &ExternalEvent,
    prior: &[ExternalEvent],
) -> CoreResult<ExternalEventDecision> {
    admit_external_event(profile, event)?;
    if prior.iter().any(|seen| duplicate_event(seen, event)) {
        return Ok(ExternalEventDecision::Duplicate);
    }
    let has_extra_fields = !event.fields.is_subset(&profile.required_fields);
    if has_extra_fields && !profile.allow_lossy_mapping {
        return Err(Diagnostic::new(
            "lossy_mapping_denied",
            "event.fields",
            "external event requires an undeclared lossy mapping",
        ));
    }
    if let Some(correlation) = &event.correlation
        && !prior
            .iter()
            .any(|seen| seen.native_event_id.as_str() == correlation.native_id.as_str())
    {
        return Err(Diagnostic::new(
            "missing_correlation",
            "event.correlation",
            "external event correlation does not name an admitted prior event",
        ));
    }
    Ok(ExternalEventDecision::Accepted)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RebuildableProjection {
    pub profile: ExternalRef,
    pub source_identity: ContentId,
    pub records: Vec<ExternalRef>,
    pub lifecycle_authority: bool,
    pub release_authority: bool,
}

pub fn validate_rebuildable_projection(projection: &RebuildableProjection) -> CoreResult<()> {
    if projection.lifecycle_authority || projection.release_authority {
        return Err(Diagnostic::new(
            "projection_authority_promotion",
            "projection.authority",
            "rebuildable projection cannot gain lifecycle or release authority",
        ));
    }
    projection.profile.validate()?;
    for record in &projection.records {
        record.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_success_has_no_state_authority() {
        let first = ExternalEvent {
            profile_version: ExternalRef::new("cdevents", "version", "v1")
                .unwrap_or_else(|_| unreachable!()),
            event_type: ExternalRef::new("cdevents", "event-type", "run-finished")
                .unwrap_or_else(|_| unreachable!()),
            native_event_id: NativeEventId::new("event").unwrap_or_else(|_| unreachable!()),
            source: ExternalRef::new("provider", "source", "source")
                .unwrap_or_else(|_| unreachable!()),
            subject: ExternalRef::new("provider", "run", "run").unwrap_or_else(|_| unreachable!()),
            correlation: None,
            causation: None,
            fields: BTreeSet::new(),
        };
        assert!(duplicate_event(&first, &first));
    }

    #[test]
    fn projections_cannot_promote_authority() {
        let projection = RebuildableProjection {
            profile: ExternalRef::new("otel", "profile", "v1").unwrap_or_else(|_| unreachable!()),
            source_identity: ContentId::from_blake3_hex(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap_or_else(|_| unreachable!()),
            records: Vec::new(),
            lifecycle_authority: true,
            release_authority: false,
        };
        assert!(validate_rebuildable_projection(&projection).is_err());
    }

    #[test]
    fn builder_and_control_plane_are_distinct() {
        let builder =
            ExternalRef::new("mantle", "builder", "builder").unwrap_or_else(|_| unreachable!());
        assert_ne!(builder.native_kind.as_str(), "kiln-control-plane");
    }
}
