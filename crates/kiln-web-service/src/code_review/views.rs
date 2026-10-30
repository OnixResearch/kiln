//! Comparison selection for the review diff endpoint: exact base or earlier
//! revision, rebase-aware interdiff (optionally narrowed to one change ID) with
//! explicit fallback, and commit ranges inside one revision.
use super::*;

const MAX_COMMIT_HEX: usize = 40;

/// The comparison to display and its annotations for the response.
pub(super) struct View {
    pub diff: CommitComparison,
    pub annotations: serde_json::Map<String, Value>,
    /// The displayed comparison is the revision's exact base comparison.
    pub exact_base: bool,
    /// Old-side lines are exact commit coordinates (not an in-memory rebase).
    pub exact_old_side: bool,
}

fn whitespace(query: &BTreeMap<String, String>) -> Result<WhitespaceMode, Failure> {
    Ok(match query.get("whitespace").map(String::as_str) {
        None | Some("exact") => WhitespaceMode::Exact,
        Some("ignore_all") => WhitespaceMode::IgnoreAll,
        Some("ignore_change") => WhitespaceMode::IgnoreChange,
        Some("ignore_eol") => WhitespaceMode::IgnoreEol,
        _ => return reject(400, "review_whitespace_invalid"),
    })
}

fn full_commit(value: &str) -> bool {
    value.len() == MAX_COMMIT_HEX
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Pure stack narrowing: the single commit carrying `change_id` on each side,
/// with its first parent. Every commit of both ranges must carry a change ID.
// r[impl kiln.review.interdiff.scenario.narrowed]
pub(super) fn narrow(
    old: &RevisionCommits,
    new: &RevisionCommits,
    change_id: &str,
) -> Result<(String, String, String, String), InterdiffUnavailable> {
    if old.incomplete
        || new.incomplete
        || old.commits.is_empty()
        || new.commits.is_empty()
        || old
            .commits
            .iter()
            .chain(&new.commits)
            .any(|commit| commit.change_id.is_none())
    {
        return Err(InterdiffUnavailable::NoChangeIds);
    }
    let pick = |range: &RevisionCommits| {
        let mut found = range
            .commits
            .iter()
            .filter(|commit| commit.change_id.as_deref() == Some(change_id));
        match (found.next(), found.next()) {
            (None, _) => Err(InterdiffUnavailable::ChangeMissing),
            (Some(_), Some(_)) => Err(InterdiffUnavailable::ChangeAmbiguous),
            (Some(commit), None) => commit
                .parents
                .first()
                .map(|parent| (parent.clone(), commit.commit.clone()))
                .ok_or(InterdiffUnavailable::ChangeMissing),
        }
    };
    let (old_parent, old_commit) = pick(old)?;
    let (new_parent, new_commit) = pick(new)?;
    Ok((old_parent, old_commit, new_parent, new_commit))
}

/// Change ID of a revision's exact head commit observed by jj intake, when
/// the provider listing has no `change-id` header for it.
pub(super) type HeadChangeId<'a> = &'a dyn Fn(&domain::Revision) -> Option<String>;

fn listed(
    provider: &impl ChangeReviewPort,
    revision: &domain::Revision,
    observed: HeadChangeId<'_>,
) -> Result<Option<RevisionCommits>, Failure> {
    match provider.revision_commits(&revision.base, &revision.commit, MAX_REVISION_COMMITS) {
        Ok(mut commits) => {
            if let Some(head) = commits
                .commits
                .last_mut()
                .filter(|head| head.commit == revision.commit && head.change_id.is_none())
            {
                head.change_id =
                    observed(revision).filter(|id| kiln_core::source::valid_jujutsu_change_id(id));
            }
            Ok(Some(commits))
        }
        Err(error) if error.kind == ReviewErrorKind::Unsupported => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn compare(
    provider: &impl ChangeReviewPort,
    old: &str,
    new: &str,
    whitespace: WhitespaceMode,
) -> Result<CommitComparison, Failure> {
    provider
        .compare(&CompareRequest {
            old_commit: old.into(),
            new_commit: new.into(),
            limits: DiffLimits::default(),
            whitespace,
        })
        .map_err(Failure::from)
}

fn revision_by_id<'a>(
    change: &'a domain::Change,
    id: &str,
    missing: &str,
) -> Result<&'a domain::Revision, Failure> {
    change
        .revisions
        .iter()
        .find(|r| r.id == id)
        .ok_or_else(|| Failure::Api(404, missing.into()))
}

// r[impl kiln.review.interdiff]
// r[impl kiln.review.commit_range]
pub(super) fn select(
    provider: &impl ChangeReviewPort,
    change: &domain::Change,
    revision: &domain::Revision,
    query: &BTreeMap<String, String>,
    observed: HeadChangeId<'_>,
) -> Result<View, Failure> {
    let whitespace = whitespace(query)?;
    let base = query.get("base").map(String::as_str);
    let interdiff = query.get("interdiff");
    let range = query.get("commits");
    let narrowed = query.get("change");
    let selectors = [
        base.is_some_and(|b| b != "base"),
        interdiff.is_some(),
        range.is_some(),
    ];
    if selectors.iter().filter(|s| **s).count() > 1 || (narrowed.is_some() && interdiff.is_none()) {
        return reject(400, "review_comparison_invalid");
    }
    let commits = listed(provider, revision, observed)?;
    let mut annotations = serde_json::Map::new();
    annotations.insert(
        "revision_commits".into(),
        serde_json::to_value(&commits).map_err(|_| Failure::Unavailable)?,
    );
    if let Some(from) = interdiff {
        let from = revision_by_id(change, from, "review_interdiff_missing")?;
        if from.number >= revision.number {
            return reject(400, "review_interdiff_order");
        }
        if narrowed.is_some_and(|id| !kiln_core::source::valid_jujutsu_change_id(id)) {
            return reject(400, "review_change_id_invalid");
        }
        let request = |(old_base, old_commit, new_base, new_commit)| InterdiffRequest {
            old_base,
            old_commit,
            new_base,
            new_commit,
            limits: DiffLimits::default(),
            whitespace,
        };
        let outcome = match narrowed {
            None => provider.interdiff(&request((
                from.base.clone(),
                from.commit.clone(),
                revision.base.clone(),
                revision.commit.clone(),
            )))?,
            Some(change_id) => {
                let old = listed(provider, from, observed)?;
                let narrowed = match (&old, &commits) {
                    (Some(old), Some(new)) => narrow(old, new, change_id),
                    _ => Err(InterdiffUnavailable::Unsupported),
                };
                match narrowed {
                    Ok(pair) => provider.interdiff(&request(pair))?,
                    Err(reason) => Interdiff::Unavailable {
                        reason,
                        message: "the change ID cannot be narrowed exactly in both revisions"
                            .into(),
                    },
                }
            }
        };
        let (diff, status, reason, message) = match outcome {
            Interdiff::Exact { comparison } => (comparison, "exact", None, None),
            Interdiff::Unavailable { reason, message } => (
                compare(provider, &from.commit, &revision.commit, whitespace)?,
                "unavailable",
                Some(reason),
                Some(message),
            ),
        };
        let exact = status == "exact";
        annotations.insert(
            "interdiff".into(),
            json!({"status":status,"reason":reason,"message":message,"from":from.id,"from_number":from.number,
                "to":revision.id,"to_number":revision.number,"change":narrowed,
                "fallback":(!exact).then_some("exact_commit_comparison")}),
        );
        return Ok(View {
            diff,
            annotations,
            exact_base: false,
            exact_old_side: !exact,
        });
    }
    if let Some(range) = range {
        let (from, to) = range
            .split_once("..")
            .filter(|(from, to)| full_commit(from) && full_commit(to))
            .ok_or_else(|| Failure::Api(400, "review_commits_invalid".into()))?;
        let listed = commits
            .as_ref()
            .map(|c| c.commits.as_slice())
            .unwrap_or_default();
        let position = |id: &str| listed.iter().position(|c| c.commit == id);
        let start = if from == revision.base {
            Some(None)
        } else {
            position(from).map(Some)
        };
        match (start, position(to)) {
            (Some(start), Some(end)) if start.is_none_or(|start| start < end) => {}
            _ => return reject(400, "review_commits_invalid"),
        }
        annotations.insert(
            "commit_range".into(),
            json!({"from":from,"to":to,"anchorable":to == revision.commit}),
        );
        return Ok(View {
            diff: compare(provider, from, to, whitespace)?,
            annotations,
            exact_base: false,
            exact_old_side: true,
        });
    }
    let old = match base {
        None | Some("base") => &revision.base,
        Some(id) => &revision_by_id(change, id, "review_base_missing")?.commit,
    };
    Ok(View {
        diff: compare(provider, old, &revision.commit, whitespace)?,
        annotations,
        exact_base: old == &revision.base && whitespace == WhitespaceMode::Exact,
        exact_old_side: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "wnloytsvxwxwxtpqyuwxutmotlmslszv";
    const B: &str = "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk";

    fn range(base: &str, commits: &[(&str, Option<&str>)]) -> RevisionCommits {
        let mut parent = base.to_owned();
        let commits = commits
            .iter()
            .map(|(commit, change)| {
                let item = RevisionCommit {
                    commit: (*commit).into(),
                    parents: vec![parent.clone()],
                    summary: String::new(),
                    change_id: change.map(str::to_owned),
                };
                parent = (*commit).into();
                item
            })
            .collect();
        RevisionCommits {
            base: base.into(),
            head: parent,
            commits,
            incomplete: false,
        }
    }

    // r[verify kiln.review.interdiff.scenario.narrowed]
    #[test]
    fn narrowing_picks_the_one_commit_of_the_change_on_each_side() {
        let old = range("base1", &[("o1", Some(A)), ("o2", Some(B))]);
        let new = range("base2", &[("n1", Some(A)), ("n2", Some(B))]);
        assert_eq!(
            narrow(&old, &new, B),
            Ok(("o1".into(), "o2".into(), "n1".into(), "n2".into()))
        );
        assert_eq!(
            narrow(&old, &new, A),
            Ok(("base1".into(), "o1".into(), "base2".into(), "n1".into()))
        );
    }

    // r[verify kiln.review.interdiff.scenario.narrowed]
    #[test]
    fn narrowing_is_unavailable_without_exact_change_ids() {
        let old = range("base1", &[("o1", Some(A)), ("o2", None)]);
        let new = range("base2", &[("n1", Some(A))]);
        assert_eq!(
            narrow(&old, &new, A),
            Err(InterdiffUnavailable::NoChangeIds)
        );
        let old = range("base1", &[("o1", Some(A))]);
        assert_eq!(
            narrow(&old, &new, B),
            Err(InterdiffUnavailable::ChangeMissing)
        );
        let doubled = range("base2", &[("n1", Some(A)), ("n2", Some(A))]);
        assert_eq!(
            narrow(&old, &doubled, A),
            Err(InterdiffUnavailable::ChangeAmbiguous)
        );
        let mut incomplete = new.clone();
        incomplete.incomplete = true;
        assert_eq!(
            narrow(&old, &incomplete, A),
            Err(InterdiffUnavailable::NoChangeIds)
        );
        assert_eq!(
            narrow(&old, &range("base2", &[]), A),
            Err(InterdiffUnavailable::NoChangeIds)
        );
    }
}
