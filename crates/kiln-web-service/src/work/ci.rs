//! Bounded monotone facts joining signed work records to admitted CI projects.
//!
//! A record reaches a CI project only through an explicit `run` link whose
//! retained run belongs to that project. These facts are derived per request
//! and are never written into signed work records.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::model::Record;

/// Schema of the derived facts exposed in the Work listing.
pub const CI_FACTS_SCHEMA: &str = "kiln.work.ci.v1";
/// Hard ceiling on project facts derived from one admitted service profile.
pub const MAX_CI_PROJECTS: usize = 512;
/// Hard ceiling on retained run-to-project facts.
pub const MAX_CI_RUN_FACTS: usize = 131_072;

/// Hard ceiling on per-record project-id references in one listing response.
pub const MAX_CI_RECORD_FACTS: usize = 8_192;

/// One admitted CI project, as a display and filter fact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CiProjectFact {
    pub id: String,
    pub label: String,
    /// Canonical repository path shared by the projects of one monorepo.
    pub repository: String,
    /// BLAKE3 over the canonical repository path.
    pub repository_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_root: Option<String>,
}

/// Bounded derived facts for one Work request. Empty facts are the default and
/// make every CI filter match nothing while ordinary views keep working.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CiFacts {
    projects: Vec<CiProjectFact>,
    runs: BTreeMap<String, String>,
    projects_truncated: usize,
    runs_truncated: usize,
}

impl CiFacts {
    /// Admit facts in deterministic order: repository key, then project id.
    /// Runs that do not name an admitted project are dropped rather than
    /// inventing a project fact.
    #[must_use]
    pub fn new(
        projects: impl IntoIterator<Item = CiProjectFact>,
        runs: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        let mut projects: Vec<CiProjectFact> = projects
            .into_iter()
            .filter(|project| !project.id.is_empty() && !project.repository_key.is_empty())
            .collect();
        projects.sort_by(|a, b| {
            a.repository_key
                .cmp(&b.repository_key)
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut seen = BTreeSet::new();
        projects.retain(|project| seen.insert(project.id.clone()));
        let projects_truncated = projects.len().saturating_sub(MAX_CI_PROJECTS);
        projects.truncate(MAX_CI_PROJECTS);
        let known: BTreeSet<&str> = projects.iter().map(|project| project.id.as_str()).collect();
        let mut runs: BTreeMap<String, String> = runs
            .into_iter()
            .filter(|(run, project)| !run.is_empty() && known.contains(project.as_str()))
            .collect();
        let runs_truncated = runs.len().saturating_sub(MAX_CI_RUN_FACTS);
        if runs_truncated > 0 {
            let keep: BTreeSet<String> = runs.keys().take(MAX_CI_RUN_FACTS).cloned().collect();
            runs.retain(|run, _| keep.contains(run));
        }
        Self {
            projects,
            runs,
            projects_truncated,
            runs_truncated,
        }
    }

    #[must_use]
    pub fn projects(&self) -> &[CiProjectFact] {
        &self.projects
    }

    #[must_use]
    pub fn project(&self, id: &str) -> Option<&CiProjectFact> {
        self.projects.iter().find(|project| project.id == id)
    }

    #[must_use]
    pub const fn projects_truncated(&self) -> usize {
        self.projects_truncated
    }

    #[must_use]
    pub const fn runs_truncated(&self) -> usize {
        self.runs_truncated
    }

    /// Exact project facts reachable from a record's explicit `run` links, in
    /// deterministic repository-then-id order. Unknown links contribute none.
    #[must_use]
    pub fn projects_for(&self, record: &Record) -> Vec<&CiProjectFact> {
        let linked: BTreeSet<&str> = record
            .fields
            .links
            .iter()
            .filter(|link| link.kind == "run")
            .filter_map(|link| self.runs.get(&link.id).map(String::as_str))
            .collect();
        self.projects
            .iter()
            .filter(|project| linked.contains(project.id.as_str()))
            .collect()
    }

    /// Exact linked project ids in the same deterministic order.
    #[must_use]
    pub fn project_ids(&self, record: &Record) -> Vec<&str> {
        self.projects_for(record)
            .into_iter()
            .map(|project| project.id.as_str())
            .collect()
    }

    /// The first exact linked project, used by grouping and lane assignment.
    #[must_use]
    pub fn first_project(&self, record: &Record) -> Option<&CiProjectFact> {
        self.projects_for(record).into_iter().next()
    }

    /// Bounded per-record project ids for the listing response. At most
    /// [`MAX_CI_RECORD_FACTS`] id references are returned; the omitted record
    /// count makes the truncation visible instead of silently dropping facts.
    #[must_use]
    pub fn record_map<'a>(
        &self,
        records: impl IntoIterator<Item = &'a Record>,
    ) -> (BTreeMap<String, Vec<String>>, usize) {
        let mut map = BTreeMap::new();
        let mut total = 0_usize;
        let mut omitted = 0_usize;
        for record in records {
            let ids = self.project_ids(record);
            if ids.is_empty() {
                continue;
            }
            if total.saturating_add(ids.len()) > MAX_CI_RECORD_FACTS {
                omitted += 1;
                continue;
            }
            total += ids.len();
            map.insert(
                record.id.clone(),
                ids.into_iter().map(str::to_owned).collect(),
            );
        }
        (map, omitted)
    }

    /// Match only the CI filter keys; every other key is left to the view
    /// matcher. An absent or unknown fact never satisfies a positive filter.
    #[must_use]
    pub fn matches(&self, record: &Record, filters: &BTreeMap<String, String>) -> bool {
        filters.iter().all(|(key, value)| {
            if value.is_empty() {
                return true;
            }
            match key.as_str() {
                "ci_project" => {
                    let selected = exact_set(value);
                    self.project_ids(record)
                        .iter()
                        .any(|id| selected.contains(*id))
                }
                "ci_repository" => {
                    let selected = exact_set(value);
                    self.projects_for(record)
                        .iter()
                        .any(|project| selected.contains(project.repository_key.as_str()))
                }
                _ => true,
            }
        })
    }
}

/// Parse one bounded comma-separated exact set. Empty members are ignored by
/// matching; the write path rejects them so a saved view cannot encode a
/// meaningless empty member.
#[must_use]
pub fn exact_set(value: &str) -> BTreeSet<&str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::model::{Fields, Kind};

