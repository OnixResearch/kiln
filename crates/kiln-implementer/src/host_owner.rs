// r[impl kiln.admission.host_owner]
//! Shell for one explicitly registered host-local resource owner.
//!
//! The pure [`HostOwnerProfile`] decides every record and reply; this shell
//! reads the clock-free ledger snapshot and persists each decision through the
//! existing [`AdmissionCommand`] transitions in the implementation store. No
//! second queue, scheduler or capacity ledger exists. The owner answers through
//! the same [`AdmissionOwnerPortV1`] contract dispatch later checks, reading the
//! ledger only after the tentative request is durable, so a crash or concurrent
//! request is reconciled rather than duplicated.
use crate::Implementer;
use crate::admission::{AdmissionCommand, AdmissionOwnerPortV1, AdmissionResult, OwnerPortFailure};
use kiln_core::fair_admission::{
    AdmissionLedger, AdmissionReason, ClaimRef, EligibilityInput, HostOwnerDecision,
    HostOwnerProfile, HostReleasePlan, HostReservationPlan, OwnerReply, OwnerReservation,
    ReservationState,
};
use kiln_core::types::{ContentId, Diagnostic};
use serde::Serialize;

/// Records `host-owner-admit` applied or found already applied.
#[derive(Clone, Debug, Serialize)]
pub struct HostOwnerAdmitted {
    pub policy: ContentId,
    pub account: ContentId,
    pub queued: Vec<HostQueuedWork>,
    /// Current ready work this registration does not admit; never queued.
    pub refused: Vec<HostRefusedWork>,
}
#[derive(Clone, Debug, Serialize)]
pub struct HostQueuedWork {
    pub work: ContentId,
    pub repository: String,
    pub class: String,
    pub reason: AdmissionReason,
}
#[derive(Clone, Debug, Serialize)]
pub struct HostRefusedWork {
    pub work: ContentId,
    pub repository: String,
    pub cause: String,
}
/// One owner observation retained by `host-owner-observe`.
#[derive(Clone, Debug, Serialize)]
pub struct HostOwnerObserved {
    pub usage_generation: u64,
    pub observations: Vec<EligibilityInput>,
    pub selected: Option<ContentId>,
}
/// Outcome of `host-owner-reserve`.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum HostOwnerReserved {
    /// The owner committed this request now.
    Committed { reservation: Box<OwnerReservation> },
    /// The exact operation was already retained; replay made no new request.
    Retained { reservation: Box<OwnerReservation> },
}
/// Outcome of `host-owner-release`.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum HostOwnerReleased {
    Settled { reservation: Box<OwnerReservation> },
    AlreadySettled { reservation: Box<OwnerReservation> },
}

/// The host-local owner port. It decides from the ledger read after the
/// request was persisted, so every other retained request occupies capacity.
#[derive(Debug)]
pub struct HostLocalOwner<'a> {
    profile: &'a HostOwnerProfile,
    ledger: AdmissionLedger,
    now_ms: u64,
}
impl<'a> HostLocalOwner<'a> {
    #[must_use]
    pub fn new(profile: &'a HostOwnerProfile, ledger: AdmissionLedger, now_ms: u64) -> Self {
        Self {
            profile,
            ledger,
            now_ms,
        }
    }
}
impl AdmissionOwnerPortV1 for HostLocalOwner<'_> {
    fn reserve(&mut self, request: &OwnerReservation) -> Result<OwnerReply, OwnerPortFailure> {
        match self.profile.decide(&self.ledger, request, self.now_ms) {
            Ok(HostOwnerDecision::Committed(reply)) => Ok(reply),
            Ok(HostOwnerDecision::OverCapacity) => {
                Err(OwnerPortFailure::Waiting { retry_at_ms: None })
            }
            Ok(HostOwnerDecision::Expired) => Err(OwnerPortFailure::ExpiredBeforeDispatch),
            Ok(HostOwnerDecision::Mismatch) | Err(_) => {
                Err(OwnerPortFailure::RejectedBeforeDispatch)
            }
        }
    }
    fn reconcile(&mut self, request: &OwnerReservation) -> Result<OwnerReply, OwnerPortFailure> {
        // The host decides from durable ledger facts alone, so reconciling a
        // retained tentative request is the same decision as the first answer.
        self.reserve(request)
    }
    fn cancel(&mut self, _request: &OwnerReservation) -> Result<OwnerReply, OwnerPortFailure> {
        // Cancellation never proves the slot unused; the debit stays held.
        Err(OwnerPortFailure::CancellationObserved)
    }
}

