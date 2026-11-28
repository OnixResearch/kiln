//! Attempt grant issuance intents and their pure reconciliation fold (#70).
//!
//! The grant-lifecycle shell persists [`GrantIssuanceRecord`]s to durable
//! SQLite authority state: `Intended` in the same logical admission flow as the
//! #61 `Transfer` reservation and before any token exists, `Issued` once the
//! minted token's proof reference is known, `Revoked` with the revocation
//! generation, `UnknownTerminated` / `UnknownResolved` for unknown-termination
//! cleanup reconciliation, `Released` on clean completion, and `Abandoned` when
//! an unissued intent is invalidated before minting.
//!
//! Folding records performs nothing, so replaying saved state issues no grant.
//! [`GrantIssuanceState::reconcile`] lists what a live shell still owes, each
//! under the original intent identity: a crash between intent and `Issued`
//! resumes that intent rather than deriving a second one, and a revoked grant
//! propagates to its issued descendants.

use std::collections::BTreeMap;

use kiln_core::implementation::authority::AUTHORITY_DIGEST_BYTES;
use kiln_core::implementation::grant::{
    AdmittedGrant, AttemptAuthorityBinding, AttemptLineage, ChildConcurrencyLimit,
    ChildConcurrencyState, ChildGrantError, ChildGrantRequest, DelegationFacts,
    GRANT_ISSUANCE_EXPLANATION_SCHEMA, GrantLifecyclePhase, GrantRole, MAX_GRANT_LINEAGE,
    RedactedGrantExplanation, admit_child_with_concurrency, format_grant_ref_digest,
    sanitize_redacted_denial_code,
};
use kiln_core::implementation::{AttemptLedger, FailureClass};
use kiln_core::types::{ContentId, CoreResult, Diagnostic, ExternalRef};
use serde::{Deserialize, Serialize};

/// Journal kind schema of grant issuance records.
pub const GRANT_ISSUANCE_SCHEMA: &str = "kiln.attempt-grant-issuance.v1";
/// Maximum issuance intents one fold retains.
pub const MAX_GRANT_ISSUANCE_INTENTS: usize = 4_096;
/// Host-clock milliseconds per grant-lifetime second.
const MS_PER_SECOND: u64 = 1_000;

/// One proof reference: BLAKE3 of a compact token, never the token.
pub type GrantRef = [u8; AUTHORITY_DIGEST_BYTES];

/// One journaled grant issuance fact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case", deny_unknown_fields)]
pub enum GrantIssuanceRecord {
    /// The admitted grant, persisted with its #61 reservation before minting.
    Intended { grant: Box<AdmittedGrant> },
    /// The minted token's proof reference for one intent.
    Issued {
        intent: ContentId,
        grant_ref: GrantRef,
    },
    /// A durable revocation of one proof reference.
    Revoked {
        grant_ref: GrantRef,
        revocation_generation: u64,
    },
    /// An unissued intent was invalidated and abandoned before minting.
    Abandoned { intent: ContentId },
    /// An issued grant cleanly completed/settled and released its active slot.
    Released { intent: ContentId },
    /// Attempt or child terminated in `Unknown`: grant is fenced/revoked while
    /// its held child capacity slot remains occupied until independently resolved.
    UnknownTerminated {
        intent: ContentId,
        revocation_generation: u64,
    },
    /// Unknown termination was independently observed and reconciled: grant
    /// stays revoked/fenced, and its held capacity slot is now released.
    UnknownResolved {
        intent: ContentId,
        termination: ExternalRef,
    },
}

