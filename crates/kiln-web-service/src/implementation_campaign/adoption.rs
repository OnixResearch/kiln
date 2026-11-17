//! Adoption of already implemented work for completion-only members (#47).
//!
//! A completion-only member schedules no coding work. It adopts canonical
//! evidence only when every required native unit was verified with the
//! authoritative Cairn completion check, integrated through the selected
//! delivery route, and remains applicable to the current target. Anything less
//! stays an explicit blocker naming the missing or stale fact.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kiln_core::change_review::CiCheck;
use kiln_core::implementation::campaign::{Freshness, MemberObservation};
use kiln_core::implementation::{
    AttemptPhase, ImplementationAttempt, ImplementationWork, WorkSourceKind,
    admit_candidate_handoff,
};
use kiln_core::types::ContentId;
use kiln_implementer::campaign_consumer::CanonicalCampaignLedgers;

use super::show;

/// The canonical CI check that records authoritative Cairn completion.
const COMPLETION_CHECK: &str = "cairn-authoritative-completion";
const COMPLETION_KIND: &str = "cairn-agent-completion-v2";
const PASSING: &str = "pass";
/// Where a native Cairn change keeps its source in the target tree.
const CHANGES_ROOT: &str = ".cairn/changes";

/// An integrated result proven through the selected delivery route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Integration {
    /// The integrated result commit.
    pub commit: String,
    /// Identity of the retained integration evidence.
    pub evidence: ContentId,
}

/// What one required native unit's canonical record proves.
#[derive(Clone, Debug)]
pub(super) struct UnitEvidence<'a> {
    pub native_id: String,
    /// Current canonical work naming the unit, with its identity.
    pub work: Option<(&'a ImplementationWork, ContentId)>,
    /// Latest attempt of that work's canonical key.
    pub attempt: Option<&'a ImplementationAttempt>,
    pub integration: Option<Integration>,
}

