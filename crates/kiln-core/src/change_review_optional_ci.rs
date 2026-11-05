//! Explicit review policy for required checks that a trigger `paths` filter
//! may legitimately skip. A filtered pipeline never satisfies required CI by
//! itself: only a declared rule whose paths cover every exactly known changed
//! file lets that project's check be absent.

use super::{Change, Facts, Policy, ReviewResult, Revision, fail, ownership_files};
use crate::paths;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Rules per policy.
pub const MAX_OPTIONAL_CI_RULES: usize = 32;
/// Path globs per rule.
pub const MAX_OPTIONAL_CI_PATTERNS: usize = 32;

/// `project`'s required check may be absent for changes whose every changed
/// path (and rename old path) matches one of `paths`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionalCi {
    pub project: String,
    /// Root-relative globs: component `*`, `?`, standalone `**`.
    pub paths: Vec<String>,
}

// r[impl kiln.triggers.required]
pub(super) fn validate(policy: &Policy) -> ReviewResult<()> {
    if policy.optional_ci.len() > MAX_OPTIONAL_CI_RULES
        || policy.optional_ci.iter().enumerate().any(|(index, rule)| {
            rule.project.trim().is_empty()
                || rule.project.trim() != rule.project
                || rule.paths.is_empty()
                || rule.paths.len() > MAX_OPTIONAL_CI_PATTERNS
                || !rule.paths.iter().all(|path| paths::valid_pattern(path))
                || policy
                    .optional_ci
                    .iter()
                    .take(index)
                    .any(|previous| previous.project == rule.project)
        })
    {
        return fail(
            "invalid_policy",
            "optional CI rules need one unique project each and bounded valid path globs",
        );
    }
    Ok(())
}