    fn record(id: &str, links: &[(&str, &str)]) -> Record {
        let mut record = Record {
            id: id.into(),
            head: format!("head-{id}"),
            kind: Kind::Issue,
            fields: Fields {
                title: id.into(),
                ..Fields::default()
            },
            comments: Vec::new(),
            updates: Vec::new(),
            created_by: "local:alice".into(),
            created_at_ms: 1,
            updated_at_ms: 1,
        };
        record.fields.links = links
            .iter()
            .map(|(kind, id)| crate::work::model::Link {
                kind: (*kind).into(),
                id: (*id).into(),
            })
            .collect();
        record
    }

    fn project(id: &str, key: &str) -> CiProjectFact {
        CiProjectFact {
            id: id.into(),
            label: id.into(),
            repository: "/repo".into(),
            repository_key: key.into(),
            scope_root: None,
        }
    }

    // r[verify kiln.work.monorepo.facts] r[verify kiln.work.monorepo.facts.scenario.resolved] r[verify kiln.work.monorepo.facts.scenario.unknown]
    #[test]
    fn facts_are_deterministic_and_only_resolve_exact_links() {
        let facts = CiFacts::new(
            [
                project("b", &"b".repeat(64)),
                project("a", &"a".repeat(64)),
                project("a", &"c".repeat(64)),
            ],
            [
                ("run-a".to_owned(), "a".to_owned()),
                ("run-b".to_owned(), "b".to_owned()),
                ("run-lost".to_owned(), "gone".to_owned()),
            ],
        );
        assert_eq!(
            facts
                .projects()
                .iter()
                .map(|project| project.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        let linked = record(
            "issue",
            &[("run", "run-a"), ("run", "run-b"), ("run", "run-lost")],
        );
        assert_eq!(facts.project_ids(&linked), ["a", "b"]);
        assert_eq!(
            facts.first_project(&linked).map(|p| p.id.as_str()),
            Some("a")
        );
        let source_only = record("source", &[("source", "repo@rev")]);
        assert!(facts.project_ids(&source_only).is_empty());
        let filters = BTreeMap::from([("ci_project".to_owned(), "a".to_owned())]);
        assert!(facts.matches(&linked, &filters));
        let unknown = BTreeMap::from([("ci_project".to_owned(), "gone".to_owned())]);
        assert!(!facts.matches(&linked, &unknown));
        let repository = BTreeMap::from([("ci_repository".to_owned(), "b".repeat(64))]);
        assert!(facts.matches(&linked, &repository));
        // A run of an admitted project is only a fact when a record links it.
        assert_eq!(
            facts.record_map([&linked, &source_only]),
            (
                BTreeMap::from([("issue".to_owned(), vec!["a".to_owned(), "b".to_owned()])]),
                0
            )
        );
    }

    // r[verify kiln.work.monorepo.facts]
    #[test]
    fn record_fact_map_and_hard_project_bound_report_truncation() {
        let projects: Vec<_> = (0..MAX_CI_PROJECTS + 1)
            .map(|index| project(&format!("p{index:04}"), &"a".repeat(64)))
            .collect();
        let facts = CiFacts::new(projects, std::iter::empty());
        assert_eq!(facts.projects().len(), MAX_CI_PROJECTS);
        assert_eq!(facts.projects_truncated(), 1);
        assert_eq!(
            facts.projects().first().map(|project| project.id.as_str()),
            Some("p0000")
        );
        // One more reference than the response bound: the later record is
        // reported omitted instead of silently disappearing.
        let many: Vec<_> = (0..MAX_CI_RECORD_FACTS / 2 + 1)
            .map(|index| {
                let first = format!("p{:04}", index % MAX_CI_PROJECTS);
                let second = format!("p{:04}", (index + 1) % MAX_CI_PROJECTS);
                record(
                    &format!("issue-{index}"),
                    &[
                        ("run", &format!("run-{first}")),
                        ("run", &format!("run-{second}")),
                    ],
                )
            })
            .collect();
        let runs = (0..MAX_CI_PROJECTS)
            .flat_map(|index| {
                let id = format!("p{index:04}");
                [
                    (format!("run-{id}"), id.clone()),
                    (format!("run-{id}x"), id),
                ]
            })
            .collect::<Vec<_>>();
        let facts = CiFacts::new(
            (0..MAX_CI_PROJECTS).map(|index| project(&format!("p{index:04}"), &"a".repeat(64))),
            runs,
        );
        let (map, omitted) = facts.record_map(&many);
        assert_eq!(omitted, 1);
        assert!(map.values().map(Vec::len).sum::<usize>() <= MAX_CI_RECORD_FACTS);
    }
}
