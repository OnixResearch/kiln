//! Durable selected-campaign state and intent-before-effect controller.
use std::{path::Path, time::Duration};

use kiln_core::{
    implementation::{
        campaign::{
            CampaignContract, CampaignDecision, CampaignOperation, CampaignState, EffectState,
            MemberObservation, MemberStage, PendingEffect, SelectionBasis,
        },
        continuation::{
            BatchOutcome, CampaignView, ContinuationPolicy, ContinuationRecord, ContinuationStep,
            DriveReason, WakeHint, WakeKind,
        },
    },
    types::ContentId,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Serialize, de::DeserializeOwned};

use kiln_core::implementation::campaign::{DecisionRequestState, HumanDecisionRequest};
const BUSY_TIMEOUT_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CampaignDecisionAnswer<'a> {
    pub identity: &'a ContentId,
    pub work: Option<&'a ContentId>,
    pub policy: &'a ContentId,
    pub candidate: Option<&'a ContentId>,
    pub answer: &'a str,
}

pub(crate) const MAX_CAMPAIGN_BYTES: i64 = 32 * 1024 * 1024;
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS implementation_campaign (campaign_id TEXT PRIMARY KEY, version INTEGER NOT NULL, bytes BLOB NOT NULL); CREATE TABLE IF NOT EXISTS implementation_campaign_continuation (campaign_id TEXT PRIMARY KEY, version INTEGER NOT NULL, bytes BLOB NOT NULL); CREATE TABLE IF NOT EXISTS implementation_campaign_command (request_id TEXT PRIMARY KEY, campaign_id TEXT NOT NULL, fingerprint TEXT NOT NULL, outcome BLOB NOT NULL);";
/// Prefix of every refusal caused by a campaign version that moved.
pub const STALE_CAMPAIGN_VERSION: &str = "stale campaign version";
/// Largest stored continuation record; it holds pacing only.
const MAX_CONTINUATION_BYTES: i64 = 64 * 1024;

