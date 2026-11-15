//! Durable selected-campaign state and intent-before-effect controller.
use std::{path::Path, time::Duration};

use kiln_core::{
    implementation::campaign::{
        CampaignContract, CampaignDecision, CampaignOperation, CampaignState, EffectState,
        MemberObservation, PendingEffect,
    },
    types::ContentId,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::de::DeserializeOwned;

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

const MAX_CAMPAIGN_BYTES: i64 = 32 * 1024 * 1024;
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS implementation_campaign (campaign_id TEXT PRIMARY KEY, version INTEGER NOT NULL, bytes BLOB NOT NULL);";

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
        let state = CampaignState::start(contract).map_err(diagnostic)?;
        let id = state.contract.campaign_id.clone();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM implementation_campaign WHERE campaign_id=?1)",
                params![id],
                |r| r.get(0),
            )
            .map_err(text)?;
        if exists {
            return Err(
                "campaign already exists; use an explicit higher-revision supersession".into(),
            );
        }
        save(&tx, &state, 0)?;
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
                "stale campaign version: expected {expected_version}, current {version}"
            ));
        }
        let mut statement = tx
            .prepare("SELECT bytes FROM implementation_campaign")
            .map_err(text)?;
        let rows = statement
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(text)?;
        let mut outstanding = 0usize;
        for row in rows {
            let other: CampaignState = decode(&row.map_err(text)?)?;
            if other.contract.parent_budget_id == state.contract.parent_budget_id {
                if other.contract.max_outstanding_decisions
                    != state.contract.max_outstanding_decisions
                {
                    return Err("campaigns sharing a parent budget scope must use one outstanding-request bound".into());
                }
                outstanding += other
                    .decisions
                    .values()
                    .filter(|d| d.state == DecisionRequestState::Outstanding)
                    .count();
            }
        }
        if request.state == DecisionRequestState::Outstanding
            && outstanding >= state.contract.max_outstanding_decisions as usize
        {
            request.state = DecisionRequestState::Unissued;
            request.pending_reason =
                Some("shared operator decision scope is at its outstanding bound".into());
        }
        drop(statement);
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
        let (mut state, version) = load_tx(&tx, campaign_id)?;
        if version != expected_version {
            return Err(format!(
                "stale campaign version: expected {expected_version}, current {version}"
            ));
        }
        let mut statement = tx
            .prepare("SELECT bytes FROM implementation_campaign")
            .map_err(text)?;
        let rows = statement
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(text)?;
        let mut outstanding = 0usize;
        for row in rows {
            let other: CampaignState = decode(&row.map_err(text)?)?;
            if other.contract.parent_budget_id == state.contract.parent_budget_id {
                if other.contract.max_outstanding_decisions
                    != state.contract.max_outstanding_decisions
                {
                    return Err("campaigns sharing a parent budget scope must use one outstanding-request bound".into());
                }
                outstanding += other
                    .decisions
                    .values()
                    .filter(|d| d.state == DecisionRequestState::Outstanding)
                    .count();
            }
        }
        drop(statement);
        if outstanding >= state.contract.max_outstanding_decisions as usize {
            return Err("shared parent-budget decision queue is full".into());
        }
        state.issue_decision(identity).map_err(diagnostic)?;
        save(&tx, &state, version)?;
        tx.commit().map_err(text)?;
        Ok(state)
    }
    pub fn resolve_decision(
        &mut self,
        campaign_id: &str,
        expected_version: u64,
        answer: CampaignDecisionAnswer<'_>,
    ) -> Result<CampaignState, String> {
        self.mutate(campaign_id, expected_version, |mut state| {
            state.resolve_decision(
                answer.identity,
                answer.work,
                answer.policy,
                answer.candidate,
                answer.answer,
            )?;
            Ok(state)
        })
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
        self.mutate(campaign_id, expected_version, |mut state| {
            state.continue_admission();
            Ok(state)
        })
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
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (state, version) = load_tx(&tx, campaign_id)?;
        if version != expected_version {
            return Err(format!(
                "stale campaign version: expected {expected_version}, current {version}"
            ));
        }
        let next = apply(state).map_err(diagnostic)?;
        let actual = next.contract.campaign_id.clone();
        if actual != campaign_id {
            return Err("campaign mutation changed identity".into());
        }
        save(&tx, &next, version)?;
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
    if !effectful(action.operation) {
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

fn effectful(operation: CampaignOperation) -> bool {
    matches!(
        operation,
        CampaignOperation::AuthorArtifact
            | CampaignOperation::RepairArtifact
            | CampaignOperation::Code
            | CampaignOperation::RepairCode
            | CampaignOperation::RetryInfrastructure
            | CampaignOperation::Verify
            | CampaignOperation::Publish
            | CampaignOperation::Integrate
            | CampaignOperation::Lifecycle
    )
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
    let row: Option<(i64, Option<Vec<u8>>)> = tx
        .query_row(
            "SELECT version,length(bytes),bytes FROM implementation_campaign WHERE campaign_id=?1",
            params![campaign_id],
            |r| Ok((r.get(0)?, Some(r.get(2)?))),
        )
        .optional()
        .map_err(text)?;
    let Some((version, Some(bytes))) = row else {
        return Err("campaign not found".into());
    };
    let version = u64::try_from(version).map_err(text)?;
    let state: CampaignState = decode(&bytes)?;
    if state.contract.campaign_id != campaign_id {
        return Err("campaign key and payload identity differ".into());
    }
    Ok((state, version))
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

#[cfg(test)]
#[path = "campaign/owner_scope_tests.rs"]
mod owner_scope_tests;

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
