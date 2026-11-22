//! Attempt grant issuance intents and their pure reconciliation fold (#70).
//!
//! Not wired yet. The grant-lifecycle shell appends [`GrantIssuanceRecord`]s to
//! its journal: `Intended` in the same transaction as the #61 `Transfer`
//! reservation and before any token exists, `Issued` once the minted token's
//! proof reference is known, and `Revoked` with the revocation generation.
//!
//! Folding records performs nothing, so replaying saved state issues no grant.
//! [`GrantIssuanceState::reconcile`] lists what a live shell still owes, each
//! under the original intent identity: a crash between intent and `Issued`
//! resumes that intent rather than deriving a second one, and a revoked grant
//! propagates to its issued descendants.

use std::collections::BTreeMap;

use kiln_core::implementation::authority::AUTHORITY_DIGEST_BYTES;
use kiln_core::implementation::grant::{AdmittedGrant, AttemptLineage, MAX_GRANT_LINEAGE};
use kiln_core::types::{ContentId, CoreResult, Diagnostic};
use serde::{Deserialize, Serialize};

/// Journal kind schema of grant issuance records.
pub const GRANT_ISSUANCE_SCHEMA: &str = "kiln.attempt-grant-issuance.v1";
/// Maximum issuance intents one fold retains.
pub const MAX_GRANT_ISSUANCE_INTENTS: usize = 4_096;

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
}

/// What a live shell still owes for one intent.
#[derive(Clone, Debug, Eq, PartialEq)]
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
struct IntentState {
    grant: AdmittedGrant,
    issued: Option<GrantRef>,
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
        }
    }

    fn intend(&mut self, grant: &AdmittedGrant) -> CoreResult<()> {
        grant.validate()?;
        if self.intents.contains_key(&grant.intent) {
            return Ok(());
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
        let binding = state.grant.binding_identity(grant_ref)?;
        state.issued = Some(*grant_ref);
        self.by_grant_ref.insert(*grant_ref, intent.clone());
        self.by_binding.insert(binding, intent.clone());
        Ok(())
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

    /// Effects a live shell still owes, in intent order.
    ///
    /// The fold spans attempt lineages, so `current` resolves each intent to
    /// the current lineage of its work unit: the shell maps the intent's work
    /// unit to its current, active attempt, and returns `None` when the work
    /// unit has none (for example, cancelled). An unissued intent is issued
    /// only while it is current: it is abandoned when `current` gives `None`,
    /// when it does not bind exactly the resolved lineage (including a
    /// replaced fence), when it is from another authority generation, under a
    /// revoked ancestor, or past its expiry.
    #[must_use]
    pub fn reconcile<'a>(
        &self,
        now_s: u64,
        authority_generation: u64,
        current: impl Fn(&AdmittedGrant) -> Option<&'a AttemptLineage>,
    ) -> Vec<GrantIssuanceAction> {
        let mut actions = Vec::new();
        for (intent, state) in &self.intents {
            let ancestor_revoked = self.ancestor_revoked(&state.grant);
            match state.issued {
                Some(grant_ref) if ancestor_revoked && !self.is_revoked(&grant_ref) => {
                    actions.push(GrantIssuanceAction::Revoke {
                        intent: intent.clone(),
                        grant_ref,
                    });
                }
                Some(_) => {}
                None if ancestor_revoked
                    || now_s >= state.grant.ceiling.not_after_s
                    || state.grant.ceiling.generation != authority_generation
                    || !current(&state.grant).is_some_and(|lineage| state.grant.binds(lineage)) =>
                {
                    actions.push(GrantIssuanceAction::Abandon {
                        intent: intent.clone(),
                    });
                }
                None => actions.push(GrantIssuanceAction::Issue {
                    intent: intent.clone(),
                }),
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
            {
                return true;
            }
            parent = state.grant.parent.as_ref();
        }
        false
    }
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::authority::ProtectedOperation;
    use kiln_core::implementation::grant::{
        AttemptAuthorityBinding, AttemptLineage, AuthorityCeiling, CeilingTerms, ChildGrantRequest,
        CurrentFacts, DelegationFacts, GrantBudget, RootFacts, RootGrantRequest, admit_child,
        admit_root,
    };
    use std::collections::BTreeSet;

    const NOW: u64 = 100;
    const NOT_AFTER: u64 = 1_000;
    const GENERATION: u64 = 1;
    const COORDINATOR: &str = "did:key:coordinator";

    fn id(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn lineage() -> AttemptLineage {
        AttemptLineage {
            campaign_revision: None,
            project: "kiln".to_string(),
            work: id('1'),
            work_version: 1,
            attempt: id('a'),
            fence: 1,
        }
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
            work: Some(id('1')),
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
        let first_lineage = lineage();
        let second_lineage = AttemptLineage {
            work: id('2'),
            attempt: id('b'),
            fence: 2,
            ..lineage()
        };
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
            sorted(state.reconcile(NOW, GENERATION, |grant| units.get(&grant.work)))
        };

        let both = BTreeMap::from([
            (id('1'), first_lineage.clone()),
            (id('2'), second_lineage.clone()),
        ]);
        assert_eq!(
            reconcile(&both),
            sorted(vec![issue(&first), issue(&second)]),
            "two current attempts are both issued in one call"
        );

        let cancelled = BTreeMap::from([(id('1'), first_lineage.clone())]);
        assert_eq!(
            reconcile(&cancelled),
            sorted(vec![issue(&first), abandon(&second)]),
            "a work unit with no current attempt abandons its intent"
        );

        let replaced = BTreeMap::from([
            (id('1'), first_lineage),
            (
                id('2'),
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
    fn a_crash_after_the_intent_resumes_that_intent_and_never_a_second() {
        let root = root_grant();
        let current = lineage();
        let crashed = GrantIssuanceState::replay(&[intended(&root), intended(&root)])
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            crashed.reconcile(NOW, GENERATION, |_| Some(&current)),
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
            crashed.reconcile(NOW, GENERATION + 1, |_| Some(&current)),
            abandoned,
            "an intent from a moved authority generation is never minted"
        );
        let mut replaced = lineage();
        replaced.attempt = id('f');
        replaced.fence = 2;
        assert_eq!(
            crashed.reconcile(NOW, GENERATION, |_| Some(&replaced)),
            abandoned,
            "an intent for a replaced attempt is never minted"
        );
        let mut other_project = lineage();
        other_project.project = "other".to_string();
        assert_eq!(
            crashed.reconcile(NOW, GENERATION, |_| Some(&other_project)),
            abandoned,
            "an intent for another lineage is never minted"
        );

        let records = [intended(&root), issued(&root, 1), issued(&root, 1)];
        let done = GrantIssuanceState::replay(&records).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            done.reconcile(NOW, GENERATION, |_| Some(&current)),
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
        let current = lineage();
        let mut actions = state.reconcile(NOW, GENERATION, |_| Some(&current));
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
            acknowledged.reconcile(NOW, GENERATION, |_| Some(&current)),
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
            expiring.reconcile(NOW + 1, GENERATION, |_| Some(&current)),
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