#[derive(Debug)]
pub struct CampaignStore {
    connection: Connection,
}
impl CampaignStore {
    /// Open the campaign table in the existing implementation database. No work or attempt rows are copied.
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(text)?;
        connection
            .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
            .map_err(text)?;
        connection.execute_batch(SCHEMA).map_err(text)?;
        Ok(Self { connection })
    }
    pub fn start(&mut self, contract: CampaignContract) -> Result<CampaignState, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let state = start_in(&tx, contract).map_err(TransitionFailure::text)?;
        tx.commit().map_err(text)?;
        Ok(state)
    }
    pub fn version(&self, campaign_id: &str) -> Result<Option<u64>, String> {
        let row: Option<i64> = self
            .connection
            .query_row(
                "SELECT version FROM implementation_campaign WHERE campaign_id=?1",
                params![campaign_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(text)?;
        row.map(u64::try_from).transpose().map_err(text)
    }
    pub fn inspect(&self, campaign_id: &str) -> Result<Option<CampaignState>, String> {
        self.load(campaign_id)
    }
    pub fn list(&self) -> Result<Vec<CampaignState>, String> {
        let mut statement = self
            .connection
            .prepare("SELECT bytes FROM implementation_campaign ORDER BY campaign_id")
            .map_err(text)?;
        let rows = statement
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(text)?;
        rows.map(|row| {
            let bytes = row.map_err(text)?;
            decode(&bytes)
        })
        .collect()
    }
    pub fn replace_revision(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        next: CampaignContract,
        explicit_supersession: bool,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |state| {
            state.revise(next, explicit_supersession)
        })
    }
    pub fn observe(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        observation: MemberObservation,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |mut state| {
            state.observe(observation)?;
            Ok(state)
        })
    }
    pub fn observe_successor(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        observation: MemberObservation,
        integration_effect: &ContentId,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |mut state| {
            state.observe_successor(observation, integration_effect)?;
            Ok(state)
        })
    }
    pub fn observe_budget(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        budget: kiln_core::implementation::campaign::CampaignBudgetObservation,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |mut state| {
            state.observe_budget(budget)?;
            Ok(state)
        })
    }
    /// Retain exact decision projections in one atomic attempt-context update.
    pub fn retain_attempt_contexts(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        contexts: Vec<kiln_core::implementation::campaign::RetainedAttemptContext>,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |mut state| {
            for context in contexts {
                state.retain_attempt_context(context)?;
            }
            Ok(state)
        })
    }
    /// Record one member's typed selected-work read without rewriting its
    /// bound attempt contract.
    pub fn observe_selected_work(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        member: &str,
        intake: kiln_core::implementation::selected_work::IntakeProfile,
        observation: kiln_core::implementation::selected_work::SelectedWorkObservation,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |mut state| {
            state.observe_selected_work(member, intake, observation)?;
            Ok(state)
        })
    }
    /// Retain the exact snapshot and guidance supplied to a member's attempt.
    pub fn bind_selected_work_attempt(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        member: &str,
        contract: kiln_core::implementation::selected_work::AttemptSnapshotContract,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |mut state| {
            state.bind_selected_work_attempt(member, contract)?;
            Ok(state)
        })
    }
    /// Add durable decision demand under the shared parent-budget scope. Excess
    /// demand remains unissued rather than multiplying the outstanding bound.
    pub fn add_decision(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        mut request: HumanDecisionRequest,
    ) -> Result<CampaignState, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (mut state, version) = load_tx(&tx, campaign_id)?;
        if version != expected_version {
            return Err(format!(
                "{STALE_CAMPAIGN_VERSION}: expected {expected_version}, current {version}"
            ));
        }
        let outstanding = scope_outstanding(&tx, &state).map_err(TransitionFailure::text)?;
        if request.state == DecisionRequestState::Outstanding
            && outstanding >= state.contract.max_outstanding_decisions as usize
        {
            request.state = DecisionRequestState::Unissued;
            request.pending_reason =
                Some("shared operator decision scope is at its outstanding bound".into());
        }
        state.add_decision(request).map_err(diagnostic)?;
        save(&tx, &state, version)?;
        tx.commit().map_err(text)?;
        Ok(state)
    }
    pub fn issue_decision(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        identity: &ContentId,
    ) -> Result<CampaignState, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let state = issue_in(&tx, campaign_id, expected_version, identity)
            .map_err(TransitionFailure::text)?;
        tx.commit().map_err(text)?;
        Ok(state)
    }
    pub fn resolve_decision(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        answer: CampaignDecisionAnswer<'_>,
    ) -> Result<CampaignState, String> {
        self.mutate_hinted(
            campaign_id,
            expected_version,
            Some(WakeKind::Review),
            |mut state| {
                state.resolve_decision(
                    answer.identity,
                    answer.work,
                    answer.policy,
                    answer.candidate,
                    answer.answer,
                )?;
                Ok(state)
            },
        )
    }
    pub fn cancel(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |mut state| {
            state.cancel();
            Ok(state)
        })
    }
    pub fn continue_admission(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
    ) -> Result<CampaignState, String> {
        self.mutate_hinted(
            campaign_id,
            expected_version,
            Some(WakeKind::Operator),
            |mut state| {
                state.continue_admission();
                Ok(state)
            },
        )
    }
    pub fn pause(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |mut state| {
            state.pause();
            Ok(state)
        })
    }
    /// The returned decision is pure; no provider call is performed.
    pub fn decide(&self, campaign_id: &str) -> Result<CampaignDecision, String> {
        self.load(campaign_id)?
            .ok_or("campaign not found")?
            .next_action()
            .map_err(diagnostic)
    }
    /// Persist effect intent atomically before the caller contacts the named owner.
    pub fn persist_effect_intent(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        member: String,
        operation: CampaignOperation,
        owner: String,
        intent_identity: ContentId,
    ) -> Result<PendingEffect, String> {
        let current = self.load(campaign_id)?.ok_or("campaign not found")?;
        let observation = current
            .observations
            .get(&member)
            .ok_or("campaign member has no current owner observation")?;
        let effect = PendingEffect {
            effect_id: effect_id(campaign_id, &member, &operation, &intent_identity)?,
            member: member.clone(),
            operation,
            owner,
            intent_identity,
            state: EffectState::IntentPersisted,
            progress_applied: false,
            selection_identity: current.selection_identity.clone(),
            source_identity: observation.source_identity.clone(),
            work_identity: observation.work_identity.clone(),
            attempt_identity: observation.attempt_identity.clone(),
            attempt_fence: observation.attempt_fence,
            subject_identity: observation.subject_identity.clone(),
            policy_identity: observation.policy_identity.clone(),
            context_identity: observation.context_identity.clone(),
            native_context_identity: observation.native_context_identity.clone(),
            profile_identity: observation.profile_identity.clone(),
            evidence_identity: None,
            integration_target_identity: observation.integration_target_identity.clone(),
        };
        self.mutate(campaign_id, expected_version, |mut state| {
            if state.cancelled || state.paused {
                return Err(kiln_core::types::Diagnostic::new(
                    "campaign_admission_closed",
                    "campaign",
                    "campaign is cancelled or paused",
                ));
            }
            if state.pending_effects.contains_key(&effect.effect_id) {
                return Err(kiln_core::types::Diagnostic::new(
                    "campaign_effect_already_retained",
                    "effect.identity",
                    "effect identity is already retained; reconcile it instead of redispatching",
                ));
            }
            if state
                .pending_effects
                .values()
                .any(|p| p.member == member && !p.progress_applied)
            {
                return Err(kiln_core::types::Diagnostic::new(
                    "campaign_effect_pending",
                    "campaign",
                    "member already has an unapplied effect",
                ));
            }
            state
                .pending_effects
                .insert(effect.effect_id.clone(), effect.clone());
            Ok(state)
        })?;
        Ok(effect)
    }
    /// Record a named owner's actual observation. A transport return alone is not accepted as evidence.
    pub fn observe_effect(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        effect_id: &ContentId,
        observed_state: EffectState,
        owner_evidence: ContentId,
    ) -> Result<CampaignState, String> {
        if !matches!(
            observed_state,
            EffectState::ObservedComplete | EffectState::ObservedFailed | EffectState::Unknown
        ) {
            return Err("effect observation must be complete, failed, or unknown".into());
        }
        self.mutate(campaign_id, expected_version, |mut state| {
            let effect = state.pending_effects.get_mut(effect_id).ok_or_else(|| {
                kiln_core::types::Diagnostic::new(
                    "campaign_effect_missing",
                    "effect",
                    "effect intent is not retained",
                )
            })?;
            effect.state = observed_state;
            let observation = state.observations.get_mut(&effect.member).ok_or_else(|| {
                kiln_core::types::Diagnostic::new(
                    "campaign_member_missing",
                    "effect.member",
                    "member observation is not retained",
                )
            })?;
            if !observation.evidence.contains(&owner_evidence) {
                observation.evidence.push(owner_evidence);
            }
            Ok(state)
        })
    }
    /// Atomically retain the named owner's exact receipt and its fresh member
    /// projection, closing the crash gap between effect reconciliation and progress.
    pub fn apply_owner_observation(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        effect_id: &ContentId,
        result: EffectState,
        evidence: ContentId,
        mut member: MemberObservation,
    ) -> Result<CampaignState, String> {
        if !matches!(
            result,
            EffectState::ObservedComplete | EffectState::ObservedFailed
        ) {
            return Err("owner observation must be complete or failed".into());
        }
        self.mutate(campaign_id, expected_version, |mut state| {
            let effect = state.pending_effects.get(effect_id).ok_or_else(|| {
                kiln_core::types::Diagnostic::new(
                    "campaign_effect_missing",
                    "effect",
                    "effect intent is not retained",
                )
            })?;
            let effect_member = effect.member.clone();
            let old = state.observations.get(&effect_member).ok_or_else(|| {
                kiln_core::types::Diagnostic::new(
                    "campaign_member_missing",
                    "effect.member",
                    "member observation is not retained",
                )
            })?;
            if effect_member != member.member {
                reject_owner_output(&mut state, effect_id, evidence.clone())?;
                return Ok(state);
            }
            if effect.selection_identity != state.selection_identity
                || effect.source_identity != old.source_identity
                || effect.work_identity != old.work_identity
                || effect.attempt_identity != old.attempt_identity
                || effect.attempt_fence != old.attempt_fence
                || effect.subject_identity != old.subject_identity
                || effect.policy_identity != old.policy_identity
                || effect.context_identity != old.context_identity
                || effect.native_context_identity != old.native_context_identity
                || effect.profile_identity != old.profile_identity
                || effect.integration_target_identity != old.integration_target_identity
            {
                reject_owner_output(&mut state, effect_id, evidence.clone())?;
                return Ok(state);
            }
            if member.member != old.member
                || member.source_identity != effect.source_identity
                || member.work_identity != effect.work_identity
                || member.attempt_identity != effect.attempt_identity
                || member.attempt_fence != effect.attempt_fence
                || member.policy_identity != effect.policy_identity
                || member.profile_identity != effect.profile_identity
                || member.subject_identity != effect.subject_identity
                || member.context_identity != effect.context_identity
                || member.native_context_identity != effect.native_context_identity
                || member.integration_target_identity != effect.integration_target_identity
            {
                reject_owner_output(&mut state, effect_id, evidence.clone())?;
                return Ok(state);
            }
            if state.cancelled {
                reject_owner_output(&mut state, effect_id, evidence)?;
                return Ok(state);
            }
            if !member.evidence.contains(&evidence) {
                member.evidence.push(evidence.clone());
            }
            state.observe(member)?;
            let effect = state.pending_effects.get_mut(effect_id).ok_or_else(|| {
                kiln_core::types::Diagnostic::new(
                    "campaign_effect_missing",
                    "effect",
                    "effect disappeared during atomic application",
                )
            })?;
            effect.state = result;
            effect.progress_applied = true;
            effect.evidence_identity = Some(evidence);
            Ok(state)
        })
    }
    pub fn recover_pending(&self, campaign_id: &str) -> Result<Vec<PendingEffect>, String> {
        Ok(self
            .load(campaign_id)?
            .ok_or("campaign not found")?
            .pending_effects
            .into_values()
            .filter(|e| !e.progress_applied)
            .collect())
    }
    /// The continuation record of an existing campaign and its row version
    /// (`0` before the first write).
    pub fn continuation(&self, campaign_id: &str) -> Result<(ContinuationRecord, u64), String> {
        if self.version(campaign_id)?.is_none() {
            return Err("campaign not found".into());
        }
        load_continuation(&self.connection, campaign_id)
    }
    // r[impl kiln.implementation.campaign.continuation.scenario.restart]
    /// Coalesce one wake hint into an existing campaign's continuation record.
    /// The hint grants nothing; it only makes the next step reread.
    pub fn record_wake(
        &mut self,
        campaign_id: &str,
        hint: WakeHint,
    ) -> Result<(ContinuationRecord, u64), String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM implementation_campaign WHERE campaign_id=?1)",
                params![campaign_id],
                |r| r.get(0),
            )
            .map_err(text)?;
        if !exists {
            return Err("campaign not found".into());
        }
        let (mut record, version) = load_continuation(&tx, campaign_id)?;
        record.record_hint(hint);
        let version = save_continuation(&tx, &record, version)?;
        tx.commit().map_err(text)?;
        Ok((record, version))
    }
    /// Merge one batch outcome into the current record. Hints recorded during
    /// the batch stay pending: consumption stops at the sequence read before it.
    pub fn record_batch(
        &mut self,
        campaign_id: &str,
        batch: RecordedBatch,
        policy: &ContinuationPolicy,
    ) -> Result<(ContinuationRecord, u64), String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (mut record, version) = load_continuation(&tx, campaign_id)?;
        record.apply_outcome(
            batch.outcome,
            batch.store_version,
            batch.consumed_sequence,
            batch.finished_ms,
            policy,
        );
        let version = save_continuation(&tx, &record, version)?;
        tx.commit().map_err(text)?;
        Ok((record, version))
    }
    fn load(&self, campaign_id: &str) -> Result<Option<CampaignState>, String> {
        let row:Option<(i64,Option<Vec<u8>>)>=self.connection.query_row("SELECT length(bytes), CASE WHEN length(bytes)<=?2 THEN bytes ELSE NULL END FROM implementation_campaign WHERE campaign_id=?1",params![campaign_id,MAX_CAMPAIGN_BYTES],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(text)?;
        match row {
            None => Ok(None),
            Some((len, _)) if !(0..=MAX_CAMPAIGN_BYTES).contains(&len) => {
                Err("campaign record exceeds bound".into())
            }
            Some((_, None)) => Err("campaign record exceeds bound".into()),
            Some((_, Some(bytes))) => Ok(Some(decode(&bytes)?)),
        }
    }
    fn mutate(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        apply: impl FnOnce(CampaignState) -> Result<CampaignState, kiln_core::types::Diagnostic>,
    ) -> Result<CampaignState, String> {
        self.mutate_hinted(campaign_id, expected_version, None, apply)
    }
    /// Apply one transition and, in the same transaction, coalesce a wake hint
    /// for the version it produced, so a committed transition never loses it.
    fn mutate_hinted(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        hint: Option<WakeKind>,
        apply: impl FnOnce(CampaignState) -> Result<CampaignState, kiln_core::types::Diagnostic>,
    ) -> Result<CampaignState, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let next = transition_in(&tx, campaign_id, expected_version, hint, apply)
            .map_err(TransitionFailure::text)?;
        tx.commit().map_err(text)?;
        Ok(next)
    }
}
/// Read-only campaign projection. Unlike `CampaignStore`, opening this handle
/// never creates schema or mutates journal/database settings.
#[derive(Debug)]
pub struct CampaignObservation {
    connection: Connection,
}
impl CampaignObservation {
    pub fn open(path: &Path) -> Result<Self, String> {
        use rusqlite::OpenFlags;
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(text)?;
        connection
            .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
            .map_err(text)?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(text)?;
        Ok(Self { connection })
    }
    pub fn inspect(&self, campaign_id: &str) -> Result<Option<(CampaignState, u64)>, String> {
        let row: Option<(i64, i64, Option<Vec<u8>>)> = self.connection.query_row(
            "SELECT version,length(bytes),CASE WHEN length(bytes)<=?2 THEN bytes ELSE NULL END FROM implementation_campaign WHERE campaign_id=?1",
            params![campaign_id, MAX_CAMPAIGN_BYTES],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional().map_err(text)?;
        let Some((version, length, bytes)) = row else {
            return Ok(None);
        };
        if !(0..=MAX_CAMPAIGN_BYTES).contains(&length) || bytes.is_none() {
            return Err("campaign record exceeds bound".into());
        }
        let state: CampaignState = decode(&bytes.unwrap_or_default())?;
        if state.contract.campaign_id != campaign_id {
            return Err("campaign key and payload identity differ".into());
        }
        Ok(Some((state, u64::try_from(version).map_err(text)?)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignOwnerObservation {
    pub result: EffectState,
    pub evidence_identity: ContentId,
    pub member: MemberObservation,
}

/// Implementations adapt existing canonical owners; dispatch is called only
/// after the corresponding intent is committed. Reconcile is read-only.
pub trait CampaignEffectConsumer {
    fn dispatch(
        &mut self,
        effect: &PendingEffect,
    ) -> Result<Option<CampaignOwnerObservation>, String>;
    fn reconcile(
        &mut self,
        effect: &PendingEffect,
    ) -> Result<Option<CampaignOwnerObservation>, String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignDriveResult {
    pub state: CampaignState,
    pub decision: CampaignDecision,
    pub effect: Option<PendingEffect>,
}

/// Advance at most one selected operation. Uncertain effects are reconciled
/// before any new dispatch; a callback failure leaves committed intent intact.
pub fn drive_one<C: CampaignEffectConsumer>(
    store: &mut CampaignStore,
    campaign_id: &str,
    expected_version: u64,
    consumer: &mut C,
) -> Result<CampaignDriveResult, String> {
    let state = store.inspect(campaign_id)?.ok_or("campaign not found")?;
    let action = state.next_action().map_err(diagnostic)?;
    if action.operation == CampaignOperation::Reconcile {
        let effect = store
            .recover_pending(campaign_id)?
            .into_iter()
            .next()
            .ok_or("campaign requests reconciliation without retained effect")?;
        return match consumer.reconcile(&effect)? {
            None => Ok(CampaignDriveResult {
                state,
                decision: action,
                effect: Some(effect),
            }),
            Some(observation) => {
                let version = store.version(campaign_id)?.ok_or("campaign not found")?;
                let state = store.apply_owner_observation(
                    campaign_id,
                    version,
                    &effect.effect_id,
                    observation.result,
                    observation.evidence_identity,
                    observation.member,
                )?;
                let decision = state.next_action().map_err(diagnostic)?;
                Ok(CampaignDriveResult {
                    state,
                    decision,
                    effect: Some(effect),
                })
            }
        };
    }
    if !action.operation.is_owner_effect() {
        return Ok(CampaignDriveResult {
            state,
            decision: action,
            effect: None,
        });
    }
    let member = action
        .member
        .clone()
        .ok_or("effectful campaign decision lacks member")?;
    let intent = intent_identity(campaign_id, &state, &action, &member)?;
    let effect = store.persist_effect_intent(
        campaign_id,
        expected_version,
        member,
        action.operation,
        action
            .owner
            .clone()
            .ok_or("effectful campaign decision lacks owner")?,
        intent,
    )?;
    match consumer.dispatch(&effect)? {
        None => {
            let state = store.inspect(campaign_id)?.ok_or("campaign not found")?;
            let decision = state.next_action().map_err(diagnostic)?;
            Ok(CampaignDriveResult {
                state,
                decision,
                effect: Some(effect),
            })
        }
        Some(observation) => {
            let version = store.version(campaign_id)?.ok_or("campaign not found")?;
            let state = store.apply_owner_observation(
                campaign_id,
                version,
                &effect.effect_id,
                observation.result,
                observation.evidence_identity,
                observation.member,
            )?;
            let decision = state.next_action().map_err(diagnostic)?;
            Ok(CampaignDriveResult {
                state,
                decision,
                effect: Some(effect),
            })
        }
    }
}

/// Native steps around each bounded campaign operation. The loop owns their
/// order: refresh owner observations, reconcile or claim for the selected
/// decision, then one intent-before-effect operation.
pub trait CampaignDriveShell: CampaignEffectConsumer {
    /// Admit fresh owner observations before selection.
    fn refresh(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        version: u64,
    ) -> Result<u64, String>;
    /// Persist reconciliation intent if this executing member's dispatch is
    /// ambiguous; returns the new store version when intent was persisted.
    fn recover_executing(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        version: u64,
        state: &CampaignState,
        member: &str,
    ) -> Result<Option<u64>, String>;
    /// Claim or reattach the canonical work for a selected writer operation.
    /// Canonical admission owns the decision; a refusal ends the drive as a
    /// capacity wait.
    fn claim(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        version: u64,
        state: &CampaignState,
        member: &str,
    ) -> Result<CampaignClaim, String>;
}

/// What canonical admission decided for a selected writer operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CampaignClaim {
    /// The claim is bound and the campaign store is at this version.
    Bound(u64),
    /// Admission refused it: another winner, a busy repository writer, or a
    /// budget or lineage refusal. Nothing was persisted.
    Refused(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CampaignDriveStep {
    pub decision: CampaignDecision,
    pub effect: Option<PendingEffect>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignDriveRun {
    pub version: u64,
    pub state: CampaignState,
    pub steps: Vec<CampaignDriveStep>,
    /// Canonical admission refused the selected claim; no intent was persisted.
    pub refusal: Option<String>,
}

// r[impl kiln.implementation.campaign.independent_progress.scenario.driver]
/// Run at most `maximum_operations` operations. The drive stops after an
/// operation without an effect, after an effect whose outcome is still
/// unobserved, at a refused claim, and at a blocked, replan, decision-request
/// or complete decision.
pub fn drive_bounded<S: CampaignDriveShell>(
    store: &mut CampaignStore,
    campaign_id: &str,
    mut version: u64,
    maximum_operations: u32,
    shell: &mut S,
) -> Result<CampaignDriveRun, String> {
    let mut steps = Vec::new();
    let mut latest = None;
    let mut refusal = None;
    for _ in 0..maximum_operations {
        version = shell.refresh(store, campaign_id, version)?;
        version = match prepare_selected(store, campaign_id, version, shell)? {
            CampaignClaim::Bound(version) => version,
            CampaignClaim::Refused(reason) => {
                refusal = Some(reason);
                break;
            }
        };
        let result = drive_one(store, campaign_id, version, shell)?;
        version = store
            .version(campaign_id)?
            .ok_or("campaign disappeared after native operation")?;
        let unresolved = result.effect.as_ref().is_some_and(|effect| {
            result
                .state
                .pending_effects
                .get(&effect.effect_id)
                .is_some_and(|retained| !retained.progress_applied)
        });
        let stop = unresolved
            || result.effect.is_none()
            || matches!(
                result.decision.operation,
                CampaignOperation::Blocked
                    | CampaignOperation::Replan
                    | CampaignOperation::RequestDecision
                    | CampaignOperation::Complete
            );
        steps.push(CampaignDriveStep {
            decision: result.decision,
            effect: result.effect,
        });
        latest = Some(result.state);
        if stop {
            break;
        }
    }
    let state = match latest {
        Some(state) => state,
        None => store
            .inspect(campaign_id)?
            .ok_or("bounded controller produced no state observation")?,
    };
    Ok(CampaignDriveRun {
        version,
        state,
        steps,
        refusal,
    })
}

/// Claim for a selected writer operation. When no member can act, check every
/// executing member for an ambiguous dispatch, whichever member's hold is
/// surfaced, so one member's wait cannot defer another member's recovery.
fn prepare_selected<S: CampaignDriveShell>(
    store: &mut CampaignStore,
    campaign_id: &str,
    version: u64,
    shell: &mut S,
) -> Result<CampaignClaim, String> {
    let state = store.inspect(campaign_id)?.ok_or("campaign not found")?;
    let evaluation = state.evaluate();
    let decision = &evaluation.decision;
    if let Some(member) = decision
        .member
        .as_deref()
        .filter(|_| decision.operation.requires_writer_claim())
    {
        return shell.claim(store, campaign_id, version, &state, member);
    }
    if evaluation.basis != SelectionBasis::MemberHold {
        return Ok(CampaignClaim::Bound(version));
    }
    for hold in evaluation
        .holds
        .iter()
        .filter(|hold| hold.operation == CampaignOperation::Wait)
    {
        let Some(member) = hold.member.as_deref() else {
            continue;
        };
        let executing = state
            .observations
            .get(member)
            .is_some_and(|observation| observation.stage == MemberStage::Executing);
        if executing
            && let Some(recovered) =
                shell.recover_executing(store, campaign_id, version, &state, member)?
        {
            return Ok(CampaignClaim::Bound(recovered));
        }
    }
    Ok(CampaignClaim::Bound(version))
}

// r[impl kiln.implementation.campaign.continuation.scenario.rescan]
/// Classify one bounded driver batch. A stale version and a refused claim are
/// not failures: the first needs a reread, the second waits for capacity.
#[must_use]
pub fn classify_batch(
    result: &Result<CampaignDriveRun, String>,
    maximum_operations: u32,
) -> BatchOutcome {
    let run = match result {
        Ok(run) => run,
        Err(error) if error.starts_with(STALE_CAMPAIGN_VERSION) => {
            return BatchOutcome::StaleVersion;
        }
        Err(_) => return BatchOutcome::Failed,
    };
    if run.refusal.is_some() {
        return BatchOutcome::AdmissionRefused;
    }
    let state = &run.state;
    if state.complete() {
        return BatchOutcome::Complete;
    }
    if state
        .pending_effects
        .values()
        .any(|effect| !effect.progress_applied)
    {
        return BatchOutcome::AwaitingOwner;
    }
    if state.cancelled {
        return BatchOutcome::Cancelled;
    }
    if state.paused {
        return BatchOutcome::Paused;
    }
    let decision = state.evaluate().decision;
    let exhausted = run.steps.len() >= usize::try_from(maximum_operations).unwrap_or(usize::MAX);
    if exhausted && decision.operation.is_owner_effect() {
        return BatchOutcome::OperationLimit;
    }
    if decision.operation == CampaignOperation::Replan {
        return BatchOutcome::NeedsReplan;
    }
    BatchOutcome::Waiting
}

/// One batch outcome to merge into the continuation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordedBatch {
    pub outcome: BatchOutcome,
    /// Campaign store version after the batch.
    pub store_version: u64,
    /// Demand sequence read before the batch started.
    pub consumed_sequence: u64,
    pub finished_ms: u64,
}

/// One driver batch run by [`continue_bounded`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContinuationBatch {
    pub reason: DriveReason,
    pub outcome: BatchOutcome,
    /// Campaign store version after the batch.
    pub store_version: u64,
    pub steps: Vec<CampaignDriveStep>,
    pub refusal: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContinuationRun {
    pub batches: Vec<ContinuationBatch>,
    /// The step after the last batch. A `drive` step here means the batch
    /// bound ended this invocation while work was still eligible.
    pub next: ContinuationStep,
    pub record: ContinuationRecord,
    pub record_version: u64,
}

// r[impl kiln.implementation.campaign.continuation.scenario.resume]
/// One bounded continuation step over the existing driver. Every decision
/// rereads canonical state, at most `policy.max_batches` batches run, and the
/// outcome of each is merged into the durable continuation record.
pub fn continue_bounded<S, C>(
    store: &mut CampaignStore,
    campaign_id: &str,
    policy: &ContinuationPolicy,
    maximum_operations: u32,
    clock: &mut C,
    shell: &mut S,
) -> Result<ContinuationRun, String>
where
    S: CampaignDriveShell,
    C: FnMut() -> Result<u64, String>,
{
    policy.validate().map_err(diagnostic)?;
    let batch_bound = usize::try_from(policy.max_batches).unwrap_or(usize::MAX);
    let mut batches = Vec::new();
    loop {
        let now_ms = clock()?;
        let (record, record_version) = store.continuation(campaign_id)?;
        let state = store.inspect(campaign_id)?.ok_or("campaign not found")?;
        let store_version = store.version(campaign_id)?.ok_or("campaign not found")?;
        let next = record.decide(CampaignView::of(&state, store_version), now_ms, policy);
        let reason = match next {
            ContinuationStep::Drive { reason } if batches.len() < batch_bound => reason,
            _ => {
                return Ok(ContinuationRun {
                    batches,
                    next,
                    record,
                    record_version,
                });
            }
        };
        let consumed_sequence = record.demand_sequence;
        let result = drive_bounded(store, campaign_id, store_version, maximum_operations, shell);
        let outcome = classify_batch(&result, maximum_operations);
        let after = store
            .version(campaign_id)?
            .ok_or("campaign disappeared during continuation")?;
        let finished_ms = clock()?;
        store.record_batch(
            campaign_id,
            RecordedBatch {
                outcome,
                store_version: after,
                consumed_sequence,
                finished_ms,
            },
            policy,
        )?;
        let (steps, refusal, error) = match result {
            Ok(run) => (run.steps, run.refusal, None),
            Err(error) => (Vec::new(), None, Some(error)),
        };
        batches.push(ContinuationBatch {
            reason,
            outcome,
            store_version: after,
            steps,
            refusal,
            error,
        });
    }
}

fn load_continuation(
    connection: &Connection,
    campaign_id: &str,
) -> Result<(ContinuationRecord, u64), String> {
    let row: Option<(i64, i64, Option<Vec<u8>>)> = connection
        .query_row(
            "SELECT version,length(bytes),CASE WHEN length(bytes)<=?2 THEN bytes ELSE NULL END FROM implementation_campaign_continuation WHERE campaign_id=?1",
            params![campaign_id, MAX_CONTINUATION_BYTES],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(text)?;
    let Some((version, length, bytes)) = row else {
        return Ok((ContinuationRecord::new(campaign_id), 0));
    };
    let (Some(bytes), true) = (bytes, (0..=MAX_CONTINUATION_BYTES).contains(&length)) else {
        return Err("continuation record exceeds its bound".into());
    };
    let record: ContinuationRecord = decode(&bytes)?;
    record.validate().map_err(diagnostic)?;
    if record.campaign_id != campaign_id {
        return Err("continuation key and payload identity differ".into());
    }
    Ok((record, u64::try_from(version).map_err(text)?))
}

fn save_continuation(
    tx: &rusqlite::Transaction<'_>,
    record: &ContinuationRecord,
    version: u64,
) -> Result<u64, String> {
    let bytes = serde_json::to_vec(record).map_err(text)?;
    if bytes.len() as i64 > MAX_CONTINUATION_BYTES {
        return Err("continuation record exceeds its bound".into());
    }
    let next = version
        .checked_add(1)
        .ok_or("continuation version exhausted")?;
    tx.execute(
        "INSERT INTO implementation_campaign_continuation(campaign_id,version,bytes) VALUES(?1,?2,?3) ON CONFLICT(campaign_id) DO UPDATE SET version=excluded.version,bytes=excluded.bytes",
        params![record.campaign_id, i64::try_from(next).map_err(text)?, bytes],
    )
    .map_err(text)?;
    Ok(next)
}

fn intent_identity(
    campaign_id: &str,
    state: &CampaignState,
    decision: &CampaignDecision,
    member: &str,
) -> Result<ContentId, String> {
    let observation = state
        .observations
        .get(member)
        .ok_or("member observation is missing")?;
    let bytes = serde_json::to_vec(&(
        campaign_id,
        &state.selection_identity,
        state.pending_effects.len(),
        member,
        decision.operation,
        &observation.source_identity,
        &observation.work_identity,
        &observation.attempt_identity,
        observation.attempt_fence,
        &observation.subject_identity,
        &observation.policy_identity,
        &observation.context_identity,
        &observation.native_context_identity,
        &observation.profile_identity,
        &observation.integration_target_identity,
    ))
    .map_err(text)?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string()).map_err(diagnostic)
}

fn load_tx(
    tx: &rusqlite::Transaction<'_>,
    campaign_id: &str,
) -> Result<(CampaignState, u64), String> {
    load_tx_optional(tx, campaign_id)?.ok_or_else(|| "campaign not found".to_owned())
}
fn load_tx_optional(
    tx: &rusqlite::Transaction<'_>,
    campaign_id: &str,
) -> Result<Option<(CampaignState, u64)>, String> {
    let row: Option<(i64, Vec<u8>)> = tx
        .query_row(
            "SELECT version,bytes FROM implementation_campaign WHERE campaign_id=?1",
            params![campaign_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(text)?;
    let Some((version, bytes)) = row else {
        return Ok(None);
    };
    let version = u64::try_from(version).map_err(text)?;
    let state: CampaignState = decode(&bytes)?;
    if state.contract.campaign_id != campaign_id {
        return Err("campaign key and payload identity differ".into());
    }
    Ok(Some((state, version)))
}

/// Why one in-transaction campaign transition did not apply.
#[derive(Debug)]
enum TransitionFailure {
    Missing,
    Stale {
        expected: u64,
        current: u64,
    },
    /// The owner's domain rules refused the transition.
    Owner(kiln_core::types::Diagnostic),
    Storage(String),
}

impl TransitionFailure {
    fn text(self) -> String {
        match self {
            Self::Missing => "campaign not found".into(),
            Self::Stale { expected, current } => {
                format!("{STALE_CAMPAIGN_VERSION}: expected {expected}, current {current}")
            }
            Self::Owner(error) => diagnostic(error),
            Self::Storage(error) => error,
        }
    }
}

impl From<String> for TransitionFailure {
    fn from(error: String) -> Self {
        Self::Storage(error)
    }
}

fn owner_refusal(code: &str, field: &str, message: &str) -> TransitionFailure {
    TransitionFailure::Owner(kiln_core::types::Diagnostic::new(code, field, message))
}

/// The campaign at exactly `expected_version`.
fn current_in(
    tx: &rusqlite::Transaction<'_>,
    campaign_id: &str,
    expected_version: u64,
) -> Result<(CampaignState, u64), TransitionFailure> {
    let (state, version) = load_tx_optional(tx, campaign_id)?.ok_or(TransitionFailure::Missing)?;
    if version != expected_version {
        return Err(TransitionFailure::Stale {
            expected: expected_version,
            current: version,
        });
    }
    Ok((state, version))
}

/// Apply one transition inside the caller's transaction and, with it,
/// coalesce a wake hint for the version it produced.
fn transition_in(
    tx: &rusqlite::Transaction<'_>,
    campaign_id: &str,
    expected_version: u64,
    hint: Option<WakeKind>,
    apply: impl FnOnce(CampaignState) -> Result<CampaignState, kiln_core::types::Diagnostic>,
) -> Result<CampaignState, TransitionFailure> {
    let (state, version) = current_in(tx, campaign_id, expected_version)?;
    let next = apply(state).map_err(TransitionFailure::Owner)?;
    if next.contract.campaign_id != campaign_id {
        return Err(TransitionFailure::Storage(
            "campaign mutation changed identity".into(),
        ));
    }
    save(tx, &next, version)?;
    if let Some(kind) = hint {
        let (mut record, record_version) = load_continuation(tx, campaign_id)?;
        record.record_hint(WakeHint {
            kind,
            observed_version: version.checked_add(1),
        });
        save_continuation(tx, &record, record_version)?;
    }
    Ok(next)
}

fn start_in(
    tx: &rusqlite::Transaction<'_>,
    contract: CampaignContract,
) -> Result<CampaignState, TransitionFailure> {
    let state = CampaignState::start(contract).map_err(TransitionFailure::Owner)?;
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM implementation_campaign WHERE campaign_id=?1)",
            params![state.contract.campaign_id],
            |r| r.get(0),
        )
        .map_err(text)?;
    if exists {
        return Err(owner_refusal(
            "campaign_exists",
            "contract.campaign_id",
            "campaign already exists; use an explicit higher-revision supersession",
        ));
    }
    save(tx, &state, 0)?;
    Ok(state)
}

/// Outstanding decisions across every campaign that shares this campaign's
/// parent-budget scope; those campaigns must share one outstanding bound.
fn scope_outstanding(
    tx: &rusqlite::Transaction<'_>,
    state: &CampaignState,
) -> Result<usize, TransitionFailure> {
    let mut statement = tx
        .prepare("SELECT bytes FROM implementation_campaign")
        .map_err(text)?;
    let rows = statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))
        .map_err(text)?;
    let mut outstanding = 0usize;
    for row in rows {
        let other: CampaignState = decode(&row.map_err(text)?)?;
        if other.contract.parent_budget_id != state.contract.parent_budget_id {
            continue;
        }
        if other.contract.max_outstanding_decisions != state.contract.max_outstanding_decisions {
            return Err(owner_refusal(
                "campaign_decision_scope_bound",
                "contract.max_outstanding_decisions",
                "campaigns sharing a parent budget scope must use one outstanding-request bound",
            ));
        }
        outstanding += other
            .decisions
            .values()
            .filter(|d| d.state == DecisionRequestState::Outstanding)
            .count();
    }
    Ok(outstanding)
}

fn issue_in(
    tx: &rusqlite::Transaction<'_>,
    campaign_id: &str,
    expected_version: u64,
    identity: &ContentId,
) -> Result<CampaignState, TransitionFailure> {
    let (mut state, version) = current_in(tx, campaign_id, expected_version)?;
    let outstanding = scope_outstanding(tx, &state)?;
    // The campaign's own rules (closed admission, its bound, the request's
    // state) refuse first; the shared parent-budget bound is checked after.
    state
        .issue_decision(identity)
        .map_err(TransitionFailure::Owner)?;
    if outstanding >= state.contract.max_outstanding_decisions as usize {
        return Err(owner_refusal(
            "campaign_decision_queue_full",
            "decision",
            "shared parent-budget decision queue is full",
        ));
    }
    save(tx, &state, version)?;
    Ok(state)
}
fn save(tx: &rusqlite::Transaction<'_>, state: &CampaignState, version: u64) -> Result<(), String> {
    let bytes = serde_json::to_vec(state).map_err(text)?;
    if bytes.len() as i64 > MAX_CAMPAIGN_BYTES {
        return Err("campaign state exceeds persistence bound".into());
    }
    let next =
        i64::try_from(version.checked_add(1).ok_or("campaign version exhausted")?).map_err(text)?;
    tx.execute("INSERT INTO implementation_campaign(campaign_id,version,bytes) VALUES(?1,?2,?3) ON CONFLICT(campaign_id) DO UPDATE SET version=excluded.version,bytes=excluded.bytes",params![state.contract.campaign_id,next,bytes]).map_err(text)?;
    Ok(())
}
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(text)
}
fn effect_id(
    campaign: &str,
    member: &str,
    operation: &CampaignOperation,
    intent: &ContentId,
) -> Result<ContentId, String> {
    let bytes = serde_json::to_vec(&(campaign, member, operation, intent)).map_err(text)?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string()).map_err(diagnostic)
}
fn text(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn diagnostic(e: kiln_core::types::Diagnostic) -> String {
    format!("{}: {}", e.code, e.message)
}
fn reject_owner_output(
    state: &mut CampaignState,
    effect_id: &ContentId,
    evidence: ContentId,
) -> Result<(), kiln_core::types::Diagnostic> {
    let effect = state.pending_effects.get_mut(effect_id).ok_or_else(|| {
        kiln_core::types::Diagnostic::new(
            "campaign_effect_missing",
            "effect",
            "effect intent is not retained",
        )
    })?;
    effect.state = EffectState::RejectedStale;
    effect.progress_applied = true;
    effect.evidence_identity = Some(evidence);
    Ok(())
}

#[path = "campaign/commands.rs"]
mod commands;
pub use commands::{
    AdmissionState, AppliedCommand, DecisionOutcome, MAX_OPERATOR_COMMANDS, MAX_REQUEST_ID_BYTES,
    MIN_REQUEST_ID_BYTES, OPERATOR_COMMAND_SCHEMA, OperatorCommand, OperatorCommandKind,
    OperatorFailure, OperatorIntent, OperatorOutcome, valid_request_id,
};

#[cfg(test)]
#[path = "campaign/command_tests.rs"]
mod command_tests;

#[cfg(test)]
#[path = "campaign/owner_scope_tests.rs"]
mod owner_scope_tests;

#[cfg(test)]
#[path = "campaign/drive_tests.rs"]
mod drive_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::campaign::*;
    #[test]
    fn campaign_and_intent_survive_reopen_before_effect() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = dir.path().join("implementation.sqlite");
        let contract = CampaignContract {
            schema: CAMPAIGN_SCHEMA.into(),
            campaign_id: "durable".into(),
            revision: 1,
            repository: "repo".into(),
            members: vec![MemberBinding {
                selector: MemberSelector::WholeChange { change: "c".into() },
                profile: WorkflowProfile::NoSpecDelta,
                profile_version: id(),
                repository_instructions: id(),
                policy: id(),
                required_checks: vec![],
                required_reviews: vec![],
                integration_policy: id(),
                goal: CompletionGoal::VerifiedCandidate,
                prerequisite_inclusion: false,
                context: vec![],
            }],
            max_concurrency: 1,
            max_outstanding_decisions: 1,
            parent_budget_id: id(),
            agent_definition: id(),
            require_one_writer_per_repository: true,
        };
        let mut store = CampaignStore::open(&path).unwrap_or_else(|_| unreachable!());
        let state = store.start(contract).unwrap_or_else(|_| unreachable!());
        drop(store);
        let store = CampaignStore::open(&path).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            store
                .inspect("durable")
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!()),
            state
        );
    }
    #[test]
    fn campaign_observation_does_not_create_schema() {
        let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = dir.path().join("campaign.sqlite");
        drop(Connection::open(&path).unwrap_or_else(|_| unreachable!()));
        let observer = CampaignObservation::open(&path).unwrap_or_else(|_| unreachable!());
        assert!(observer.inspect("absent").is_err());
        drop(observer);
        let connection = Connection::open(&path).unwrap_or_else(|_| unreachable!());
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(-1);
        assert_eq!(count, 0);
    }
    fn id() -> ContentId {
        ContentId::from_blake3_hex("b".repeat(64)).unwrap_or_else(|_| unreachable!())
    }
}