impl Implementer {
    /// Configure the registration's policy and root account and queue every
    /// current ready work it admits. Replay re-admits identical records and
    /// never resets quota; a changed registration is refused by the ledger.
    pub fn host_owner_admit(
        &mut self,
        profile: &HostOwnerProfile,
        now_ms: u64,
    ) -> Result<HostOwnerAdmitted, String> {
        profile.validate().map_err(diagnostic)?;
        if let Some(entry) = profile
            .repositories
            .iter()
            .find(|entry| !self.enrollment.repositories.contains(&entry.repository))
        {
            return Err(format!(
                "host owner: registered repository {} is not enrolled",
                entry.repository
            ));
        }
        let policy = match self.admission_apply(
            AdmissionCommand::Configure {
                policy: profile.policy().map_err(diagnostic)?,
            },
            now_ms,
        )? {
            AdmissionResult::Policy { identity } => identity,
            other => return Err(format!("host owner: unexpected policy result {other:?}")),
        };
        let account = profile.root_account().map_err(diagnostic)?;
        let identity = account.identity.clone();
        self.admission_apply(AdmissionCommand::Root { account }, now_ms)?;
        let ledger = self.work()?;
        let mut queued = Vec::new();
        let mut refused = Vec::new();
        for row in ledger.ranked() {
            let Some(work) = ledger
                .current()
                .find(|work| work.identity().is_ok_and(|id| id == row.identity))
            else {
                continue;
            };
            if !self.enrollment.repositories.contains(&work.repository)
                || !self.enrollment.actors.contains(&work.actor)
            {
                continue;
            }
            let class = match profile.work_class(work) {
                Ok(class) => class.name.as_str().to_owned(),
                Err(error) => {
                    refused.push(HostRefusedWork {
                        work: row.identity.clone(),
                        repository: work.repository.clone(),
                        cause: diagnostic(error),
                    });
                    continue;
                }
            };
            let item = profile.queue_item(work, now_ms).map_err(diagnostic)?;
            let reason = match self.admission_apply(AdmissionCommand::Enqueue { item }, now_ms)? {
                AdmissionResult::Queued { reason } => reason,
                other => return Err(format!("host owner: unexpected queue result {other:?}")),
            };
            queued.push(HostQueuedWork {
                work: row.identity.clone(),
                repository: work.repository.clone(),
                class,
                reason,
            });
        }
        Ok(HostOwnerAdmitted {
            policy,
            account: identity,
            queued,
            refused,
        })
    }

    /// Retain this owner's current usage and one fresh eligibility observation
    /// per open queued work, then run the existing selection over them.
    pub fn host_owner_observe(
        &mut self,
        profile: &HostOwnerProfile,
        now_ms: u64,
    ) -> Result<HostOwnerObserved, String> {
        let ledger = self.host_owner_ledger(profile)?;
        let observation = profile
            .usage_observation(&ledger, now_ms)
            .map_err(diagnostic)?;
        let usage_generation = observation.generation;
        self.admission_apply(AdmissionCommand::Observe { observation }, now_ms)?;
        let observations = profile.eligibility(&ledger, now_ms).map_err(diagnostic)?;
        let selected = match self.admission_apply(
            AdmissionCommand::Select {
                inputs: observations.clone(),
            },
            now_ms,
        )? {
            AdmissionResult::Selected { work_version } => work_version,
            other => return Err(format!("host owner: unexpected selection {other:?}")),
        };
        Ok(HostOwnerObserved {
            usage_generation,
            observations,
            selected,
        })
    }

