//! Pure ownership and lifecycle for an opt-in retained Lattice session.
//!
//! A retained session is not a second VMM supervisor. This module owns only the
//! placement generation, fence checks and legal lifecycle commands that the
//! existing hosted runner and a future migration transport must consult. It
//! performs no I/O and grants no work, retry or landing authority.

use serde::{Deserialize, Serialize};

use kiln_core::types::{ContentId, Diagnostic};

pub const RETAINED_SESSION_SCHEMA: &str = "kiln.retained-session.v1";
pub const MAX_HOST_BYTES: usize = 256;
pub const MAX_CAUSE_BYTES: usize = 512;
pub const MAX_VCPUS: u16 = 256;
pub const MAX_MEMORY_MIB: u32 = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionIdentity {
    pub session: ContentId,
    pub work: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub host: String,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedResources {
    pub vcpus: u16,
    pub memory_mib: u32,
    pub slot: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetainedState {
    Receiving,
    Restoring,
    Paused,
    Running,
    Migrating,
    MigratedAway,
    /// Ownership, confinement or cleanup is unknown; capacity stays occupied.
    ReconcileRequired,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetainedEffect {
    Restore,
    Resume,
    Pause,
    Migrate,
    /// Retire the source after migration or pause, releasing capacity only when
    /// the shell observes cleanup.
    Retire,
    Reconcile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationIntent {
    pub target_host: String,
    pub target_generation: u64,
    pub intent: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerToken {
    pub session: ContentId,
    pub fence: u64,
    pub host: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedSession {
    pub schema: String,
    pub identity: SessionIdentity,
    pub placement: Placement,
    pub resources: RetainedResources,
    pub state: RetainedState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration: Option<MigrationIntent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl RetainedSession {
    /// Admit one retained session owned by the exact placement.
    // r[impl kiln.hosted.retained.lifecycle]
    pub fn admit(
        identity: SessionIdentity,
        placement: Placement,
        resources: RetainedResources,
    ) -> Result<Self, Diagnostic> {
        identity.session.validate()?;
        identity.work.validate()?;
        identity.attempt.validate()?;
        validate_host(&placement.host, "retained.host")?;
        if placement.generation == 0
            || identity.fence == 0
            || resources.vcpus == 0
            || resources.vcpus > MAX_VCPUS
            || resources.memory_mib == 0
            || resources.memory_mib > MAX_MEMORY_MIB
            || resources.slot == 0
        {
            return Err(refusal(
                "retained_session_admission",
                "retained.session",
                "generation, fence or resources are zero or over bound",
            ));
        }
        Ok(Self {
            schema: RETAINED_SESSION_SCHEMA.to_string(),
            identity,
            placement,
            resources,
            state: RetainedState::Receiving,
            migration: None,
            cause: None,
        })
    }

    /// Whether this session still needs its occupied capacity retained.
    #[must_use]
    pub fn occupies_capacity(&self) -> bool {
        self.state != RetainedState::Retired
    }

    fn authorize(&self, owner: &OwnerToken) -> Result<(), Diagnostic> {
        owner.session.validate()?;
        validate_host(&owner.host, "retained.owner.host")?;
        if self.schema != RETAINED_SESSION_SCHEMA
            || self.identity.session != owner.session
            || self.identity.fence != owner.fence
            || self.placement.host != owner.host
            || self.placement.generation != owner.generation
        {
            return Err(refusal(
                "retained_owner_stale",
                "retained.owner",
                "owner token does not hold the current placement and fence",
            ));
        }
        if self.state == RetainedState::Retired {
            return Err(refusal(
                "retained_session_retired",
                "retained.state",
                "retired session admits no further commands",
            ));
        }
        Ok(())
    }

    /// Begin restore only from receiving.
    pub fn begin_restore(&mut self, owner: &OwnerToken) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        self.require(RetainedState::Receiving)?;
        self.state = RetainedState::Restoring;
        Ok(RetainedEffect::Restore)
    }

    /// Observation after restore. Failed confinement never reaches workload
    /// effects and requires reconciliation.
    pub fn observe_restored(
        &mut self,
        owner: &OwnerToken,
        confinement_ok: bool,
    ) -> Result<Option<RetainedEffect>, Diagnostic> {
        self.authorize(owner)?;
        self.require(RetainedState::Restoring)?;
        if confinement_ok {
            self.state = RetainedState::Paused;
            Ok(None)
        } else {
            self.state = RetainedState::ReconcileRequired;
            self.cause = Some("confinement_unobserved".to_string());
            Ok(Some(RetainedEffect::Reconcile))
        }
    }

    pub fn resume(&mut self, owner: &OwnerToken) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        self.require(RetainedState::Paused)?;
        self.state = RetainedState::Running;
        Ok(RetainedEffect::Resume)
    }

    pub fn pause(&mut self, owner: &OwnerToken) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        self.require(RetainedState::Running)?;
        self.state = RetainedState::Paused;
        Ok(RetainedEffect::Pause)
    }

    /// Begin one outbound migration from the paused source.
    pub fn begin_migration(
        &mut self,
        owner: &OwnerToken,
        target_host: String,
        target_generation: u64,
        intent: ContentId,
    ) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        self.require(RetainedState::Paused)?;
        validate_host(&target_host, "retained.migration.target")?;
        intent.validate()?;
        if target_generation <= self.placement.generation || target_host == self.placement.host {
            return Err(refusal(
                "retained_migration_target",
                "retained.migration",
                "migration target must advance the placement generation on another host",
            ));
        }
        self.migration = Some(MigrationIntent {
            target_host,
            target_generation,
            intent,
        });
        self.state = RetainedState::Migrating;
        Ok(RetainedEffect::Migrate)
    }

    /// Observe the source as migrated away. The source still needs retirement;
    /// migration success is not placement commit or cleanup.
    pub fn observe_migrated_away(
        &mut self,
        owner: &OwnerToken,
    ) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        self.require(RetainedState::Migrating)?;
        if self.migration.is_none() {
            return Err(refusal(
                "retained_migration_missing",
                "retained.migration",
                "migrated-away observation has no migration intent",
            ));
        }
        self.state = RetainedState::MigratedAway;
        Ok(RetainedEffect::Retire)
    }

    /// Retire a paused or migrated-away source. Running sessions must pause.
    pub fn retire(&mut self, owner: &OwnerToken) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        if !matches!(
            self.state,
            RetainedState::Paused | RetainedState::MigratedAway
        ) {
            return Err(refusal(
                "retained_retire_phase",
                "retained.state",
                "only a paused or migrated-away session may be retired",
            ));
        }
        self.state = RetainedState::Retired;
        Ok(RetainedEffect::Retire)
    }

    /// Mark an uncertain ownership/confinement/cleanup observation. Capacity
    /// stays occupied until a later observed retirement.
    pub fn require_reconciliation(
        &mut self,
        owner: &OwnerToken,
        cause: &str,
    ) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        if cause.is_empty() || cause.len() > MAX_CAUSE_BYTES || cause.chars().any(char::is_control)
        {
            return Err(refusal(
                "retained_cause",
                "retained.cause",
                "retained cause is empty, oversized or malformed",
            ));
        }
        self.state = RetainedState::ReconcileRequired;
        self.cause = Some(cause.to_string());
        Ok(RetainedEffect::Reconcile)
    }

    fn require(&self, expected: RetainedState) -> Result<(), Diagnostic> {
        if self.state != expected {
            return Err(refusal(
                "retained_session_phase",
                "retained.state",
                "retained session phase does not admit this command",
            ));
        }
        Ok(())
    }
}