/// Facts read from the delivery route's target repository.
pub(super) trait Applicability {
    /// The integrated `commit` is the current target or one of its ancestors.
    fn contained(&self, commit: &str) -> Result<bool, String>;
    /// The change source at the current target is the same as at `commit`.
    fn unchanged_since(&self, commit: &str) -> Result<bool, String>;
    /// `later` is `earlier` or descends from it.
    fn descends(&self, later: &str, earlier: &str) -> Result<bool, String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Adoption {
    /// Every unit is adoptable; bind the unit at this index.
    Adopted { binding: usize },
    /// Keep the member blocked for this reason.
    Blocked(String),
}

// r[impl kiln.implementation.campaign.adoption]
/// Decide whether a completion-only member adopts its units' evidence.
pub(super) fn decide(units: &[UnitEvidence<'_>], target: &impl Applicability) -> Adoption {
    if units.is_empty() {
        return Adoption::Blocked("completion-only member names no required native unit".into());
    }
    for unit in units {
        if let Some(reason) = refusal(unit, target) {
            return Adoption::Blocked(format!("{}: {reason}", unit.native_id));
        }
    }
    // Bind the integrated result that contains every other one.
    for (index, unit) in units.iter().enumerate() {
        let Some(candidate) = &unit.integration else {
            continue;
        };
        let mut covers = true;
        for other in units.iter().filter_map(|other| other.integration.as_ref()) {
            match target.descends(&candidate.commit, &other.commit) {
                Ok(true) => {}
                Ok(false) => {
                    covers = false;
                    break;
                }
                Err(error) => {
                    return Adoption::Blocked(format!(
                        "ancestry of integrated result {} could not be read: {error}",
                        candidate.commit
                    ));
                }
            }
        }
        if covers {
            return Adoption::Adopted { binding: index };
        }
    }
    Adoption::Blocked("adoptable integrated results diverge; none contains every other".into())
}

/// Why one unit's evidence cannot be adopted, or `None` when it can.
fn refusal(unit: &UnitEvidence<'_>, target: &impl Applicability) -> Option<String> {
    let Some((_, work_identity)) = &unit.work else {
        return Some(
            "no current canonical work names this unit; its implementation was not admitted through Kiln"
                .into(),
        );
    };
    let Some(attempt) = unit.attempt else {
        return Some("the current canonical work has no attempt".into());
    };
    if attempt.work_identity != *work_identity {
        return Some("the latest attempt is bound to another work version".into());
    }
    let verification = match &attempt.verification {
        Some(verification)
            if attempt.phase == AttemptPhase::Verified && verification.run.success =>
        {
            verification
        }
        _ => return Some("the latest attempt is not independently verified".into()),
    };
    let completions: Vec<&CiCheck> = verification
        .run
        .checks
        .iter()
        .filter(|check| check.name == COMPLETION_CHECK)
        .collect();
    if !matches!(
        completions.as_slice(),
        [check] if check.outcome == PASSING && check.kind == COMPLETION_KIND
    ) {
        return Some(
            "the verification lacks exactly one passing authoritative Cairn completion check"
                .into(),
        );
    }
    let Some(integration) = &unit.integration else {
        return Some(
            "no integration of this exact attempt was observed through the selected route".into(),
        );
    };
    match target.contained(&integration.commit) {
        Ok(true) => {}
        Ok(false) => {
            return Some(format!(
                "integrated result {} is not contained in the current target",
                integration.commit
            ));
        }
        Err(error) => {
            return Some(format!(
                "containment of integrated result {} could not be read: {error}",
                integration.commit
            ));
        }
    }
    match target.unchanged_since(&integration.commit) {
        Ok(true) => None,
        Ok(false) => Some(format!(
            "the change source moved after integrated result {}",
            integration.commit
        )),
        Err(error) => Some(format!(
            "the change source at integrated result {} could not be read: {error}",
            integration.commit
        )),
    }
}

/// Read each required unit's current canonical work in `repository`, the
/// latest attempt of its canonical key and that attempt's route integration.
pub(super) fn unit_evidence<'a>(
    ledgers: &'a CanonicalCampaignLedgers,
    repository: &str,
    units: &BTreeSet<String>,
    mut integration_of: impl FnMut(&ImplementationAttempt) -> Result<Option<Integration>, String>,
) -> Result<Vec<UnitEvidence<'a>>, String> {
    let mut evidence = Vec::with_capacity(units.len());
    for native_id in units {
        let named = ledgers.work.current().find(|work| {
            work.repository == repository
                && work.aliases.iter().any(|alias| {
                    alias.kind == WorkSourceKind::CairnTask && alias.native_id == *native_id
                })
        });
        let (work, attempt) = match named {
            Some(work) => {
                let key = work.canonical_key().map_err(show)?;
                let latest = ledgers
                    .attempts
                    .attempts()
                    .filter(|attempt| attempt.canonical_key == key)
                    .max_by_key(|attempt| (attempt.ordinal, attempt.fence));
                (Some((work, work.identity().map_err(show)?)), latest)
            }
            None => (None, None),
        };
        let integration = match attempt {
            Some(attempt) if work.is_some() => integration_of(attempt)?,
            _ => None,
        };
        evidence.push(UnitEvidence {
            native_id: native_id.clone(),
            work,
            attempt,
            integration,
        });
    }
    Ok(evidence)
}

/// The facts an adopted member binds, owned so they outlive the ledgers and
/// survive the observation's per-work reset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Adopted {
    work_identity: ContentId,
    canonical_key: ContentId,
    source_identity: Option<ContentId>,
    attempt: ContentId,
    fence: u64,
    subject: Option<ContentId>,
    checks: Vec<CiCheck>,
    integration: ContentId,
    units: BTreeSet<String>,
}

