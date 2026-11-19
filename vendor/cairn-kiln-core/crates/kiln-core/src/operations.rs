use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cmp::Ordering;
use serde::{Deserialize, Serialize};

use crate::domain::{Event, State, evolve};
use crate::types::{
    Alias, CONTRACT_VERSION_V1, ContentId, CoreResult, Diagnostic, EffectId, ExternalRef,
    ProviderProfileId,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct JournalRecord {
    pub schema_version: u16,
    pub stream: ExternalRef,
    pub expected_revision: u64,
    pub event_identity: ExternalRef,
    pub event: Event,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OutboxRecord {
    pub effect_id: EffectId,
    pub provider_profile: ProviderProfileId,
    pub idempotency_key: ExternalRef,
    pub lease: LeaseState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LeaseState {
    Unclaimed,
    Leased,
    Expired,
    Completed,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RecoveryAction {
    Claim,
    Wait,
    Reconcile,
    None,
}

#[must_use]
pub const fn classify_recovery(state: LeaseState) -> RecoveryAction {
    match state {
        LeaseState::Unclaimed => RecoveryAction::Claim,
        LeaseState::Leased => RecoveryAction::Wait,
        LeaseState::Expired | LeaseState::Ambiguous => RecoveryAction::Reconcile,
        LeaseState::Completed => RecoveryAction::None,
    }
}

pub fn replay(initial: &State, records: &[JournalRecord]) -> CoreResult<State> {
    replay_compatible(initial, records, &[CONTRACT_VERSION_V1])
}

pub fn replay_compatible(
    initial: &State,
    records: &[JournalRecord],
    supported_versions: &[u16],
) -> CoreResult<State> {
    let mut state = initial.clone();
    for record in records {
        if !supported_versions.contains(&record.schema_version) {
            return Err(Diagnostic::new(
                "unsupported_history_version",
                "journal.schema_version",
                "journal history version has no admitted deterministic replay path",
            ));
        }
        if record.expected_revision != state.revision {
            return Err(Diagnostic::new(
                "history_revision_mismatch",
                "journal.expected_revision",
                "journal record expected revision does not match replay state",
            ));
        }
        record.stream.validate()?;
        record.event_identity.validate()?;
        state = evolve(&state, &record.event)?;
    }
    Ok(state)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MigrationRecord {
    pub from_version: u16,
    pub to_version: u16,
    pub input_identity: ContentId,
    pub output_identity: ContentId,
    pub migration_profile: ExternalRef,
}

pub fn validate_migration(
    migration: &MigrationRecord,
    admitted_edges: &[(u16, u16)],
) -> CoreResult<()> {
    if !admitted_edges.contains(&(migration.from_version, migration.to_version)) {
        return Err(Diagnostic::new(
            "unadmitted_history_migration",
            "journal.migration",
            "journal migration version edge is not declared",
        ));
    }
    if migration.input_identity == migration.output_identity {
        return Err(Diagnostic::new(
            "history_migration_identity",
            "journal.migration.output_identity",
            "history migration must record a distinct deterministic output identity",
        ));
    }
    migration.migration_profile.validate()?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UsageValue {
    pub amount: u64,
    pub capacity: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TenantUsage {
    pub tenant: Alias,
    pub share: u64,
    pub provider: ProviderProfileId,
    pub interval_start: u64,
    pub interval_end: u64,
    pub observed_at: u64,
    pub resources: BTreeMap<Alias, UsageValue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FairnessStrategy {
    WeightedScalar { weights: BTreeMap<Alias, u64> },
    DominantShare,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Score {
    numerator: u128,
    denominator: u128,
}

pub fn validate_usage_sequence(usage: &[TenantUsage]) -> CoreResult<()> {
    let mut prior: BTreeMap<(Alias, ProviderProfileId), &TenantUsage> = BTreeMap::new();
    for observation in usage {
        let key = (observation.tenant.clone(), observation.provider.clone());
        if let Some(previous) = prior.get(&key) {
            if observation.observed_at < previous.observed_at
                || observation.interval_start < previous.interval_start
                || observation.interval_end < previous.interval_end
            {
                return Err(Diagnostic::new(
                    "usage_regression",
                    "usage",
                    "tenant usage observations regress in time",
                ));
            }
            for (dimension, previous_value) in &previous.resources {
                if let Some(current_value) = observation.resources.get(dimension)
                    && observation.interval_start == previous.interval_start
                    && current_value.amount < previous_value.amount
                {
                    return Err(Diagnostic::new(
                        "usage_amount_regression",
                        "usage.resources",
                        "usage amount regressed within one accounting interval",
                    ));
                }
            }
        }
        prior.insert(key, observation);
    }
    Ok(())
}

pub fn rank_tenants(
    usage: &[TenantUsage],
    strategy: &FairnessStrategy,
    current_time: u64,
    max_age: u64,
) -> CoreResult<Vec<Alias>> {
    validate_usage_sequence(usage)?;
    let mut scored = Vec::with_capacity(usage.len());
    for tenant in usage {
        validate_usage(tenant, current_time, max_age)?;
        scored.push((tenant.tenant.clone(), score(tenant, strategy)?));
    }
    scored.sort_by(compare_score);
    Ok(scored.into_iter().map(|entry| entry.0).collect())
}

fn validate_usage(usage: &TenantUsage, current_time: u64, max_age: u64) -> CoreResult<()> {
    if usage.share == 0 {
        return Err(Diagnostic::new(
            "invalid_share",
            "usage.share",
            "tenant share must be positive",
        ));
    }
    if usage.interval_start > usage.interval_end || usage.observed_at > current_time {
        return Err(Diagnostic::new(
            "time_regression",
            "usage.interval",
            "usage time facts regress or are in the future",
        ));
    }
    if current_time.saturating_sub(usage.observed_at) > max_age {
        return Err(Diagnostic::new(
            "stale_usage",
            "usage.observed_at",
            "usage observation is stale",
        ));
    }
    Ok(())
}

fn score(usage: &TenantUsage, strategy: &FairnessStrategy) -> CoreResult<Score> {
    match strategy {
        FairnessStrategy::WeightedScalar { weights } => {
            let mut total = 0_u128;
            for (dimension, value) in &usage.resources {
                let weight = weights.get(dimension).ok_or(Diagnostic::new(
                    "unsupported_dimension",
                    "usage.resources",
                    "weighted strategy has no weight for a resource dimension",
                ))?;
                let term = u128::from(value.amount)
                    .checked_mul(u128::from(*weight))
                    .ok_or(Diagnostic::new(
                        "score_overflow",
                        "usage.resources",
                        "weighted fairness score overflowed",
                    ))?;
                total = total.checked_add(term).ok_or(Diagnostic::new(
                    "score_overflow",
                    "usage.resources",
                    "weighted fairness score overflowed",
                ))?;
            }
            Ok(Score {
                numerator: total,
                denominator: u128::from(usage.share),
            })
        }
        FairnessStrategy::DominantShare => {
            let mut dominant = Score {
                numerator: 0,
                denominator: 1,
            };
            for value in usage.resources.values() {
                let capacity = value.capacity.ok_or(Diagnostic::new(
                    "missing_capacity",
                    "usage.resources.capacity",
                    "dominant-share strategy requires a capacity basis",
                ))?;
                if capacity == 0 {
                    return Err(Diagnostic::new(
                        "invalid_capacity",
                        "usage.resources.capacity",
                        "resource capacity must be positive",
                    ));
                }
                let candidate = Score {
                    numerator: u128::from(value.amount),
                    denominator: u128::from(capacity),
                };
                if compare_fraction(candidate, dominant) == Ordering::Greater {
                    dominant = candidate;
                }
            }
            let denominator = dominant
                .denominator
                .checked_mul(u128::from(usage.share))
                .ok_or(Diagnostic::new(
                    "score_overflow",
                    "usage.share",
                    "dominant fairness score overflowed",
                ))?;
            Ok(Score {
                numerator: dominant.numerator,
                denominator,
            })
        }
    }
}

fn compare_score(left: &(Alias, Score), right: &(Alias, Score)) -> Ordering {
    let score_order = compare_fraction(left.1, right.1);
    if score_order == Ordering::Equal {
        return left.0.as_str().cmp(right.0.as_str());
    }
    score_order
}

fn compare_fraction(left: Score, right: Score) -> Ordering {
    let left_product = left.numerator.saturating_mul(right.denominator);
    let right_product = right.numerator.saturating_mul(left.denominator);
    left_product.cmp(&right_product)
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    const SHARE_ONE: u64 = 1;
    const SHARE_TWO: u64 = 2;
    const USAGE: u64 = 10;
    const CAPACITY: u64 = 100;
    const NOW: u64 = 20;
    const START: u64 = 1;
    const END: u64 = 10;
    const MAX_AGE: u64 = 20;

    fn tenant(name: &str, share: u64, amount: u64, with_capacity: bool) -> TenantUsage {
        let mut resources = BTreeMap::new();
        resources.insert(
            Alias::new("cpu").unwrap_or_else(|_| unreachable!()),
            UsageValue {
                amount,
                capacity: with_capacity.then_some(CAPACITY),
            },
        );
        TenantUsage {
            tenant: Alias::new(name).unwrap_or_else(|_| unreachable!()),
            share,
            provider: ProviderProfileId::new("provider").unwrap_or_else(|_| unreachable!()),
            interval_start: START,
            interval_end: END,
            observed_at: END,
            resources,
        }
    }

    #[test]
    fn larger_share_ranks_first_for_equal_usage() {
        let mut weights = BTreeMap::new();
        weights.insert(
            Alias::new("cpu").unwrap_or_else(|_| unreachable!()),
            SHARE_ONE,
        );
        let ranked = rank_tenants(
            &[
                tenant("small", SHARE_ONE, USAGE, false),
                tenant("large", SHARE_TWO, USAGE, false),
            ],
            &FairnessStrategy::WeightedScalar { weights },
            NOW,
            MAX_AGE,
        );
        assert_eq!(
            ranked.map(|items| items.first().cloned()),
            Ok(Some(Alias::new("large").unwrap_or_else(|_| unreachable!())))
        );
    }

    #[test]
    fn dominant_share_rejects_missing_capacity() {
        let result = rank_tenants(
            &[tenant("tenant", SHARE_ONE, USAGE, false)],
            &FairnessStrategy::DominantShare,
            NOW,
            MAX_AGE,
        );
        assert!(result.is_err());
    }
}
