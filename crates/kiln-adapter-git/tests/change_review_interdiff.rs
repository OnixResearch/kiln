// r[verify kiln.review.interdiff]
//! Rebase-aware interdiff and revision commit listing against real Git fixtures.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use git2::{ObjectType, Oid, Repository};
use kiln_adapter_git::change_review::{LocalGitReviewProfile, LocalGitReviewProvider};
use kiln_provider_api::change_review::*;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const CHANGE_A: &str = "wnloytsvxwxwxtpqyuwxutmotlmslszv";
const CHANGE_B: &str = "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk";

struct Fixture {
    path: PathBuf,
    repo: Repository,
}
impl Fixture {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "kiln-review-interdiff-{}-{label}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let repo = Repository::init(&path).unwrap_or_else(|e| unreachable!("fixture: {e:?}"));
        Self { path, repo }
    }
    fn provider(&self, capabilities: Vec<ReviewCapability>) -> LocalGitReviewProvider {
        LocalGitReviewProvider::open(LocalGitReviewProfile {
            repository_id: "test-repository".into(),
            repository_path: self.path.clone(),
            target_refs: vec!["refs/heads/main".into()],
            capabilities,
        })
        .unwrap_or_else(|e| unreachable!("fixture: {e:?}"))
    }
    /// Commit `files` on `parents`, optionally with a `change-id` header.
    fn commit(&self, parents: &[Oid], files: &[(&str, &str)], change_id: Option<&str>) -> Oid {
        let mut builder = self
            .repo
            .treebuilder(None)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        for (path, content) in files {
            let blob = self
                .repo
                .blob(content.as_bytes())
                .unwrap_or_else(|e| unreachable!("{e:?}"));
            builder
                .insert(*path, blob, 0o100644)
                .unwrap_or_else(|e| unreachable!("{e:?}"));
        }
        let tree = builder.write().unwrap_or_else(|e| unreachable!("{e:?}"));
        let mut text = format!("tree {tree}\n");
        for parent in parents {
            text.push_str(&format!("parent {parent}\n"));
        }
        text.push_str("author Fixture <fixture@example.invalid> 1600000000 +0000\n");
        text.push_str("committer Fixture <fixture@example.invalid> 1600000000 +0000\n");
        if let Some(id) = change_id {
            text.push_str(&format!("change-id {id}\n"));
        }
        text.push_str(&format!(
            "\nfixture {}\n",
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let id = self
            .repo
            .odb()
            .and_then(|odb| odb.write(ObjectType::Commit, text.as_bytes()))
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        if self.repo.find_reference("refs/heads/main").is_err() {
            self.repo
                .reference("refs/heads/main", id, false, "fixture target")
                .unwrap_or_else(|e| unreachable!("{e:?}"));
        }
        id
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn all() -> Vec<ReviewCapability> {
    vec![ReviewCapability::Inspect, ReviewCapability::Compare]
}
fn request(old_base: Oid, old: Oid, new_base: Oid, new: Oid) -> InterdiffRequest {
    InterdiffRequest {
        old_base: old_base.to_string(),
        old_commit: old.to_string(),
        new_base: new_base.to_string(),
        new_commit: new.to_string(),
        limits: DiffLimits::default(),
        whitespace: WhitespaceMode::Exact,
    }
}
fn changed(comparison: &CommitComparison) -> Vec<(String, String)> {
    comparison
        .files
        .iter()
        .flat_map(|file| {
            let path = file
                .new_path
                .clone()
                .or(file.old_path.clone())
                .unwrap_or_default();
            file.hunks.iter().flat_map(move |hunk| {
                let path = path.clone();
                hunk.lines
                    .iter()
                    .filter(|line| line.kind != DiffLineKind::Context)
                    .map(move |line| {
                        (
                            path.clone(),
                            format!("{:?}:{}", line.kind, line.text.trim_end()),
                        )
                    })
            })
        })
        .collect()
}
fn objects(path: &Path) -> usize {
    fn walk(path: &Path) -> usize {
        std::fs::read_dir(path)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| {
                        if entry.path().is_dir() {
                            walk(&entry.path())
                        } else {
                            1
                        }
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
    walk(&path.join(".git").join("objects"))
}

const TEN: &str = "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n";

// r[verify kiln.review.interdiff.scenario.rebased]
#[test]
fn rebased_revision_interdiff_shows_only_the_authors_delta_and_writes_nothing() {
    let f = Fixture::new("rebased");
    let base1 = f.commit(&[], &[("file.txt", TEN), ("other.txt", "target\n")], None);
    let rev1 = f.commit(
        &[base1],
        &[
            ("file.txt", &TEN.replace("l2\n", "author-2\n")),
            ("other.txt", "target\n"),
        ],
        None,
    );
    // The target moves: another file and a distant line of the same file change.
    let base2 = f.commit(
        &[base1],
        &[
            ("file.txt", &TEN.replace("l9\n", "target-9\n")),
            ("other.txt", "target moved\n"),
        ],
        None,
    );
    // Revision 2 rebases revision 1 onto the moved target and edits one more line.
    let rev2_text = TEN
        .replace("l2\n", "author-2\n")
        .replace("l9\n", "target-9\n")
        .replace("l5\n", "author-5\n");
    let rev2 = f.commit(
        &[base2],
        &[("file.txt", &rev2_text), ("other.txt", "target moved\n")],
        None,
    );
    let provider = f.provider(all());
    let before = objects(&f.path);
    let Interdiff::Exact { comparison } = provider
        .interdiff(&request(base1, rev1, base2, rev2))
        .unwrap_or_else(|e| unreachable!("{e:?}"))
    else {
        unreachable!("clean rebase must be exact")
    };
    assert_eq!(
        changed(&comparison),
        vec![
            ("file.txt".to_string(), "Deletion:l5".to_string()),
            ("file.txt".to_string(), "Addition:author-5".to_string())
        ]
    );
    assert_eq!(comparison.old_commit, rev1.to_string());
    assert_eq!(comparison.new_commit, rev2.to_string());
    // New-side coordinates are exact revision 2 blob lines.
    let file = comparison
        .files
        .first()
        .unwrap_or_else(|| unreachable!("file"));
    let line = file
        .hunks
        .first()
        .unwrap_or_else(|| unreachable!("hunk"))
        .lines
        .iter()
        .find(|line| line.kind == DiffLineKind::Addition)
        .unwrap_or_else(|| unreachable!("addition"));
    assert_eq!(line.new_line, Some(5));
    assert_eq!(file.new_line_count, Some(10));
    // The in-memory rebase never writes objects into the repository.
    assert_eq!(objects(&f.path), before);
    // The plain exact comparison is unchanged and still shows target movement.
    let plain = provider
        .compare(&CompareRequest {
            old_commit: rev1.to_string(),
            new_commit: rev2.to_string(),
            limits: DiffLimits::default(),
            whitespace: WhitespaceMode::Exact,
        })
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let plain = changed(&plain);
    assert!(plain.contains(&("other.txt".to_string(), "Addition:target moved".to_string())));
    assert!(plain.contains(&("file.txt".to_string(), "Addition:target-9".to_string())));
    // Same base: the interdiff equals the exact commit comparison.
    let rev1b = f.commit(
        &[base1],
        &[
            ("file.txt", &TEN.replace("l2\n", "author-2b\n")),
            ("other.txt", "target\n"),
        ],
        None,
    );
    let Interdiff::Exact { comparison: same } = provider
        .interdiff(&request(base1, rev1, base1, rev1b))
        .unwrap_or_else(|e| unreachable!("{e:?}"))
    else {
        unreachable!("same base must be exact")
    };
    let direct = provider
        .compare(&CompareRequest {
            old_commit: rev1.to_string(),
            new_commit: rev1b.to_string(),
            limits: DiffLimits::default(),
            whitespace: WhitespaceMode::Exact,
        })
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(changed(&same), changed(&direct));
}

// r[verify kiln.review.interdiff.scenario.unavailable]
#[test]
fn conflicted_rebase_missing_commit_and_disabled_compare_are_explicit() {
    let f = Fixture::new("unavailable");
    let base1 = f.commit(&[], &[("file.txt", TEN)], None);
    let rev1 = f.commit(
        &[base1],
        &[("file.txt", &TEN.replace("l2\n", "author\n"))],
        None,
    );
    let base2 = f.commit(
        &[base1],
        &[("file.txt", &TEN.replace("l2\n", "target\n"))],
        None,
    );
    let rev2 = f.commit(
        &[base2],
        &[("file.txt", &TEN.replace("l2\n", "author again\n"))],
        None,
    );
    let provider = f.provider(all());
    let before = objects(&f.path);
    match provider.interdiff(&request(base1, rev1, base2, rev2)) {
        Ok(Interdiff::Unavailable { reason, .. }) => {
            assert_eq!(reason, InterdiffUnavailable::ConflictedRebase)
        }
        other => unreachable!("conflict must be unavailable, got {other:?}"),
    }
    assert_eq!(objects(&f.path), before);
    let missing = Oid::from_str("0123456789abcdef0123456789abcdef01234567")
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    match provider.interdiff(&request(missing, rev1, base2, rev2)) {
        Ok(Interdiff::Unavailable { reason, .. }) => {
            assert_eq!(reason, InterdiffUnavailable::MissingCommit)
        }
        other => unreachable!("missing base must be unavailable, got {other:?}"),
    }
    let mut malformed = request(base1, rev1, base2, rev2);
    malformed.old_base = "HEAD".into();
    assert_eq!(
        provider.interdiff(&malformed).err().map(|e| e.kind),
        Some(ReviewErrorKind::InvalidRequest)
    );
    let mut unbounded = request(base1, rev1, base2, rev2);
    unbounded.limits.max_files = 0;
    assert!(provider.interdiff(&unbounded).is_err());
    let disabled = f.provider(vec![ReviewCapability::Inspect]);
    assert_eq!(
        disabled
            .interdiff(&request(base1, rev1, base2, rev2))
            .err()
            .map(|e| e.kind),
        Some(ReviewErrorKind::Unsupported)
    );
}

// r[verify kiln.review.commit_range]
// r[verify kiln.review.interdiff.scenario.narrowed]
#[test]
fn revision_commits_list_change_ids_oldest_first_and_flag_foreign_bases() {
    let f = Fixture::new("commits");
    let base = f.commit(&[], &[("a.txt", "a\n")], None);
    let first = f.commit(&[base], &[("a.txt", "a1\n")], Some(CHANGE_A));
    let second = f.commit(&[first], &[("a.txt", "a2\n")], Some(CHANGE_B));
    let third = f.commit(&[second], &[("a.txt", "a3\n")], Some("NOT-A-CHANGE-ID"));
    let provider = f.provider(all());
    let listed = provider
        .revision_commits(&base.to_string(), &third.to_string(), MAX_REVISION_COMMITS)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(!listed.incomplete);
    let ids: Vec<_> = listed
        .commits
        .iter()
        .map(|c| (c.commit.clone(), c.change_id.clone()))
        .collect();
    assert_eq!(
        ids,
        vec![
            (first.to_string(), Some(CHANGE_A.to_string())),
            (second.to_string(), Some(CHANGE_B.to_string())),
            // An invalid header value is not a change identity.
            (third.to_string(), None),
        ]
    );
    assert_eq!(
        listed.commits.first().map(|c| c.parents.clone()),
        Some(vec![base.to_string()])
    );
    // A base that is not a first-parent ancestor, or a hit bound, is incomplete.
    let unrelated = f.commit(&[], &[("a.txt", "x\n")], None);
    let foreign = provider
        .revision_commits(
            &unrelated.to_string(),
            &third.to_string(),
            MAX_REVISION_COMMITS,
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(foreign.incomplete);
    let bounded = provider
        .revision_commits(&base.to_string(), &third.to_string(), 2)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(bounded.incomplete);
    assert_eq!(bounded.commits.len(), 2);
    assert!(
        provider
            .revision_commits("main", &third.to_string(), 4)
            .is_err()
    );
    // Blob reads are exact and bounded.
    let tree = f
        .repo
        .find_commit(first)
        .and_then(|c| c.tree())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let blob = tree
        .get_name("a.txt")
        .map(|entry| entry.id().to_string())
        .unwrap_or_default();
    assert_eq!(
        provider.blob(&blob, 1024).unwrap_or_default(),
        Some(b"a1\n".to_vec())
    );
    assert_eq!(provider.blob(&blob, 1).unwrap_or_default(), None);
}
