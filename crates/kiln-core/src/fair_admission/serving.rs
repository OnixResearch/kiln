// r[impl kiln.admission.serving]
//! Serving-owner backpressure from injected snapshots.
//!
//! A snapshot is the provider-neutral projection of one service entry of a
//! host-supplied availability snapshot (Animus #17 shape). It is evidence for
//! waiting only: it never reserves a slot, never replaces the owner's request
//! grant, never refunds an operation and never resets an attempt.
use super::{
    ADMISSION_VERSION, AdmissionLedger, AdmissionPolicy, AdmissionReason, ChargeKind, MAX_RECORDS,
    MAX_SERVING_REQUESTS, OwnerReservation, QueueItem, ReservationState, ServingBounds, err,
};
use crate::types::{ContentId, CoreResult, ProviderProfileId};
use serde::{Deserialize, Serialize};

/// Whether unknown owner counters can be left to the owner's request admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServingCapacityRule {
    /// Remaining slots must be known from a trusted origin.
    KnownRequired,
    /// Unknown counters pass to the owner, whose grant still decides.
    OwnerAdmissionDecides,
}

/// Opt-in serving gate for worker-request owners.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServingAdmissionPolicy {
    pub max_snapshot_age_ms: u64,
    pub capacity_rule: ServingCapacityRule,
}
impl ServingAdmissionPolicy {
    pub(super) fn validate(&self, policy: &AdmissionPolicy) -> CoreResult<()> {
        if self.max_snapshot_age_ms == 0 || self.max_snapshot_age_ms > policy.max_wait_ms {
            return Err(err(
                "admission_serving_policy",
                "serving snapshot age must be positive and within the queue wait bound",
            ));
        }
        Ok(())
    }
    pub(super) fn hash(&self, hasher: &mut blake3::Hasher) {
        hasher.update(b"kiln.fair-admission-serving.v1");
        hasher.update(&self.max_snapshot_age_ms.to_le_bytes());
        hasher.update(&[match self.capacity_rule {
            ServingCapacityRule::KnownRequired => 0,
            ServingCapacityRule::OwnerAdmissionDecides => 1,
        }]);
    }
}

/// Readiness as reported by the snapshot source.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServingReadiness {
    Absent,
    Unknown,
    Loading,
    Ready,
    Draining,
    Unavailable,
    Stale,
}

/// Who produced the counters. A host probe is never capacity evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServingOrigin {
    ServingOwner,
    ProviderApi,
    HostProbe,
}

/// One owner aggregate scope at one source generation and sequence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServingObservation {
    pub version: u16,
    /// Serving owner profile; the same profile named by worker requests.
    pub provider: ProviderProfileId,
    /// Owner aggregate scope across callers, never a worker-local queue.
    pub scope: ContentId,
    pub source_generation: u64,
    pub sequence: u64,
    /// Must equal the request's owner generation to gate that request.
    pub deployment_generation: u64,
    pub readiness: ServingReadiness,
    pub origin: ServingOrigin,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
    pub bounds: ServingBounds,
    /// Remaining ongoing-request slots; `None` is unknown, never zero.
    pub remaining_slots: Option<u32>,
    /// Requests waiting in the owner's aggregate queue; `None` is unknown.
    pub queued: Option<u32>,
}

/// Display-only serving state for one queued worker-request owner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServingExplanation {
    pub provider: ProviderProfileId,
    pub gate: AdmissionReason,
    pub observation: Option<ServingObservation>,
}

impl AdmissionLedger {
    /// Retain one snapshot per scope. Order cannot regress, a scope cannot
    /// change owner, and the same order with different content is ambiguous.
    pub fn observe_serving(&mut self, observation: ServingObservation) -> CoreResult<()> {
        self.policy()?;
        let bounds = &observation.bounds;
        if observation.version != ADMISSION_VERSION
            || observation.source_generation == 0
            || observation.sequence == 0
            || observation.deployment_generation == 0
            || observation.observed_at_ms >= observation.valid_until_ms
            || bounds.max_ongoing == 0
            || bounds.max_ongoing > MAX_SERVING_REQUESTS
            || bounds.max_queued > MAX_SERVING_REQUESTS
            || bounds.scope != observation.scope
            || observation
                .remaining_slots
                .is_some_and(|slots| slots > bounds.max_ongoing)
            || observation
                .queued
                .is_some_and(|queued| queued > bounds.max_queued)
        {
            return Err(err(
                "admission_serving_snapshot",
                "serving snapshot needs positive order, a finite window and consistent aggregate bounds",
            ));
        }
        if let Some(prior) = self.serving.get(&observation.scope) {
            if prior.provider != observation.provider {
                return Err(err(
                    "admission_serving_owner",
                    "serving scope cannot change owner",
                ));
            }
            let order = (observation.source_generation, observation.sequence);
            let prior_order = (prior.source_generation, prior.sequence);
            if order < prior_order {
                return Err(err(
                    "admission_serving_order",
                    "serving snapshot is out of order",
                ));
            }
            if order == prior_order {
                return if prior == &observation {
                    Ok(())
                } else {
                    Err(err(
                        "admission_serving_ambiguous",
                        "same snapshot order cannot change content",
                    ))
                };
            }
        } else if self.serving.len() >= MAX_RECORDS {
            return Err(err("admission_serving_full", "serving scope history full"));
        }
        self.serving.insert(observation.scope.clone(), observation);
        Ok(())
    }