impl Adopted {
    /// The binding of an adopted decision over `units`.
    pub(super) fn new(units: &[UnitEvidence<'_>], binding: usize) -> Result<Self, String> {
        let unit = units
            .get(binding)
            .ok_or("adoption binding is outside its units")?;
        let (Some((work, work_identity)), Some(attempt), Some(integration)) =
            (&unit.work, unit.attempt, &unit.integration)
        else {
            return Err("adopted unit lost its canonical evidence".into());
        };
        Ok(Self {
            work_identity: work_identity.clone(),
            canonical_key: work.canonical_key().map_err(show)?,
            source_identity: work
                .aliases
                .iter()
                .find(|alias| alias.kind == WorkSourceKind::CairnTask)
                .map(|alias| alias.version.clone()),
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            subject: attempt
                .handoff
                .as_ref()
                .map(|handoff| admit_candidate_handoff(work, attempt, handoff).map_err(show))
                .transpose()?,
            checks: attempt
                .verification
                .as_ref()
                .map(|verification| verification.run.checks.clone())
                .unwrap_or_default(),
            integration: integration.evidence.clone(),
            units: units.iter().map(|unit| unit.native_id.clone()).collect(),
        })
    }

    /// The adopted work, set where the stage is decided.
    pub(super) fn identify(&self, member: &mut MemberObservation) {
        member.work_identity = Some(self.work_identity.clone());
        member.canonical_work_key = Some(self.canonical_key.clone());
        member.source_identity = self.source_identity.clone();
    }

    /// The attempt facts the verification and delivery owners record, applied
    /// after the per-work reset. Adopted units leave the remaining obligations.
    pub(super) fn bind(&self, member: &mut MemberObservation, required_checks: &[String]) {
        member.attempt_identity = Some(self.attempt.clone());
        member.attempt_fence = Some(self.fence);
        member.subject_identity = self.subject.clone();
        member.candidate_freshness = Freshness::Current;
        member.verifier_freshness = Freshness::Current;
        member.required_checks_satisfied =
            super::required_checks_present(required_checks, &self.checks);
        member.recovery = None;
        let artifacts = self
            .checks
            .iter()
            .filter_map(|check| check.artifact_digest.as_deref())
            .filter_map(|digest| ContentId::from_blake3_hex(digest.to_owned()).ok());
        for identity in artifacts.chain(std::iter::once(self.integration.clone())) {
            if !member.evidence.contains(&identity) {
                member.evidence.push(identity);
            }
        }
        for unit in &self.units {
            member.remaining_obligations.remove(unit);
        }
    }
}

/// Containment and change-source facts read from one target repository.
pub(super) struct TargetHistory {
    repository: git2::Repository,
    target: git2::Oid,
    change: PathBuf,
}

impl TargetHistory {
    pub(super) fn open(repository: &Path, target: &str, change: &str) -> Result<Self, String> {
        let repository = git2::Repository::open(repository)
            .map_err(|error| format!("the target repository is unavailable: {error}"))?;
        let target = oid(target)?;
        Ok(Self {
            repository,
            target,
            change: Path::new(CHANGES_ROOT).join(change),
        })
    }

    /// The change source's tree entry at `commit`, or `None` when absent.
    fn change_entry(&self, commit: git2::Oid) -> Result<Option<git2::Oid>, String> {
        let tree = self
            .repository
            .find_commit(commit)
            .and_then(|commit| commit.tree())
            .map_err(|error| error.to_string())?;
        match tree.get_path(&self.change) {
            Ok(entry) => Ok(Some(entry.id())),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    /// Both commits exist here, and `later` is `earlier` or descends from it.
    fn descendant(&self, later: git2::Oid, earlier: git2::Oid) -> Result<bool, String> {
        for commit in [later, earlier] {
            self.repository.find_commit(commit).map_err(|error| {
                format!("commit {commit} is not available in the target repository: {error}")
            })?;
        }
        Ok(later == earlier
            || self
                .repository
                .graph_descendant_of(later, earlier)
                .map_err(|error| error.to_string())?)
    }
}

fn oid(value: &str) -> Result<git2::Oid, String> {
    git2::Oid::from_str(value).map_err(|error| format!("commit {value} is malformed: {error}"))
}

impl Applicability for TargetHistory {
    fn contained(&self, commit: &str) -> Result<bool, String> {
        self.descendant(self.target, oid(commit)?)
    }

    fn unchanged_since(&self, commit: &str) -> Result<bool, String> {
        // An absent change source at either commit is not proof of anything.
        Ok(
            match (
                self.change_entry(oid(commit)?)?,
                self.change_entry(self.target)?,
            ) {
                (Some(before), Some(now)) => before == now,
                _ => false,
            },
        )
    }

    fn descends(&self, later: &str, earlier: &str) -> Result<bool, String> {
        self.descendant(oid(later)?, oid(earlier)?)
    }
}

#[cfg(test)]
#[path = "adoption_tests.rs"]
mod tests;