// r[impl kiln.triggers.required]
/// Whether `project`'s missing check is waived for this exact revision: the
/// changed files are complete, non-empty, match the revision exactly, and every
/// path is covered by the project's rule. Unknown facts never waive.
pub(super) fn waived(
    policy: Option<&Policy>,
    project: &str,
    change: &Change,
    revision: &Revision,
    facts: &Facts,
) -> bool {
    let Some(rule) = policy.and_then(|policy| {
        policy
            .optional_ci
            .iter()
            .find(|rule| rule.project == project)
    }) else {
        return false;
    };
    let covered = |path: &String| {
        rule.paths
            .iter()
            .any(|pattern| paths::matches(pattern, path))
    };
    ownership_files(change, revision, facts).is_ok_and(|files| {
        !files.is_empty()
            && files
                .iter()
                .all(|file| covered(&file.path) && file.old_path.as_ref().is_none_or(covered))
    })
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use alloc::vec;

    const OWNER: &str = "alice";

    fn policy(optional_ci: Vec<OptionalCi>) -> Policy {
        Policy {
            id: "policy-1".into(),
            eligible_reviewers: ["bob".into()].into_iter().collect(),
            submitters: [OWNER.into()].into_iter().collect(),
            minimum_approvals: 1,
            require_ci: true,
            ownership: OwnershipPolicy::default(),
            optional_ci,
            agent_votes: AgentVotes::Advisory,

            required_checks: Vec::new(),
        }
    }

    fn docs_rule() -> OptionalCi {
        OptionalCi {
            project: "project".into(),
            paths: vec!["docs/**".into(), "*.md".into()],
        }
    }

    fn facts(files: Option<(&[&str], bool)>) -> Facts {
        Facts {
            policy: Some(policy(vec![docs_rule()])),
            source: Some(SourceFact {
                project: "project".into(),
                repository: "repository".into(),
                target_ref: "refs/heads/main".into(),
                commit: "commit-1".into(),
                base: "base".into(),
                current_target: "base".into(),
                immutable: true,
                target_contains_commit: false,
                target_is_ancestor: true,
                target_ancestors: BTreeSet::new(),
                changed_files: files.map(|(paths, complete)| ChangedFiles {
                    files: paths
                        .iter()
                        .map(|path| ChangedFile {
                            path: (*path).into(),
                            old_path: None,
                        })
                        .collect(),
                    complete,
                }),
            }),
            // The path-filtered pipeline produced no run for this commit.
            ci: Vec::new(),
            ..Facts::default()
        }
    }

    fn state(facts: &mut Facts, files: &[&str]) -> ReviewState {
        let mut state = ReviewState::default();
        facts.now_ms += 1;
        facts.event_id = "event-1".into();
        let created = apply(
            &mut state,
            OWNER,
            Command::Create {
                id: "c".into(),
                project: "project".into(),
                repository: "repository".into(),
                target_ref: "refs/heads/main".into(),
                title: "Review".into(),
                description: "Docs".into(),
                wip: false,
                revision: Revision {
                    id: "r1".into(),
                    number: 1,
                    commit: "commit-1".into(),
                    base: "base".into(),
                    files: files.iter().map(|path| (*path).into()).collect(),
                    author: OWNER.into(),
                    created_ms: facts.now_ms,
                    authorship: Authorship::Undeclared,
                    submitted_by: None,
                },
            },
            facts,
        );
        if let Err(error) = created {
            unreachable!("fixture change rejected: {error}");
        }
        state
    }

    fn ci_satisfied(state: &ReviewState, facts: &Facts) -> bool {
        checklist(state, "c", facts)
            .unwrap_or_else(|error| unreachable!("checklist: {error}"))
            .items
            .iter()
            .any(|item| item.key == "ci" && item.satisfied)
    }

    // r[verify kiln.triggers.required]
    // r[verify kiln.triggers.required.scenario.unmet]
    // r[verify kiln.triggers.required.scenario.optional]
    #[test]
    fn filtered_required_check_is_unmet_unless_policy_covers_every_changed_path() {
        let docs: &[&str] = &["docs/guide.md", "README.md"];
        // Every exact changed file is covered: the missing check is waived.
        let mut covered = facts(Some((docs, true)));
        let reviewed = state(&mut covered, docs);
        assert!(ci_satisfied(&reviewed, &covered));
        // Without the policy rule a filtered required check stays unmet.
        let mut no_rule = covered.clone();
        no_rule.policy = Some(policy(Vec::new()));
        assert!(!ci_satisfied(&reviewed, &no_rule));
        // The rule names one project; monorepo requirements of others stay.
        let mut other_project = covered.clone();
        other_project.ci_required = Some(vec![
            CiRequirement {
                project: "project".into(),
                scope_identity: None,
            },
            CiRequirement {
                project: "api".into(),
                scope_identity: None,
            },
        ]);
        assert!(!ci_satisfied(&reviewed, &other_project));
        // One file outside the declared paths keeps CI unmet.
        let mixed: &[&str] = &["docs/guide.md", "src/lib.rs"];
        let mut outside = facts(Some((mixed, true)));
        let mixed_state = state(&mut outside, mixed);
        assert!(!ci_satisfied(&mixed_state, &outside));
        // Unknown or incomplete changed files never waive.
        let mut unknown = facts(None);
        let unknown_state = state(&mut unknown, docs);
        assert!(!ci_satisfied(&unknown_state, &unknown));
        let mut incomplete = facts(Some((docs, false)));
        let incomplete_state = state(&mut incomplete, docs);
        assert!(!ci_satisfied(&incomplete_state, &incomplete));
        // A run that exists and failed still blocks.
        let mut failed = covered.clone();
        failed.ci.push(CiFact {
            project: "project".into(),
            repository: "repository".into(),
            revision: "commit-1".into(),
            run_id: "run-1".into(),
            ordinal: 1,
            success: false,
            full_run: true,
            clean: true,
            scope_identity: None,
            terminal: true,
            untrusted: None,

            checks: Vec::new(),
        });
        assert!(!ci_satisfied(&reviewed, &failed));
    }

    #[test]
    fn optional_ci_rules_are_bounded_unique_and_omitted_when_empty() {
        assert!(validate_policy(&policy(vec![docs_rule()])).is_ok());
        for invalid in [
            vec![docs_rule(), docs_rule()],
            vec![OptionalCi {
                project: "project".into(),
                paths: Vec::new(),
            }],
            vec![OptionalCi {
                project: "project".into(),
                paths: vec!["../escape".into()],
            }],
            vec![OptionalCi {
                project: " project".into(),
                paths: vec!["docs/**".into()],
            }],
        ] {
            assert!(validate_policy(&policy(invalid)).is_err());
        }
        let encoded = serde_json::to_string(&policy(Vec::new())).unwrap_or_default();
        assert!(!encoded.contains("optional_ci"));
    }
}