    /// Provider-level gate used before claim. `None` when the policy is off.
    pub(super) fn provider_serving_gate(
        &self,
        provider: &ProviderProfileId,
        now_ms: u64,
    ) -> Option<AdmissionReason> {
        Some(self.best_provider_observation(provider, now_ms)?.0)
    }

    /// Exact scope/generation/bounds gate before a tentative worker request.
    /// `None` when the serving policy is off. Never a reservation.
    pub fn request_serving_gate(
        &self,
        reservation: &OwnerReservation,
        now_ms: u64,
    ) -> Option<AdmissionReason> {
        let policy = self.policy.as_ref()?.serving.as_ref()?;
        let Some(bounds) = reservation.serving_bounds.as_ref() else {
            return Some(AdmissionReason::Incompatible);
        };
        let Some(observation) = self.serving.get(&bounds.scope) else {
            return Some(AdmissionReason::Incomplete);
        };
        if observation.provider != reservation.provider || &observation.bounds != bounds {
            return Some(AdmissionReason::Incompatible);
        }
        if observation.deployment_generation != reservation.generation {
            return Some(AdmissionReason::Stale);
        }
        Some(gate(policy, observation, now_ms))
    }

    pub(super) fn serving_explanation(
        &self,
        item: &QueueItem,
        now_ms: u64,
    ) -> Option<ServingExplanation> {
        let (provider, _) = item
            .required_owners
            .iter()
            .find(|(_, kind)| **kind == ChargeKind::WorkerRequest)?;
        let (gate, observation) = self.best_provider_observation(provider, now_ms)?;
        let retained = self.reservations.values().any(|r| {
            r.work_version == item.work_version
                && &r.provider == provider
                && matches!(
                    r.state,
                    ReservationState::Tentative
                        | ReservationState::Committed
                        | ReservationState::Unknown
                )
        });
        Some(ServingExplanation {
            provider: provider.clone(),
            // A retained owner operation is decided by the owner, not the snapshot.
            gate: if retained {
                AdmissionReason::Eligible
            } else {
                gate
            },
            observation: observation.cloned(),
        })
    }

    fn best_provider_observation(
        &self,
        provider: &ProviderProfileId,
        now_ms: u64,
    ) -> Option<(AdmissionReason, Option<&ServingObservation>)> {
        let policy = self.policy.as_ref()?.serving.as_ref()?;
        let mut best: (AdmissionReason, Option<&ServingObservation>) =
            (AdmissionReason::Incomplete, None);
        for observation in self.serving.values().filter(|o| &o.provider == provider) {
            let candidate = gate(policy, observation, now_ms);
            if best.1.is_none() || preference(candidate) < preference(best.0) {
                best = (candidate, Some(observation));
            }
        }
        Some(best)
    }
}

/// Lower is more permissive/informative for the provider-level summary.
fn preference(reason: AdmissionReason) -> u8 {
    match reason {
        AdmissionReason::Eligible => 0,
        AdmissionReason::Overloaded => 1,
        AdmissionReason::Waiting => 2,
        AdmissionReason::Stale => 3,
        AdmissionReason::Incomplete => 4,
        _ => 5,
    }
}

fn gate(
    policy: &ServingAdmissionPolicy,
    observation: &ServingObservation,
    now_ms: u64,
) -> AdmissionReason {
    if observation.readiness == ServingReadiness::Stale
        || observation.observed_at_ms > now_ms
        || now_ms - observation.observed_at_ms > policy.max_snapshot_age_ms
        || now_ms >= observation.valid_until_ms
    {
        return AdmissionReason::Stale;
    }
    match observation.readiness {
        ServingReadiness::Ready => {}
        ServingReadiness::Absent => return AdmissionReason::Incompatible,
        _ => return AdmissionReason::Waiting,
    }
    let trusted = observation.origin != ServingOrigin::HostProbe;
    let slots = observation.remaining_slots.filter(|_| trusted);
    let queued = observation.queued.filter(|_| trusted);
    match (slots, queued) {
        (Some(slots), _) if slots > 0 => AdmissionReason::Eligible,
        (Some(_), Some(queued)) if queued >= observation.bounds.max_queued => {
            AdmissionReason::Overloaded
        }
        // No ongoing slot but owner queue room: the owner may queue the request.
        (Some(_), Some(_)) => AdmissionReason::Eligible,
        _ => match policy.capacity_rule {
            ServingCapacityRule::KnownRequired => AdmissionReason::Incomplete,
            ServingCapacityRule::OwnerAdmissionDecides => AdmissionReason::Eligible,
        },
    }
}