/// What a live shell still owes for one intent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum GrantIssuanceAction {
    /// Mint exactly once under this intent. The shell first looks for a token
    /// already minted for it and records that one instead of minting again.
    Issue { intent: ContentId },
    /// Durably revoke an issued grant whose ancestor is revoked.
    Revoke {
        intent: ContentId,
        grant_ref: GrantRef,
    },
    /// Never mint: an ancestor is revoked or the grant expired before issuance.
    Abandon { intent: ContentId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum IntentLifecycle {
    Pending,
    Active,
    Abandoned,
    Released,
    UnknownFenced {
        revocation_generation: u64,
    },
    UnknownResolved {
        revocation_generation: u64,
        termination: ExternalRef,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IntentState {
    grant: AdmittedGrant,
    issued: Option<GrantRef>,
    lifecycle: IntentLifecycle,
}

// r[impl kiln.attempt.grant_lineage.issuance]
/// Folded issuance state. Pure: no clock, store or token.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GrantIssuanceState {
    intents: BTreeMap<ContentId, IntentState>,
    /// Budget account to the parent budget it was reserved from.
    accounts: BTreeMap<ContentId, ContentId>,
    by_grant_ref: BTreeMap<GrantRef, ContentId>,
    /// Issued binding identity to its intent.
    by_binding: BTreeMap<ContentId, ContentId>,
    revoked: BTreeMap<GrantRef, u64>,
}

impl GrantIssuanceState {
    /// Fold journaled records in order.
    ///
    /// # Errors
    /// Returns the first record's refusal.
    pub fn replay<'a>(
        records: impl IntoIterator<Item = &'a GrantIssuanceRecord>,
    ) -> CoreResult<Self> {
        let mut state = Self::default();
        for record in records {
            state.apply(record)?;
        }
        Ok(state)
    }

    /// Fold one record. Repeating a record is idempotent.
    ///
    /// # Errors
    /// Refuses an intent that does not recompute, exceeds the fold bound or
    /// reserves an account from another parent budget; an issuance for an
    /// unknown intent, a second proof for one intent, or one proof for two
    /// intents.
    pub fn apply(&mut self, record: &GrantIssuanceRecord) -> CoreResult<()> {
        match record {
            GrantIssuanceRecord::Intended { grant } => self.intend(grant),
            GrantIssuanceRecord::Issued { intent, grant_ref } => self.issue(intent, grant_ref),
            GrantIssuanceRecord::Revoked {
                grant_ref,
                revocation_generation,
            } => {
                let generation = self.revoked.entry(*grant_ref).or_default();
                *generation = (*generation).max(*revocation_generation);
                Ok(())
            }
            GrantIssuanceRecord::Abandoned { intent } => self.abandon(intent),
            GrantIssuanceRecord::Released { intent } => self.release(intent),
            GrantIssuanceRecord::UnknownTerminated {
                intent,
                revocation_generation,
            } => self.unknown_terminate(intent, *revocation_generation),
            GrantIssuanceRecord::UnknownResolved {
                intent,
                termination,
            } => self.unknown_resolve(intent, termination),
        }
    }

    fn intend(&mut self, grant: &AdmittedGrant) -> CoreResult<()> {
        grant.validate()?;
        if let Some(existing) = self.intents.get(&grant.intent) {
            if &existing.grant == grant {
                return Ok(());
            }
            return Err(refusal(
                "grant_issuance_intent_conflict",
                "grant.intent",
                "an existing grant intent differs from the supplied grant",
            ));
        }
        if self.intents.len() >= MAX_GRANT_ISSUANCE_INTENTS {
            return Err(refusal(
                "grant_issuance_limit",
                "grant_issuance.intents",
                "grant issuance intents exceed the fold bound",
            ));
        }
        if self
            .accounts
            .get(&grant.budget.account)
            .is_some_and(|parent| *parent != grant.budget.parent_budget_identity)
        {
            return Err(refusal(
                "grant_issuance_budget_conflict",
                "grant.budget",
                "one budget account cannot be reserved from two parent budgets",
            ));
        }
        self.accounts.insert(
            grant.budget.account.clone(),
            grant.budget.parent_budget_identity.clone(),
        );
        self.intents.insert(
            grant.intent.clone(),
            IntentState {
                grant: grant.clone(),
                issued: None,
                lifecycle: IntentLifecycle::Pending,
            },
        );
        Ok(())
    }

    fn issue(&mut self, intent: &ContentId, grant_ref: &GrantRef) -> CoreResult<()> {
        if self
            .by_grant_ref
            .get(grant_ref)
            .is_some_and(|owner| owner != intent)
        {
            return Err(refusal(
                "grant_issuance_reused_proof",
                "grant_issuance.grant_ref",
                "one proof reference cannot issue two intents",
            ));
        }
        let Some(existing_grant) = self.intents.get(intent).map(|s| s.grant.clone()) else {
            return Err(refusal(
                "grant_issuance_unknown_intent",
                "grant_issuance.intent",
                "issuance names no recorded intent",
            ));
        };
        let ancestor_revoked = self.ancestor_revoked(&existing_grant);
        let state = self.intents.get_mut(intent).ok_or_else(|| {
            refusal(
                "grant_issuance_unknown_intent",
                "grant_issuance.intent",
                "issuance names no recorded intent",
            )
        })?;
        match state.issued {
            Some(recorded) if recorded == *grant_ref => return Ok(()),
            Some(_) => {
                return Err(refusal(
                    "grant_issuance_duplicate",
                    "grant_issuance.grant_ref",
                    "an intent is issued at most once",
                ));
            }
            None => {}
        }
        if state.lifecycle != IntentLifecycle::Pending {
            return Err(refusal(
                "grant_issuance_fenced",
                "grant_issuance.intent",
                "abandoned, released, or unknown-terminated intent cannot be issued",
            ));
        }
        if ancestor_revoked || self.revoked.contains_key(grant_ref) {
            return Err(refusal(
                "grant_issuance_revoked",
                "grant_issuance.grant_ref",
                "cannot issue under a revoked grant or ancestor",
            ));
        }
        let binding = state.grant.binding_identity(grant_ref)?;
        state.issued = Some(*grant_ref);
        state.lifecycle = IntentLifecycle::Active;
        self.by_grant_ref.insert(*grant_ref, intent.clone());
        self.by_binding.insert(binding, intent.clone());
        Ok(())
    }

    fn abandon(&mut self, intent: &ContentId) -> CoreResult<()> {
        let state = self.intents.get_mut(intent).ok_or_else(|| {
            refusal(
                "grant_issuance_unknown_intent",
                "grant_issuance.intent",
                "abandon names no recorded intent",
            )
        })?;
        match state.lifecycle {
            IntentLifecycle::Abandoned => Ok(()),
            IntentLifecycle::Pending if state.issued.is_none() => {
                state.lifecycle = IntentLifecycle::Abandoned;
                Ok(())
            }
            _ => Err(refusal(
                "grant_issuance_abandon_invalid",
                "grant_issuance.intent",
                "only an unissued pending intent can be abandoned",
            )),
        }
    }

    fn release(&mut self, intent: &ContentId) -> CoreResult<()> {
        let state = self.intents.get_mut(intent).ok_or_else(|| {
            refusal(
                "grant_issuance_unknown_intent",
                "grant_issuance.intent",
                "release names no recorded intent",
            )
        })?;
        match state.lifecycle {
            IntentLifecycle::Released => Ok(()),
            IntentLifecycle::Active if state.issued.is_some() => {
                state.lifecycle = IntentLifecycle::Released;
                Ok(())
            }
            IntentLifecycle::UnknownFenced { .. } | IntentLifecycle::UnknownResolved { .. } => {
                Err(refusal(
                    "grant_issuance_unknown_reconciliation_required",
                    "grant_issuance.intent",
                    "unknown termination must reconcile independently before releasing",
                ))
            }
            _ => Err(refusal(
                "grant_issuance_release_invalid",
                "grant_issuance.intent",
                "only an issued active intent can be cleanly released",
            )),
        }
    }

    fn unknown_terminate(
        &mut self,
        intent: &ContentId,
        revocation_generation: u64,
    ) -> CoreResult<()> {
        let state = self.intents.get_mut(intent).ok_or_else(|| {
            refusal(
                "grant_issuance_unknown_intent",
                "grant_issuance.intent",
                "unknown termination names no recorded intent",
            )
        })?;
        match &mut state.lifecycle {
            IntentLifecycle::Released | IntentLifecycle::Abandoned => {
                return Err(refusal(
                    "grant_issuance_terminal_conflict",
                    "grant_issuance.intent",
                    "released or abandoned intent cannot enter unknown termination",
                ));
            }
            IntentLifecycle::UnknownResolved { .. } => {
                return Err(refusal(
                    "grant_issuance_terminal_conflict",
                    "grant_issuance.intent",
                    "resolved unknown termination cannot revert to fenced",
                ));
            }
            IntentLifecycle::UnknownFenced {
                revocation_generation: existing_gen,
            } => {
                *existing_gen = (*existing_gen).max(revocation_generation);
            }
            IntentLifecycle::Pending | IntentLifecycle::Active => {
                state.lifecycle = IntentLifecycle::UnknownFenced {
                    revocation_generation,
                };
            }
        }
        if let Some(grant_ref) = state.issued {
            let generation = self.revoked.entry(grant_ref).or_default();
            *generation = (*generation).max(revocation_generation);
        }
        Ok(())
    }

    fn unknown_resolve(&mut self, intent: &ContentId, termination: &ExternalRef) -> CoreResult<()> {
        termination.validate()?;
        let state = self.intents.get_mut(intent).ok_or_else(|| {
            refusal(
                "grant_issuance_unknown_intent",
                "grant_issuance.intent",
                "unknown resolution names no recorded intent",
            )
        })?;
        match &state.lifecycle {
            IntentLifecycle::UnknownResolved {
                termination: existing,
                ..
            } => {
                if existing == termination {
                    return Ok(());
                }
                Err(refusal(
                    "grant_issuance_reconciliation_conflict",
                    "grant_issuance.termination",
                    "a different unknown termination observation is already retained",
                ))
            }
            IntentLifecycle::UnknownFenced {
                revocation_generation,
            } => {
                let rev_gen = *revocation_generation;
                state.lifecycle = IntentLifecycle::UnknownResolved {
                    revocation_generation: rev_gen,
                    termination: termination.clone(),
                };
                Ok(())
            }
            _ => Err(refusal(
                "grant_issuance_reconciliation_binding",
                "grant_issuance.intent",
                "unknown resolution requires an unknown-terminated grant",
            )),
        }
    }

    /// Recorded proof reference of one intent, once issued.
    #[must_use]
    pub fn issued(&self, intent: &ContentId) -> Option<&GrantRef> {
        self.intents
            .get(intent)
            .and_then(|state| state.issued.as_ref())
    }

    /// Whether a proof reference is durably revoked.
    #[must_use]
    pub fn is_revoked(&self, grant_ref: &GrantRef) -> bool {
        self.revoked.contains_key(grant_ref)
    }

    /// Recorded revocation generation of a proof reference, if revoked.
    #[must_use]
    pub fn revocation_generation_of(&self, grant_ref: &GrantRef) -> Option<u64> {
        self.revoked.get(grant_ref).copied()
    }

    /// Highest revocation generation recorded across all revoked grants.
    #[must_use]
    pub fn max_revocation_generation(&self) -> u64 {
        self.revoked
            .values()
            .copied()
            .chain(self.intents.values().filter_map(|s| match s.lifecycle {
                IntentLifecycle::UnknownFenced {
                    revocation_generation,
                }
                | IntentLifecycle::UnknownResolved {
                    revocation_generation,
                    ..
                } => Some(revocation_generation),
                _ => None,
            }))
            .max()
            .unwrap_or(0)
    }

    /// Admitted grant intent record.
    #[must_use]
    pub fn grant(&self, intent: &ContentId) -> Option<&AdmittedGrant> {
        self.intents.get(intent).map(|state| &state.grant)
    }

    /// Reconstruct the [`AttemptAuthorityBinding`] for an issued intent.
    #[must_use]
    pub fn binding(&self, intent: &ContentId) -> Option<AttemptAuthorityBinding> {
        let state = self.intents.get(intent)?;
        let grant_ref = state.issued?;
        state.grant.clone().issue(grant_ref).ok()
    }

    /// Reconstruct the [`AttemptAuthorityBinding`] by its binding identity.
    #[must_use]
    pub fn binding_by_id(&self, binding_id: &ContentId) -> Option<AttemptAuthorityBinding> {
        let intent = self.by_binding.get(binding_id)?;
        self.binding(intent)
    }

    /// Collect root-first ancestors for `parent_binding_id`, excluding the
    /// binding itself? No — `DelegationFacts::ancestors` lists the parent's
    /// ancestors root-first excluding `parent`!
    ///
    /// # Errors
    /// Returns a diagnostic if a referenced ancestor binding is missing or exceeds
    /// [`MAX_GRANT_LINEAGE`].
    pub fn ancestors_of(
        &self,
        parent_binding_id: &ContentId,
    ) -> CoreResult<Vec<AttemptAuthorityBinding>> {
        let Some(parent_intent) = self.by_binding.get(parent_binding_id) else {
            return Err(refusal(
                "grant_issuance_unknown_parent",
                "grant.parent",
                "parent binding is not issued in this journal",
            ));
        };
        let mut chain = Vec::new();
        let mut cursor = self
            .intents
            .get(parent_intent)
            .and_then(|state| state.grant.parent.as_ref());
        for _ in 0..MAX_GRANT_LINEAGE {
            let Some(binding_id) = cursor else {
                chain.reverse();
                return Ok(chain);
            };
            let ancestor = self.binding_by_id(binding_id).ok_or_else(|| {
                refusal(
                    "grant_issuance_broken_lineage",
                    "grant.parent",
                    "ancestor binding is missing from this journal",
                )
            })?;
            cursor = self
                .by_binding
                .get(binding_id)
                .and_then(|intent| self.intents.get(intent))
                .and_then(|state| state.grant.parent.as_ref());
            chain.push(ancestor);
        }
        Err(refusal(
            "grant_issuance_lineage_depth",
            "grant.parent",
            "grant lineage exceeds maximum depth",
        ))
    }

    /// Lifecycle phase of an intent.
    #[must_use]
    pub fn lifecycle_phase(&self, intent: &ContentId) -> GrantLifecyclePhase {
        let Some(state) = self.intents.get(intent) else {
            return GrantLifecyclePhase::None;
        };
        match state.lifecycle {
            IntentLifecycle::UnknownFenced { .. } => GrantLifecyclePhase::UnknownFenced,
            IntentLifecycle::UnknownResolved { .. } => GrantLifecyclePhase::UnknownResolved,
            IntentLifecycle::Abandoned => GrantLifecyclePhase::Abandoned,
            IntentLifecycle::Released => GrantLifecyclePhase::Released,
            IntentLifecycle::Active => {
                if state
                    .issued
                    .is_some_and(|grant_ref| self.is_revoked(&grant_ref))
                    || self.ancestor_revoked(&state.grant)
                {
                    GrantLifecyclePhase::Revoked
                } else {
                    GrantLifecyclePhase::Issued
                }
            }
            IntentLifecycle::Pending => {
                if self.ancestor_revoked(&state.grant) {
                    GrantLifecyclePhase::Revoked
                } else {
                    GrantLifecyclePhase::Intended
                }
            }
        }
    }

    /// Whether `intent` is revoked, fenced, abandoned, released, or under a revoked ancestor.
    #[must_use]
    pub fn is_intent_fenced_or_revoked(&self, intent: &ContentId) -> bool {
        let Some(state) = self.intents.get(intent) else {
            return false;
        };
        !matches!(
            self.lifecycle_phase(intent),
            GrantLifecyclePhase::Intended | GrantLifecyclePhase::Issued
        ) || state
            .issued
            .is_some_and(|grant_ref| self.is_revoked(&grant_ref))
            || self.ancestor_revoked(&state.grant)
    }

    /// Whether a binding identity is revoked, fenced, released, or under a revoked ancestor.
    #[must_use]
    pub fn is_binding_fenced_or_revoked(&self, binding_id: &ContentId) -> bool {
        let Some(intent) = self.by_binding.get(binding_id) else {
            return true;
        };
        self.is_intent_fenced_or_revoked(intent)
    }

    /// Whether `work` has any unresolved `UnknownFenced` grant.
    #[must_use]
    pub fn has_unresolved_unknown_for_work(&self, work: &ContentId) -> bool {
        self.intents.values().any(|state| {
            &state.grant.work == work
                && matches!(state.lifecycle, IntentLifecycle::UnknownFenced { .. })
        })
    }

    /// Whether `attempt` has any unresolved `UnknownFenced` grant.
    #[must_use]
    pub fn has_unresolved_unknown_for_attempt(&self, attempt: &ContentId) -> bool {
        self.intents.values().any(|state| {
            &state.grant.attempt == attempt
                && matches!(state.lifecycle, IntentLifecycle::UnknownFenced { .. })
        })
    }

    /// Whether any grant for `attempt` is revoked, unknown-fenced, or abandoned.
    #[must_use]
    pub fn is_attempt_fenced_or_revoked(&self, attempt: &ContentId) -> bool {
        self.intents.iter().any(|(intent, state)| {
            &state.grant.attempt == attempt && self.is_intent_fenced_or_revoked(intent)
        })
    }

    /// All intents recorded in this state.
    #[must_use]
    pub fn intents_for_work_all(&self) -> Vec<ContentId> {
        self.intents.keys().cloned().collect()
    }

    /// All intents recorded for a work identity.
    #[must_use]
    pub fn intents_for_work(&self, work: &ContentId) -> Vec<ContentId> {
        self.intents
            .iter()
            .filter(|(_, state)| &state.grant.work == work)
            .map(|(intent, _)| intent.clone())
            .collect()
    }

    /// All intents recorded for an attempt identity.
    #[must_use]
    pub fn intents_for_attempt(&self, attempt: &ContentId) -> Vec<ContentId> {
        self.intents
            .iter()
            .filter(|(_, state)| &state.grant.attempt == attempt)
            .map(|(intent, _)| intent.clone())
            .collect()
    }

    /// All intents charged to `account`.
    #[must_use]
    pub fn intents_for_account(&self, account: &ContentId) -> Vec<ContentId> {
        self.intents
            .iter()
            .filter(|(_, state)| &state.grant.budget.account == account)
            .map(|(intent, _)| intent.clone())
            .collect()
    }

    /// Count active, pending, and unresolved-unknown child slots held under `parent_binding`.
    #[must_use]
    pub fn child_concurrency(&self, parent_binding: &ContentId) -> ChildConcurrencyState {
        let mut state = ChildConcurrencyState::default();
        for (intent, entry) in &self.intents {
            if entry.grant.parent.as_ref() != Some(parent_binding) {
                continue;
            }
            match self.lifecycle_phase(intent) {
                GrantLifecyclePhase::Intended => {
                    state.pending_children = state.pending_children.saturating_add(1);
                }
                GrantLifecyclePhase::Issued => {
                    state.active_children = state.active_children.saturating_add(1);
                }
                GrantLifecyclePhase::UnknownFenced => {
                    state.unknown_children = state.unknown_children.saturating_add(1);
                }
                GrantLifecyclePhase::None
                | GrantLifecyclePhase::Revoked
                | GrantLifecyclePhase::UnknownResolved
                | GrantLifecyclePhase::Released
                | GrantLifecyclePhase::Abandoned => {}
            }
        }
        state
    }

    /// Admit a child grant against both the lineage policy and concurrent child cap.
    ///
    /// # Errors
    /// Returns [`ChildGrantError`] if the parent is revoked/fenced, the fold
    /// bound is reached, lineage/attenuation/`#61` budget fails, concurrency cap
    /// is full, or the budget account conflicts with an existing reservation.
    pub fn check_child_admission(
        &self,
        parent: &AttemptAuthorityBinding,
        request: ChildGrantRequest,
        facts: &DelegationFacts<'_>,
        limit: ChildConcurrencyLimit,
    ) -> Result<AdmittedGrant, ChildGrantError> {
        limit.validate()?;
        if self.intents.len() >= MAX_GRANT_ISSUANCE_INTENTS {
            return Err(refusal(
                "grant_issuance_limit",
                "grant_issuance.intents",
                "grant issuance intents exceed the fold bound",
            )
            .into());
        }
        if self.is_revoked(&parent.grant_ref) || self.is_binding_fenced_or_revoked(&parent.identity)
        {
            return Err(ChildGrantError::ParentFenced);
        }
        if self
            .accounts
            .get(&request.budget.account)
            .is_some_and(|existing_parent| {
                *existing_parent != request.budget.parent_budget_identity
            })
        {
            return Err(refusal(
                "grant_issuance_budget_conflict",
                "grant.budget",
                "one budget account cannot be reserved from two parent budgets",
            )
            .into());
        }
        let concurrency = self.child_concurrency(&parent.identity);
        admit_child_with_concurrency(parent, request, facts, concurrency, limit)
    }

    /// Build a redacted `#60` / `#70` explanation for `work` without any credentials or paths.
    #[must_use]
    pub fn explain_for_work(
        &self,
        work: &ContentId,
        authority_generation: u64,
        revocation_generation: u64,
        max_concurrent_children: u32,
        last_denial: Option<&str>,
    ) -> RedactedGrantExplanation {
        let selected = self
            .intents
            .iter()
            .filter(|(_, entry)| &entry.grant.work == work)
            .max_by_key(|(_, entry)| (entry.grant.fence, entry.grant.depth));
        let (
            attempt,
            fence,
            intent,
            parent_binding,
            grant_ref_digest,
            depth,
            role,
            phase,
            concurrency,
        ) = match selected {
            Some((intent_id, entry)) => {
                let phase = self.lifecycle_phase(intent_id);
                let grant_ref_digest = entry.issued.as_ref().map(format_grant_ref_digest);
                let role = entry
                    .grant
                    .ceiling
                    .operations
                    .iter()
                    .next()
                    .copied()
                    .map(GrantRole::of);
                let concurrency = entry
                    .issued
                    .and_then(|grant_ref| entry.grant.binding_identity(&grant_ref).ok())
                    .map(|binding_id| self.child_concurrency(&binding_id))
                    .unwrap_or_default();
                (
                    Some(entry.grant.attempt.clone()),
                    Some(entry.grant.fence),
                    Some(intent_id.clone()),
                    entry.grant.parent.clone(),
                    grant_ref_digest,
                    Some(entry.grant.depth),
                    role,
                    phase,
                    concurrency,
                )
            }
            None => (
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                GrantLifecyclePhase::None,
                ChildConcurrencyState::default(),
            ),
        };
        RedactedGrantExplanation {
            schema: GRANT_ISSUANCE_EXPLANATION_SCHEMA.to_string(),
            work_version: work.clone(),
            attempt,
            fence,
            intent,
            parent_binding,
            grant_ref_digest,
            depth,
            role,
            authority_generation,
            revocation_generation: revocation_generation.max(self.max_revocation_generation()),
            phase,
            denial_reason: last_denial.map(sanitize_redacted_denial_code),
            active_children: concurrency.active_children,
            pending_children: concurrency.pending_children,
            unknown_children: concurrency.unknown_children,
            max_concurrent_children,
        }
    }

    /// Effects a live shell still owes, in intent order.
    ///
    /// The fold spans attempt lineages, so `current` resolves each intent to
    /// the current lineage of its work unit: the shell maps the intent's work
    /// unit to its current attempt, and returns `None` when the work unit has
    /// none (for example, cancelled). The resolver alone is not trusted for
    /// liveness: `attempts` must also report the intent's attempt as current
    /// at its fence at `now_ms` (active, unexpired lease, the work unit's
    /// current claim), so a lapsed lease that `expire` has not processed yet
    /// still abandons. An unissued intent is issued only when all of that
    /// holds and it binds exactly the resolved lineage; it is abandoned when
    /// `current` gives `None`, when it does not bind the resolved lineage
    /// (including a replaced fence), when its attempt is not current, when it
    /// is from another authority generation, under a revoked ancestor, past
    /// its expiry, or when its attempt ended in `Unknown`. `now_ms` is the one
    /// host clock; grant lifetimes compare in whole seconds of it.
    #[must_use]
    pub fn reconcile<'a>(
        &self,
        now_ms: u64,
        authority_generation: u64,
        attempts: &AttemptLedger,
        current: impl Fn(&AdmittedGrant) -> Option<&'a AttemptLineage>,
    ) -> Vec<GrantIssuanceAction> {
        let now_s = now_ms / MS_PER_SECOND;
        let mut actions = Vec::new();
        for (intent, state) in &self.intents {
            let ancestor_revoked = self.ancestor_revoked(&state.grant);
            let attempt_unknown = attempts
                .get(&state.grant.attempt)
                .is_some_and(|attempt| attempt.failure == Some(FailureClass::Unknown));
            match (&state.lifecycle, state.issued) {
                (
                    IntentLifecycle::Abandoned
                    | IntentLifecycle::Released
                    | IntentLifecycle::UnknownResolved { .. },
                    _,
                ) => {}
                (IntentLifecycle::UnknownFenced { .. }, Some(grant_ref)) => {
                    if !self.is_revoked(&grant_ref) {
                        actions.push(GrantIssuanceAction::Revoke {
                            intent: intent.clone(),
                            grant_ref,
                        });
                    }
                }
                (IntentLifecycle::UnknownFenced { .. }, None) => {
                    actions.push(GrantIssuanceAction::Abandon {
                        intent: intent.clone(),
                    });
                }
                (IntentLifecycle::Active | IntentLifecycle::Pending, Some(grant_ref)) => {
                    if (ancestor_revoked || attempt_unknown) && !self.is_revoked(&grant_ref) {
                        actions.push(GrantIssuanceAction::Revoke {
                            intent: intent.clone(),
                            grant_ref,
                        });
                    }
                }
                (IntentLifecycle::Active | IntentLifecycle::Pending, None) => {
                    if ancestor_revoked
                        || attempt_unknown
                        || now_s >= state.grant.ceiling.not_after_s
                        || state.grant.ceiling.generation != authority_generation
                        || !current(&state.grant).is_some_and(|lineage| state.grant.binds(lineage))
                        || !attempts.is_current(&state.grant.attempt, state.grant.fence, now_ms)
                    {
                        actions.push(GrantIssuanceAction::Abandon {
                            intent: intent.clone(),
                        });
                    } else {
                        actions.push(GrantIssuanceAction::Issue {
                            intent: intent.clone(),
                        });
                    }
                }
            }
        }
        actions
    }

    /// Whether any recorded ancestor of `grant` is revoked. The walk is bounded
    /// by the lineage limit and stops at an ancestor issued outside this journal.
    fn ancestor_revoked(&self, grant: &AdmittedGrant) -> bool {
        let mut parent = grant.parent.as_ref();
        for _ in 0..MAX_GRANT_LINEAGE {
            let Some(state) = parent
                .and_then(|binding| self.by_binding.get(binding))
                .and_then(|intent| self.intents.get(intent))
            else {
                return false;
            };
            if state
                .issued
                .as_ref()
                .is_some_and(|grant_ref| self.is_revoked(grant_ref))
                || matches!(
                    state.lifecycle,
                    IntentLifecycle::UnknownFenced { .. }
                        | IntentLifecycle::UnknownResolved { .. }
                        | IntentLifecycle::Abandoned
                )
            {
                return true;
            }
            parent = state.grant.parent.as_ref();
        }
        false
    }
}

