//! Lattice's closed wait-reason vocabulary (`docs/worker-inventory.md`,
//! "Wait reasons") mirrored for validation, and its mapping into
//! provider-neutral inventory wait reasons. A reason is an explanation, never
//! a reservation or dispatch decision, and it carries no free text.
// r[impl kiln.inventory.producers.scenario.wait]
use kiln_core::inventory::{WaitReason, WaitSubject};
use serde::Deserialize;

/// Lattice bounds each profile's and instance's reasons (`MAX_WAIT_REASONS`).
pub(super) const MAX_LATTICE_WAIT_REASONS: usize = 32;
/// The profile's own worker configuration declares the missing fact.
const CONFIGURATION_OWNER: &str = "configuration";
/// Qualification evidence that promotes declared support.
const QUALIFICATION_OWNER: &str = "animus#18";
/// Desired, prepared, qualified, selected and pinned revisions.
pub(super) const REVISION_OWNER: &str = "lattice#25";
/// Serving readiness of one route.
const SERVING_OWNER: &str = "animus#17";
/// Global claims, attempts and dispatch.
const DISPATCH_OWNER: &str = "kiln#45";
/// One admitted attempt's recovery and cancellation observation.
const ATTEMPT_OWNER: &str = "lattice#6";
/// Live host, serving port and liveness observation.
pub(super) const LIVE_HOST_OWNER: &str = "kiln#49";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(super) enum Code {
    CapabilityUndeclared,
    QualificationUnbound,
    RevisionSelectionUnpublished,
    ServingPortAbsent,
    ServingSnapshotUnavailable,
    ServingPortIssue,
    ServingSnapshotNotCurrent,
    ServingNotReady,
    AwaitingDispatch,
    AdmissionExpired,
    DispatchOutcomeUnknown,
    CancellationUnobserved,
    LivenessUnobserved,
    LivenessStale,
    HostReachabilityUnknown,
    AllocationLivenessUnknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(super) enum Field {
    TaskFamilies,
    Tools,
    QualificationRef,
    Revisions,
    ServingReadiness,
    Lifecycle,
    Cancellation,
    Liveness,
    HostReachability,
    AllocationLiveness,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum SubjectDto {
    Fact { field: Field },
    Route { name: String },
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
pub(super) struct ReasonDto {
    pub(super) code: Code,
    pub(super) subject: SubjectDto,
    pub(super) owner: String,
}

/// Where a code may appear.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Section {
    Profile,
    Instance,
}

impl Code {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CapabilityUndeclared => "capability_undeclared",
            Self::QualificationUnbound => "qualification_unbound",
            Self::RevisionSelectionUnpublished => "revision_selection_unpublished",
            Self::ServingPortAbsent => "serving_port_absent",
            Self::ServingSnapshotUnavailable => "serving_snapshot_unavailable",
            Self::ServingPortIssue => "serving_port_issue",
            Self::ServingSnapshotNotCurrent => "serving_snapshot_not_current",
            Self::ServingNotReady => "serving_not_ready",
            Self::AwaitingDispatch => "awaiting_dispatch",
            Self::AdmissionExpired => "admission_expired",
            Self::DispatchOutcomeUnknown => "dispatch_outcome_unknown",
            Self::CancellationUnobserved => "cancellation_unobserved",
            Self::LivenessUnobserved => "liveness_unobserved",
            Self::LivenessStale => "liveness_stale",
            Self::HostReachabilityUnknown => "host_reachability_unknown",
            Self::AllocationLivenessUnknown => "allocation_liveness_unknown",
        }
    }

    /// The owner Lattice fixes for this code.
    const fn owner(self) -> &'static str {
        match self {
            Self::CapabilityUndeclared => CONFIGURATION_OWNER,
            Self::QualificationUnbound => QUALIFICATION_OWNER,
            Self::RevisionSelectionUnpublished => REVISION_OWNER,
            Self::ServingSnapshotUnavailable
            | Self::ServingPortIssue
            | Self::ServingSnapshotNotCurrent
            | Self::ServingNotReady => SERVING_OWNER,
            Self::AwaitingDispatch | Self::AdmissionExpired => DISPATCH_OWNER,
            Self::DispatchOutcomeUnknown | Self::CancellationUnobserved => ATTEMPT_OWNER,
            Self::ServingPortAbsent
            | Self::LivenessUnobserved
            | Self::LivenessStale
            | Self::HostReachabilityUnknown
            | Self::AllocationLivenessUnknown => LIVE_HOST_OWNER,
        }
    }

    /// Codes about the definition itself, as opposed to one inspection of
    /// serving readiness or one instance.
    pub(super) const fn is_definition_scoped(self) -> bool {
        matches!(
            self,
            Self::CapabilityUndeclared
                | Self::QualificationUnbound
                | Self::RevisionSelectionUnpublished
        )
    }

    const fn is_serving(self) -> bool {
        matches!(
            self,
            Self::ServingPortAbsent
                | Self::ServingSnapshotUnavailable
                | Self::ServingPortIssue
                | Self::ServingSnapshotNotCurrent
                | Self::ServingNotReady
        )
    }

    const fn allowed_in(self, section: Section) -> bool {
        self.is_serving()
            || match section {
                Section::Profile => self.is_definition_scoped(),
                Section::Instance => !self.is_definition_scoped(),
            }
    }

    fn fits(self, subject: &SubjectDto, routes: &[&str]) -> bool {
        match subject {
            SubjectDto::Route { name } => {
                matches!(
                    self,
                    Self::ServingSnapshotUnavailable
                        | Self::ServingPortIssue
                        | Self::ServingSnapshotNotCurrent
                        | Self::ServingNotReady
                ) && routes.contains(&name.as_str())
            }
            SubjectDto::Fact { field } => match self {
                Self::CapabilityUndeclared => matches!(field, Field::TaskFamilies | Field::Tools),
                Self::QualificationUnbound => *field == Field::QualificationRef,
                Self::RevisionSelectionUnpublished => *field == Field::Revisions,
                Self::ServingPortAbsent => *field == Field::ServingReadiness,
                Self::AwaitingDispatch | Self::AdmissionExpired | Self::DispatchOutcomeUnknown => {
                    *field == Field::Lifecycle
                }
                Self::CancellationUnobserved => *field == Field::Cancellation,
                Self::LivenessUnobserved | Self::LivenessStale => *field == Field::Liveness,
                Self::HostReachabilityUnknown => *field == Field::HostReachability,
                Self::AllocationLivenessUnknown => *field == Field::AllocationLiveness,
                Self::ServingSnapshotUnavailable
                | Self::ServingPortIssue
                | Self::ServingSnapshotNotCurrent
                | Self::ServingNotReady => false,
            },
        }
    }
}