    /// Commit this owner's reservation for one exact current claim through the
    /// owner port. Replay returns the retained operation without a new request.
    pub fn host_owner_reserve(
        &mut self,
        profile: &HostOwnerProfile,
        attempt: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<HostOwnerReserved, String> {
        let ledger = self.host_owner_ledger(profile)?;
        let attempts = self.attempts()?;
        let current = attempts
            .get(attempt)
            .filter(|current| current.fence == fence)
            .ok_or("host owner: attempt/fence is not retained")?;
        let work = self
            .work()?
            .current()
            .find(|work| work.identity().is_ok_and(|id| id == current.work_identity))
            .cloned()
            .ok_or("host owner: attempt work is not current")?;
        let claim = ClaimRef {
            attempt: attempt.clone(),
            fence,
        };
        let request = match profile
            .plan_reservation(&ledger, &work, &claim, current.lease_until_ms, now_ms)
            .map_err(diagnostic)?
        {
            HostReservationPlan::Retained(retained)
                if retained.state != ReservationState::Tentative =>
            {
                return Ok(HostOwnerReserved::Retained {
                    reservation: Box::new(retained),
                });
            }
            HostReservationPlan::Retained(retained) => retained,
            HostReservationPlan::Request(request) => {
                match self.admission_apply(
                    AdmissionCommand::Request {
                        reservation: request.clone(),
                        inputs: Vec::new(),
                    },
                    now_ms,
                )? {
                    AdmissionResult::Updated | AdmissionResult::Retained { .. } => request,
                    AdmissionResult::Owner { reason } => {
                        return Err(format!(
                            "host owner: admission holds the request as {reason:?}"
                        ));
                    }
                    other => {
                        return Err(format!("host owner: unexpected request result {other:?}"));
                    }
                }
            }
        };
        let snapshot = self.admission_snapshot()?;
        let mut owner = HostLocalOwner::new(profile, snapshot, now_ms);
        let answer = owner.reserve(&request);
        let result = self.admission_record_port_result(&request, answer, now_ms)?;
        let retained = self
            .admission_snapshot()?
            .reservations
            .get(&request.operation)
            .cloned()
            .ok_or("host owner: requested operation disappeared")?;
        if retained.state != ReservationState::Committed {
            return Err(format!(
                "host owner: reservation not committed ({:?}): {result:?}",
                retained.state
            ));
        }
        Ok(HostOwnerReserved::Committed {
            reservation: Box::new(retained),
        })
    }

    /// Settle one claim's reservation once the attempt ledger records the
    /// holder terminal and resolved. Without that observation nothing changes.
    pub fn host_owner_release(
        &mut self,
        profile: &HostOwnerProfile,
        attempt: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<HostOwnerReleased, String> {
        let ledger = self.host_owner_ledger(profile)?;
        let attempts = self.attempts()?;
        let claim = ClaimRef {
            attempt: attempt.clone(),
            fence,
        };
        let operation = match profile
            .plan_release(&ledger, &attempts, &claim)
            .map_err(diagnostic)?
        {
            HostReleasePlan::Settled(reservation) => {
                return Ok(HostOwnerReleased::AlreadySettled {
                    reservation: Box::new(reservation),
                });
            }
            HostReleasePlan::Settle {
                operation,
                provider,
                generation,
                receipt,
                consumed,
            } => {
                self.admission_apply(
                    AdmissionCommand::Settle {
                        operation: operation.clone(),
                        provider,
                        generation,
                        receipt,
                        consumed,
                    },
                    now_ms,
                )?;
                operation
            }
        };
        let reservation = self
            .admission_snapshot()?
            .reservations
            .get(&operation)
            .cloned()
            .ok_or("host owner: settled operation disappeared")?;
        Ok(HostOwnerReleased::Settled {
            reservation: Box::new(reservation),
        })
    }

    /// The ledger, refusing one configured by another policy.
    fn host_owner_ledger(&mut self, profile: &HostOwnerProfile) -> Result<AdmissionLedger, String> {
        profile.validate().map_err(diagnostic)?;
        let ledger = self.admission_snapshot()?;
        if ledger.policy.as_ref() != Some(&profile.policy().map_err(diagnostic)?) {
            return Err(
                "host owner: ledger policy is not this registration's admitted policy; run host-owner-admit"
                    .into(),
            );
        }
        Ok(ledger)
    }
}

fn diagnostic(error: Diagnostic) -> String {
    format!("{}: {}", error.code, error.message)
}