/// Redacted `#60` / `#70` admission and grant-authority explanation for one work unit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedactedWorkAdmissionExplanation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admission: Option<kiln_core::fair_admission::AdmissionExplanation>,
    pub grant_authority: RedactedGrantExplanation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_denials: Vec<crate::authority_state::RedactedGrantDenial>,
}

/// Build a redacted `#60` admission and grant-lineage explanation for `work`
/// without mutating state or leaking credentials, seeds, or host paths.
///
/// Works on both read-write and `open_read_only` [`crate::Implementer`] handles.
///
/// # Errors
/// Returns a message if the ledger or authority database cannot be read or the
/// resulting explanation fails redaction validation.
pub fn explain_work_admission(
    implementer: &mut crate::Implementer,
    work: &ContentId,
    now_ms: u64,
    authority_generation: u64,
    max_concurrent_children: u32,
) -> Result<RedactedWorkAdmissionExplanation, String> {
    let ledger = implementer.admission_snapshot()?;
    let admission = ledger.explanation(work, now_ms).ok();
    let state_path = crate::authority_state::authority_state_path(&implementer.database)?;
    let (grant_state, rev_gen, denials) = if let Some(authority_state) =
        crate::authority_state::SqliteAuthorityState::open_read_only(&state_path)?
    {
        use crate::authority_state::AuthorityStateStore as _;
        let gs = authority_state.load_grant_issuance_state()?;
        let rg = authority_state.revocation_generation()?;
        let ds = authority_state
            .grant_denials_for_work(work, crate::authority_state::MAX_GRANT_DENIALS_QUERY)?;
        (gs, rg, ds)
    } else {
        (GrantIssuanceState::default(), 0, Vec::new())
    };
    let latest_denial = denials.first().map(|d| d.reason.as_str());
    let grant_authority = grant_state.explain_for_work(
        work,
        authority_generation,
        rev_gen,
        max_concurrent_children,
        latest_denial,
    );
    grant_authority
        .validate()
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    Ok(RedactedWorkAdmissionExplanation {
        admission,
        grant_authority,
        recent_denials: denials,
    })
}