impl Field {
    const fn as_str(self) -> &'static str {
        match self {
            Self::TaskFamilies => "task_families",
            Self::Tools => "tools",
            Self::QualificationRef => "qualification_ref",
            Self::Revisions => "revisions",
            Self::ServingReadiness => "serving_readiness",
            Self::Lifecycle => "lifecycle",
            Self::Cancellation => "cancellation",
            Self::Liveness => "liveness",
            Self::HostReachability => "host_reachability",
            Self::AllocationLiveness => "allocation_liveness",
        }
    }
}

impl ReasonDto {
    pub(super) fn names(&self, code: Code, field: Field) -> bool {
        self.code == code && self.subject == SubjectDto::Fact { field }
    }
}

/// Validate one section's reasons against the published vocabulary: the
/// owner fixed per code, the section, the subject, routes the same document
/// names, the bound and uniqueness.
pub(super) fn validate(
    reasons: &[ReasonDto],
    section: Section,
    routes: &[&str],
) -> Result<(), &'static str> {
    if reasons.len() > MAX_LATTICE_WAIT_REASONS {
        return Err("lattice_wait_reason_bound");
    }
    for (index, reason) in reasons.iter().enumerate() {
        if reason.owner != reason.code.owner()
            || !reason.code.allowed_in(section)
            || !reason.code.fits(&reason.subject, routes)
            || reasons.iter().take(index).any(|earlier| earlier == reason)
        {
            return Err("lattice_wait_reason_inconsistent");
        }
    }
    Ok(())
}

/// The provider-neutral reason, with the exact code, subject and owner.
pub(super) fn map(reason: &ReasonDto) -> WaitReason {
    WaitReason {
        code: reason.code.as_str().to_owned(),
        subject: match &reason.subject {
            SubjectDto::Fact { field } => WaitSubject::Fact {
                field: field.as_str().to_owned(),
            },
            SubjectDto::Route { name } => WaitSubject::Route { name: name.clone() },
        },
        owner: reason.owner.clone(),
    }
}