fn validate_host(host: &str, field: &str) -> Result<(), Diagnostic> {
    if host.is_empty()
        || host.len() > MAX_HOST_BYTES
        || host.chars().any(char::is_control)
        || host.chars().any(char::is_whitespace)
    {
        return Err(refusal(
            "retained_host",
            field,
            "retained host is empty, oversized or malformed",
        ));
    }
    Ok(())
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn identity() -> SessionIdentity {
        SessionIdentity {
            session: content('1'),
            work: content('2'),
            work_revision: 4,
            attempt: content('3'),
            fence: 9,
        }
    }

    fn owner(session: &RetainedSession) -> OwnerToken {
        OwnerToken {
            session: session.identity.session.clone(),
            fence: session.identity.fence,
            host: session.placement.host.clone(),
            generation: session.placement.generation,
        }
    }

    fn session() -> RetainedSession {
        RetainedSession::admit(
            identity(),
            Placement {
                host: "host-a".to_string(),
                generation: 1,
            },
            RetainedResources {
                vcpus: 2,
                memory_mib: 1024,
                slot: 3,
            },
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"))
    }

    fn paused(session: &mut RetainedSession) -> OwnerToken {
        let token = owner(session);
        session
            .begin_restore(&token)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        session
            .observe_restored(&token, true)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        token
    }

    // r[verify kiln.hosted.retained.lifecycle] r[verify kiln.hosted.retained.lifecycle.scenario.restore]
    #[test]
    fn restored_session_is_paused_and_resumes_only_under_the_owner() {
        let mut session = session();
        let token = paused(&mut session);
        assert_eq!(session.state, RetainedState::Paused);
        assert_eq!(
            session
                .resume(&token)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            RetainedEffect::Resume
        );
        assert_eq!(session.state, RetainedState::Running);
    }

    // r[verify kiln.hosted.retained.lifecycle.scenario.confinement]
    #[test]
    fn failed_confinement_requires_reconciliation_and_never_runs() {
        let mut session = session();
        let token = owner(&session);
        session
            .begin_restore(&token)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(
            session
                .observe_restored(&token, false)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            Some(RetainedEffect::Reconcile)
        );
        assert_eq!(session.state, RetainedState::ReconcileRequired);
        assert!(session.occupies_capacity());
        assert!(session.resume(&token).is_err());
    }

    // r[verify kiln.hosted.retained.lifecycle.scenario.stale]
    #[test]
    fn stale_owner_or_generation_cannot_resume_or_retire() {
        let mut session = session();
        let token = paused(&mut session);
        let mut stale = token.clone();
        stale.generation += 1;
        assert!(session.resume(&stale).is_err());
        stale = token.clone();
        stale.fence += 1;
        assert!(session.resume(&stale).is_err());
        assert_eq!(session.state, RetainedState::Paused);
    }

    // r[verify kiln.hosted.retained.lifecycle.scenario.migration]
    #[test]
    fn migrated_away_source_cannot_resume_and_retirement_is_separate() {
        let mut session = session();
        let token = paused(&mut session);
        session
            .begin_migration(&token, "host-b".to_string(), 2, content('4'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(session.state, RetainedState::Migrating);
        assert_eq!(
            session
                .observe_migrated_away(&token)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            RetainedEffect::Retire
        );
        assert_eq!(session.state, RetainedState::MigratedAway);
        assert!(session.resume(&token).is_err());
        assert!(session.occupies_capacity());
        session
            .retire(&token)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(session.state, RetainedState::Retired);
        assert!(!session.occupies_capacity());
        assert!(session.retire(&token).is_err());
    }

    #[test]
    fn retained_session_round_trips_and_rejects_bad_admission() {
        let session = session();
        let json = serde_json::to_string(&session).unwrap_or_else(|_| unreachable!());
        let decoded: RetainedSession =
            serde_json::from_str(&json).unwrap_or_else(|_| unreachable!());
        assert_eq!(decoded, session);
        assert!(
            RetainedSession::admit(
                identity(),
                Placement {
                    host: "host a".to_string(),
                    generation: 1
                },
                RetainedResources {
                    vcpus: 2,
                    memory_mib: 1024,
                    slot: 1
                }
            )
            .is_err()
        );
    }
}