/// Fence or revoke every grant bound to `attempt` (and its descendants via
/// lineage checks) when `attempt` cancels, expires, fails, blocks, or is
/// superseded. When `is_unknown` is true, records `UnknownTerminated` so the
/// grant is revoked while its child capacity slot remains held until
/// independently reconciled.
pub(crate) fn fence_or_revoke_for_attempt(
    database: &std::path::Path,
    attempt: &ContentId,
    is_unknown: bool,
    now_ms: u64,
) -> Result<(), String> {
    let state_path = crate::authority_state::authority_state_path(database)?;
    if !state_path.exists() {
        return Ok(());
    }
    let state = crate::authority_state::SqliteAuthorityState::open(&state_path)?;
    use crate::authority_state::AuthorityStateStore as _;
    let current = state.load_grant_issuance_state()?;
    let rev_gen = state
        .revocation_generation()?
        .max(current.max_revocation_generation())
        .saturating_add(1);
    let intents = current.intents_for_attempt(attempt);
    if intents.is_empty() {
        return Ok(());
    }
    let mut records = Vec::new();
    for intent in intents {
        let phase = current.lifecycle_phase(&intent);
        if is_unknown {
            if matches!(
                phase,
                GrantLifecyclePhase::Intended
                    | GrantLifecyclePhase::Issued
                    | GrantLifecyclePhase::Revoked
            ) {
                records.push(GrantIssuanceRecord::UnknownTerminated {
                    intent: intent.clone(),
                    revocation_generation: rev_gen,
                });
            }
        } else {
            match phase {
                GrantLifecyclePhase::Intended => {
                    records.push(GrantIssuanceRecord::Abandoned {
                        intent: intent.clone(),
                    });
                }
                GrantLifecyclePhase::Issued => {
                    if let Some(grant_ref) = current.issued(&intent) {
                        records.push(GrantIssuanceRecord::Revoked {
                            grant_ref: *grant_ref,
                            revocation_generation: rev_gen,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    if !records.is_empty() {
        state.record_grant_issuance_batch(&records)?;
    }
    if is_unknown {
        for intent in current.intents_for_attempt(attempt) {
            if let Some(grant) = current.grant(&intent) {
                let denial = crate::authority_state::RedactedGrantDenial::redacted(
                    grant.work.clone(),
                    Some(attempt.clone()),
                    Some(grant.fence),
                    "unknown_termination_fenced",
                    grant.ceiling.generation,
                    rev_gen,
                    now_ms,
                );
                let _ = state.record_grant_denial(&denial);
            }
        }
    }
    Ok(())
}

/// Reconcile an independently observed provider termination for `attempt` in
/// durable grant-issuance state: ensures `UnknownTerminated` is recorded if not
/// already fenced, then records `UnknownResolved` so the held capacity slot
/// releases while keeping the grant permanently revoked.
pub(crate) fn resolve_unknown_for_attempt(
    database: &std::path::Path,
    attempt: &ContentId,
    termination: &ExternalRef,
) -> Result<(), String> {
    let state_path = crate::authority_state::authority_state_path(database)?;
    if !state_path.exists() {
        return Ok(());
    }
    let state = crate::authority_state::SqliteAuthorityState::open(&state_path)?;
    use crate::authority_state::AuthorityStateStore as _;
    let current = state.load_grant_issuance_state()?;
    let rev_gen = state
        .revocation_generation()?
        .max(current.max_revocation_generation())
        .saturating_add(1);
    let intents = current.intents_for_attempt(attempt);
    if intents.is_empty() {
        return Ok(());
    }
    let mut records = Vec::new();
    for intent in intents {
        match current.lifecycle_phase(&intent) {
            GrantLifecyclePhase::UnknownFenced => {
                records.push(GrantIssuanceRecord::UnknownResolved {
                    intent,
                    termination: termination.clone(),
                });
            }
            GrantLifecyclePhase::Intended
            | GrantLifecyclePhase::Issued
            | GrantLifecyclePhase::Revoked => {
                records.push(GrantIssuanceRecord::UnknownTerminated {
                    intent: intent.clone(),
                    revocation_generation: rev_gen,
                });
                records.push(GrantIssuanceRecord::UnknownResolved {
                    intent,
                    termination: termination.clone(),
                });
            }
            GrantLifecyclePhase::UnknownResolved
            | GrantLifecyclePhase::Released
            | GrantLifecyclePhase::Abandoned
            | GrantLifecyclePhase::None => {}
        }
    }
    if !records.is_empty() {
        state.record_grant_issuance_batch(&records)?;
    }
    Ok(())
}

/// Cascade revocation in durable grant state to all grants charged against any
/// account in `accounts`.
pub(crate) fn cascade_revoke_for_accounts(
    database: &std::path::Path,
    accounts: &std::collections::BTreeSet<ContentId>,
    now_ms: u64,
) -> Result<(), String> {
    let state_path = crate::authority_state::authority_state_path(database)?;
    if !state_path.exists() {
        return Ok(());
    }
    let state = crate::authority_state::SqliteAuthorityState::open(&state_path)?;
    use crate::authority_state::AuthorityStateStore as _;
    let current = state.load_grant_issuance_state()?;
    let rev_gen = state
        .revocation_generation()?
        .max(current.max_revocation_generation())
        .saturating_add(1);
    let mut records = Vec::new();
    let mut denials = Vec::new();
    for account in accounts {
        for intent in current.intents_for_account(account) {
            match current.lifecycle_phase(&intent) {
                GrantLifecyclePhase::Intended => {
                    records.push(GrantIssuanceRecord::Abandoned {
                        intent: intent.clone(),
                    });
                }
                GrantLifecyclePhase::Issued => {
                    if let Some(grant_ref) = current.issued(&intent) {
                        records.push(GrantIssuanceRecord::Revoked {
                            grant_ref: *grant_ref,
                            revocation_generation: rev_gen,
                        });
                    }
                }
                _ => {}
            }
            if let Some(grant) = current.grant(&intent) {
                denials.push(crate::authority_state::RedactedGrantDenial::redacted(
                    grant.work.clone(),
                    Some(grant.attempt.clone()),
                    Some(grant.fence),
                    "account_revoked",
                    grant.ceiling.generation,
                    rev_gen,
                    now_ms,
                ));
            }
        }
    }
    if !records.is_empty() {
        state.record_grant_issuance_batch(&records)?;
    }
    for denial in denials {
        let _ = state.record_grant_denial(&denial);
    }
    Ok(())
}

/// Check that `(attempt, fence)` has no revoked/fenced grant and `work` has no
/// unresolved unknown termination blocking execution/reservation/transfer.
pub(crate) fn ensure_attempt_grant_usable(
    database: &std::path::Path,
    work: &ContentId,
    attempt: &ContentId,
    fence: u64,
    now_ms: u64,
) -> Result<(), String> {
    let state_path = crate::authority_state::authority_state_path(database)?;
    if !state_path.exists() {
        return Ok(());
    }
    let state = crate::authority_state::SqliteAuthorityState::open(&state_path)?;
    use crate::authority_state::AuthorityStateStore as _;
    let current = state.load_grant_issuance_state()?;
    let rev_gen = state
        .revocation_generation()?
        .max(current.max_revocation_generation());
    if current.has_unresolved_unknown_for_work(work)
        || current.has_unresolved_unknown_for_attempt(attempt)
    {
        let _ = state.record_grant_denial(&crate::authority_state::RedactedGrantDenial::redacted(
            work.clone(),
            Some(attempt.clone()),
            Some(fence),
            "unknown_termination_pending",
            0,
            rev_gen,
            now_ms,
        ));
        return Err(
            "admission_lineage_blocked: attempt has an unresolved unknown termination grant"
                .to_string(),
        );
    }
    for intent in current.intents_for_attempt(attempt) {
        if let Some(grant) = current.grant(&intent)
            && grant.fence == fence
            && current.is_intent_fenced_or_revoked(&intent)
        {
            let _ =
                state.record_grant_denial(&crate::authority_state::RedactedGrantDenial::redacted(
                    work.clone(),
                    Some(attempt.clone()),
                    Some(fence),
                    "grant_revoked_or_fenced",
                    grant.ceiling.generation,
                    rev_gen,
                    now_ms,
                ));
            return Err(
                "admission_lineage_blocked: attempt grant is revoked, released, or fenced"
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// Record a redacted denial in `.authority` if the database already exists.
pub(crate) fn record_denial_if_present(
    database: &std::path::Path,
    work: &ContentId,
    attempt: Option<&ContentId>,
    fence: Option<u64>,
    reason: &str,
    authority_generation: u64,
    now_ms: u64,
) {
    let Ok(state_path) = crate::authority_state::authority_state_path(database) else {
        return;
    };
    if !state_path.exists() {
        return;
    }
    let Ok(state) = crate::authority_state::SqliteAuthorityState::open(&state_path) else {
        return;
    };
    use crate::authority_state::AuthorityStateStore as _;
    let rev_gen = state.revocation_generation().unwrap_or(0);
    let denial = crate::authority_state::RedactedGrantDenial::redacted(
        work.clone(),
        attempt.cloned(),
        fence,
        reason,
        authority_generation,
        rev_gen,
        now_ms,
    );
    let _ = state.record_grant_denial(&denial);
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::authority::ProtectedOperation;
    use kiln_core::implementation::grant::{
        AttemptAuthorityBinding, AuthorityCeiling, CeilingTerms, ChildGrantRequest, CurrentFacts,
        DelegationFacts, GrantBudget, RootFacts, RootGrantRequest, admit_child, admit_root,
    };
    use kiln_core::implementation::{
        AcceptanceObligation, DEFAULT_LEASE_MS, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork,
        SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
    };
    use kiln_core::types::ExternalRef;
    use std::collections::BTreeSet;

    const NOW: u64 = 100;
    const NOW_MS: u64 = NOW * MS_PER_SECOND;
    const NOT_AFTER: u64 = 1_000;
    const GENERATION: u64 = 1;
    const COORDINATOR: &str = "did:key:coordinator";
    const FIRST_ISSUE: u32 = 70;
    const SECOND_ISSUE: u32 = 71;

    fn id(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn work_unit(issue: u32) -> ImplementationWork {
        let native_id = format!("onix/kiln#{issue}");
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: None,
            title: format!("Grant issuance work {issue}"),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: id('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", &native_id)
                    .unwrap_or_else(|_| unreachable!()),
                native_id,
                version: id('1'),
            }],
        }
    }

    /// Claim one work unit at time zero and return its current lineage.
    fn claim(attempts: &mut AttemptLedger, issue: u32, lease_ms: u64) -> AttemptLineage {
        let work = work_unit(issue);
        let attempt = attempts
            .claim(&work, 0, lease_ms)
            .unwrap_or_else(|_| unreachable!());
        AttemptLineage {
            campaign_revision: None,
            project: work.project,
            work: attempt.work_identity,
            work_version: attempt.work_revision,
            attempt: attempt.identity,
            fence: attempt.fence,
        }
    }

    /// A ledger holding one live claim, and that claim's lineage.
    fn ledger() -> (AttemptLedger, AttemptLineage) {
        let mut attempts = AttemptLedger::default();
        let lineage = claim(&mut attempts, FIRST_ISSUE, DEFAULT_LEASE_MS);
        (attempts, lineage)
    }

    fn lineage() -> AttemptLineage {
        ledger().1
    }

    fn budget(parent: char, account: char) -> GrantBudget {
        GrantBudget {
            parent_budget_identity: id(parent),
            account: id(account),
        }
    }

    fn terms(max_depth: u8, not_after_s: u64) -> CeilingTerms {
        CeilingTerms {
            audience: "did:web:kiln".to_string(),
            project: "kiln".to_string(),
            work: Some(lineage().work),
            operations: BTreeSet::from([ProtectedOperation::Dispatch]),
            not_after_s,
            max_depth,
            disclosure_routes: BTreeSet::new(),
        }
    }

    fn root_grant() -> AdmittedGrant {
        root_grant_with(COORDINATOR, budget('b', 'd'))
    }

    fn root_grant_with(holder: &str, reserved: GrantBudget) -> AdmittedGrant {
        root_grant_in(holder, reserved, &lineage())
    }

    fn root_grant_in(
        holder: &str,
        reserved: GrantBudget,
        attempt: &AttemptLineage,
    ) -> AdmittedGrant {
        let revoked = BTreeSet::new();
        let mut ceiling_terms = terms(2, NOT_AFTER);
        ceiling_terms.work = Some(attempt.work.clone());
        admit_root(
            RootGrantRequest {
                ceiling: AuthorityCeiling::new(GENERATION, ceiling_terms)
                    .unwrap_or_else(|_| unreachable!()),
                root_policy_ref: "policy:kiln".to_string(),
                holder: holder.to_string(),
                incarnation: None,
                budget: reserved.clone(),
            },
            &RootFacts {
                current: CurrentFacts {
                    now_s: NOW,
                    authority_generation: GENERATION,
                    revocation_generation: 0,
                    revoked: &revoked,
                    attempt,
                },
                budget: Some(&reserved),
            },
        )
        .unwrap_or_else(|_| unreachable!())
    }

    fn child_grant(
        parent: &AttemptAuthorityBinding,
        holder: &str,
        account: char,
        not_after_s: u64,
    ) -> AdmittedGrant {
        let revoked = BTreeSet::new();
        let attempt = lineage();
        let reserved = budget('d', account);
        admit_child(
            parent,
            ChildGrantRequest {
                terms: terms(0, not_after_s),
                holder: holder.to_string(),
                incarnation: Some(id('9')),
                budget: reserved.clone(),
            },
            &DelegationFacts {
                current: CurrentFacts {
                    now_s: NOW,
                    authority_generation: GENERATION,
                    revocation_generation: 0,
                    revoked: &revoked,
                    attempt: &attempt,
                },
                delegator: COORDINATOR,
                delegate_permitted: true,
                ancestors: &[],
                budget: Some(&reserved),
            },
        )
        .unwrap_or_else(|_| unreachable!())
    }

    fn proof(byte: u8) -> GrantRef {
        [byte; AUTHORITY_DIGEST_BYTES]
    }

    fn intended(grant: &AdmittedGrant) -> GrantIssuanceRecord {
        GrantIssuanceRecord::Intended {
            grant: Box::new(grant.clone()),
        }
    }

    fn issued(grant: &AdmittedGrant, byte: u8) -> GrantIssuanceRecord {
        GrantIssuanceRecord::Issued {
            intent: grant.intent.clone(),
            grant_ref: proof(byte),
        }
    }

    fn code(result: CoreResult<()>) -> Option<String> {
        result.err().map(|diagnostic| diagnostic.code)
    }

    #[test]
    // r[verify kiln.attempt.grant_lineage.issuance] r[verify kiln.attempt.grant_lineage.issuance.scenario.recover]
    fn reconcile_resolves_each_intent_against_its_own_work_unit() {
        let mut attempts = AttemptLedger::default();
        let first_lineage = claim(&mut attempts, FIRST_ISSUE, DEFAULT_LEASE_MS);
        let second_lineage = claim(&mut attempts, SECOND_ISSUE, DEFAULT_LEASE_MS);
        let first = root_grant_in(COORDINATOR, budget('b', 'd'), &first_lineage);
        let second = root_grant_in(COORDINATOR, budget('b', 'e'), &second_lineage);
        let state = GrantIssuanceState::replay(&[intended(&first), intended(&second)])
            .unwrap_or_else(|_| unreachable!());
        let sorted = |mut actions: Vec<GrantIssuanceAction>| {
            actions.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
            actions
        };
        let issue = |grant: &AdmittedGrant| GrantIssuanceAction::Issue {
            intent: grant.intent.clone(),
        };
        let abandon = |grant: &AdmittedGrant| GrantIssuanceAction::Abandon {
            intent: grant.intent.clone(),
        };
        let reconcile = |units: &BTreeMap<ContentId, AttemptLineage>| {
            sorted(state.reconcile(NOW_MS, GENERATION, &attempts, |grant| {
                units.get(&grant.work)
            }))
        };

        let both = BTreeMap::from([
            (first_lineage.work.clone(), first_lineage.clone()),
            (second_lineage.work.clone(), second_lineage.clone()),
        ]);
        assert_eq!(
            reconcile(&both),
            sorted(vec![issue(&first), issue(&second)]),
            "two current attempts are both issued in one call"
        );

        let cancelled = BTreeMap::from([(first_lineage.work.clone(), first_lineage.clone())]);
        assert_eq!(
            reconcile(&cancelled),
            sorted(vec![issue(&first), abandon(&second)]),
            "a work unit with no current attempt abandons its intent"
        );

        let replaced = BTreeMap::from([
            (first_lineage.work.clone(), first_lineage),
            (
                second_lineage.work.clone(),
                AttemptLineage {
                    attempt: id('c'),
                    fence: 3,
                    ..second_lineage
                },
            ),
        ]);
        assert_eq!(
            reconcile(&replaced),
            sorted(vec![issue(&first), abandon(&second)]),
            "a stale attempt abandons while a current one issues"
        );
    }

    #[test]
    // r[verify kiln.attempt.grant_lineage.issuance] r[verify kiln.attempt.grant_lineage.issuance.scenario.recover]
    fn a_lapsed_lease_abandons_before_expire_runs() {
        let lapsing_lease_ms = NOW_MS / 2;
        let mut attempts = AttemptLedger::default();
        let live = claim(&mut attempts, FIRST_ISSUE, DEFAULT_LEASE_MS);
        let lapsed = claim(&mut attempts, SECOND_ISSUE, lapsing_lease_ms);
        let lapsed_attempt = attempts
            .get(&lapsed.attempt)
            .unwrap_or_else(|| unreachable!());
        assert!(
            lapsed_attempt.phase.is_active() && lapsed_attempt.lease_until_ms <= NOW_MS,
            "the lease lapsed but `expire` has not run"
        );
        let live_grant = root_grant_in(COORDINATOR, budget('b', 'd'), &live);
        let lapsed_grant = root_grant_in(COORDINATOR, budget('b', 'e'), &lapsed);
        let state = GrantIssuanceState::replay(&[intended(&live_grant), intended(&lapsed_grant)])
            .unwrap_or_else(|_| unreachable!());
        // The resolver still reports the lapsed attempt as its work unit's
        // current one, as `AttemptLedger::current` does until `expire` runs.
        let units = BTreeMap::from([
            (live.work.clone(), live.clone()),
            (lapsed.work.clone(), lapsed.clone()),
        ]);
        let mut actions = state.reconcile(NOW_MS, GENERATION, &attempts, |grant| {
            units.get(&grant.work)
        });
        actions.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
        let mut expected = vec![
            GrantIssuanceAction::Issue {
                intent: live_grant.intent.clone(),
            },
            GrantIssuanceAction::Abandon {
                intent: lapsed_grant.intent.clone(),
            },
        ];
        expected.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
        assert_eq!(actions, expected);
    }

    #[test]
    // r[verify kiln.attempt.grant_lineage.issuance] r[verify kiln.attempt.grant_lineage.issuance.scenario.recover]
    fn a_crash_after_the_intent_resumes_that_intent_and_never_a_second() {
        let root = root_grant();
        let (attempts, current) = ledger();
        let crashed = GrantIssuanceState::replay(&[intended(&root), intended(&root)])
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            crashed.reconcile(NOW_MS, GENERATION, &attempts, |_| Some(&current)),
            vec![GrantIssuanceAction::Issue {
                intent: root.intent.clone()
            }],
            "a duplicate intent stays one logical issuance"
        );
        assert_eq!(crashed.issued(&root.intent), None);
        let abandoned = vec![GrantIssuanceAction::Abandon {
            intent: root.intent.clone(),
        }];
        assert_eq!(
            crashed.reconcile(NOW_MS, GENERATION + 1, &attempts, |_| Some(&current)),
            abandoned,
            "an intent from a moved authority generation is never minted"
        );
        let mut replaced = lineage();
        replaced.attempt = id('f');
        replaced.fence = 2;
        assert_eq!(
            crashed.reconcile(NOW_MS, GENERATION, &attempts, |_| Some(&replaced)),
            abandoned,
            "an intent for a replaced attempt is never minted"
        );
        let mut other_project = lineage();
        other_project.project = "other".to_string();
        assert_eq!(
            crashed.reconcile(NOW_MS, GENERATION, &attempts, |_| Some(&other_project)),
            abandoned,
            "an intent for another lineage is never minted"
        );

        let records = [intended(&root), issued(&root, 1), issued(&root, 1)];
        let done = GrantIssuanceState::replay(&records).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            done.reconcile(NOW_MS, GENERATION, &attempts, |_| Some(&current)),
            Vec::new(),
            "replay owes no new grant"
        );
        assert_eq!(done.issued(&root.intent), Some(&proof(1)));
        assert_eq!(
            GrantIssuanceState::replay(&records).ok(),
            Some(done.clone()),
            "replay is deterministic"
        );

        let mut second = done.clone();
        assert_eq!(
            code(second.apply(&issued(&root, 2))),
            Some("grant_issuance_duplicate".to_string())
        );
        let child = child_grant(
            &root
                .clone()
                .issue(proof(1))
                .unwrap_or_else(|_| unreachable!()),
            "did:key:worker",
            'e',
            NOT_AFTER,
        );
        let mut reused = done.clone();
        reused
            .apply(&intended(&child))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            code(reused.apply(&issued(&child, 1))),
            Some("grant_issuance_reused_proof".to_string())
        );
        let mut unknown = GrantIssuanceState::default();
        assert_eq!(
            code(unknown.apply(&issued(&root, 1))),
            Some("grant_issuance_unknown_intent".to_string())
        );
        let mut forged = root.clone();
        forged.holder = "did:key:other".to_string();
        assert_eq!(
            code(unknown.apply(&intended(&forged))),
            Some("authority_grant_intent".to_string())
        );
    }

    #[test]
    // r[verify kiln.attempt.grant_lineage.budget] r[verify kiln.attempt.grant_lineage.budget.scenario.refused]
    fn one_account_cannot_be_reserved_from_two_parent_budgets() {
        let root = root_grant();
        let mut state =
            GrantIssuanceState::replay(&[intended(&root)]).unwrap_or_else(|_| unreachable!());
        let binding = root.issue(proof(1)).unwrap_or_else(|_| unreachable!());
        let shared = child_grant(&binding, "did:key:worker", 'e', NOT_AFTER);
        let other = child_grant(&binding, "did:key:helper", 'e', NOT_AFTER);
        state
            .apply(&intended(&shared))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            code(state.apply(&intended(&other))),
            None,
            "two grants on one reserved account share its allowance"
        );
        let repointed = root_grant_with("did:key:other", budget('f', 'd'));
        assert_eq!(
            code(state.apply(&intended(&repointed))),
            Some("grant_issuance_budget_conflict".to_string())
        );
    }

    #[test]
    // r[verify kiln.attempt.grant_lineage.currentness.scenario.propagate]
    fn revocation_propagates_to_issued_and_pending_descendants() {
        let root = root_grant();
        let binding = root
            .clone()
            .issue(proof(1))
            .unwrap_or_else(|_| unreachable!());
        let issued_child = child_grant(&binding, "did:key:worker", 'e', NOT_AFTER);
        let pending_child = child_grant(&binding, "did:key:helper", 'f', NOT_AFTER);
        let records = vec![
            intended(&root),
            issued(&root, 1),
            intended(&issued_child),
            issued(&issued_child, 2),
            intended(&pending_child),
            GrantIssuanceRecord::Revoked {
                grant_ref: proof(1),
                revocation_generation: 4,
            },
        ];
        let state = GrantIssuanceState::replay(&records).unwrap_or_else(|_| unreachable!());
        let mut expected = vec![
            GrantIssuanceAction::Revoke {
                intent: issued_child.intent.clone(),
                grant_ref: proof(2),
            },
            GrantIssuanceAction::Abandon {
                intent: pending_child.intent.clone(),
            },
        ];
        let (attempts, current) = ledger();
        let mut actions = state.reconcile(NOW_MS, GENERATION, &attempts, |_| Some(&current));
        actions.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
        expected.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
        assert_eq!(actions, expected);

        let mut acknowledged = state.clone();
        acknowledged
            .apply(&GrantIssuanceRecord::Revoked {
                grant_ref: proof(2),
                revocation_generation: 5,
            })
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            acknowledged.reconcile(NOW_MS, GENERATION, &attempts, |_| Some(&current)),
            vec![GrantIssuanceAction::Abandon {
                intent: pending_child.intent.clone()
            }],
            "an acknowledged revocation is not repeated"
        );

        let short = child_grant(&binding, "did:key:short", 'c', NOW + 1);
        let expiring =
            GrantIssuanceState::replay(&[intended(&root), issued(&root, 1), intended(&short)])
                .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            expiring.reconcile((NOW + 1) * MS_PER_SECOND, GENERATION, &attempts, |_| {
                Some(&current)
            }),
            vec![GrantIssuanceAction::Abandon {
                intent: short.intent.clone()
            }],
            "an expired intent is never minted"
        );

        let json = serde_json::to_string(&records).unwrap_or_else(|_| unreachable!());
        let restored: Vec<GrantIssuanceRecord> =
            serde_json::from_str(&json).unwrap_or_else(|_| unreachable!());
        assert_eq!(restored, records);
    }
}
