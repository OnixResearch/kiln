use kiln_core::change_review::{Anchor, Change, ChangeStatus, Comment, Revision, Side, Thread};
use kiln_provider_api::change_review::{
    ChangeReviewPort, CommitComparison, CommitInspection, CompareRequest, DiffFile, DiffHunk,
    ReviewEffect, ReviewError, ReviewErrorKind, ReviewOperationRequest, ReviewProviderInfo,
    ReviewResult,
};

use super::thread_outcome;

struct Provider {
    comparison: CommitComparison,
}

impl ChangeReviewPort for Provider {
    fn info(&self) -> ReviewProviderInfo {
        ReviewProviderInfo {
            version: 1,
            repository_id: "repository".into(),
            capabilities: Vec::new(),
            target_refs: Vec::new(),
        }
    }

    fn inspect(&self, _commit: &str, _target_ref: &str) -> ReviewResult<CommitInspection> {
        Err(ReviewError {
            kind: ReviewErrorKind::Unsupported,
            message: "inspect unavailable".into(),
        })
    }

    fn compare(&self, _request: &CompareRequest) -> ReviewResult<CommitComparison> {
        Ok(self.comparison.clone())
    }

    fn execute(&self, _request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect> {
        Err(ReviewError {
            kind: ReviewErrorKind::Unsupported,
            message: "execute unavailable".into(),
        })
    }

    fn reconcile(&self, _request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect> {
        Err(ReviewError {
            kind: ReviewErrorKind::Unsupported,
            message: "reconcile unavailable".into(),
        })
    }
}

fn comparison(new_start: u32, new_lines: u32) -> CommitComparison {
    CommitComparison {
        repository_id: "repository".into(),
        old_commit: "base".into(),
        new_commit: "revision-two".into(),
        files: vec![DiffFile {
            old_path: Some("src/lib.rs".into()),
            new_path: Some("src/lib.rs".into()),
            old_blob: None,
            new_blob: None,
            old_line_count: None,
            new_line_count: None,
            renamed: false,
            binary: false,
            truncated: false,
            hunks: vec![DiffHunk {
                old_start: 1,
                old_lines: 1,
                new_start,
                new_lines,
                header: String::new(),
                lines: Vec::new(),
            }],
        }],
        truncated: false,
    }
}

fn change(resolved_revision: Option<&str>, resolved: bool) -> Change {
    let revision = |id: &str, number: u32| Revision {
        id: id.into(),
        number,
        commit: id.into(),
        base: "base".into(),
        files: vec!["src/lib.rs".into()],
        author: "alice".into(),
        created_ms: i64::from(number),
        authorship: Default::default(),
        submitted_by: None,
    };
    Change {
        id: "change".into(),
        project: "project".into(),
        repository: "repository".into(),
        target_ref: "refs/heads/main".into(),
        owner: "alice".into(),
        title: "change".into(),
        description: String::new(),
        state: ChangeStatus::Open,
        wip: false,
        revisions: vec![revision("revision-one", 1), revision("revision-two", 2)],
        reviewers: Default::default(),
        votes: Vec::new(),
        threads: vec![Thread {
            id: "thread".into(),
            anchor: Anchor {
                revision: "revision-one".into(),
                base_commit: "base".into(),
                file: "src/lib.rs".into(),
                side: Side::Revision,
                start_line: 2,
                end_line: 2,
            },
            blocking: true,
            resolved,
            resolved_revision: resolved_revision.map(str::to_owned),
            comments: vec![Comment {
                id: "comment".into(),
                author: "agent".into(),
                body: "objection".into(),
                created_ms: 2,
            }],
        }],
        reviewed: Vec::new(),
        reviewed_hunks: Vec::new(),
        last_viewed: Default::default(),
        attention: Default::default(),
        dependencies: Vec::new(),
        candidate: None,
        submission: None,
        maintenance: Vec::new(),
        advisory: Vec::new(),
    }
}

#[test]
fn outcomes_use_resolved_revision_and_anchor_overlap() {
    let addressed = change(Some("revision-two"), true);
    let dismissed = change(Some("revision-two"), true);
    let open = change(None, false);
    let unknown = change(None, true);
    assert_eq!(
        thread_outcome(
            &Provider {
                comparison: comparison(2, 1)
            },
            &addressed,
            &addressed.threads[0]
        ),
        "addressed"
    );
    assert_eq!(
        thread_outcome(
            &Provider {
                comparison: comparison(10, 1)
            },
            &dismissed,
            &dismissed.threads[0]
        ),
        "dismissed"
    );
    assert_eq!(
        thread_outcome(
            &Provider {
                comparison: comparison(2, 1)
            },
            &open,
            &open.threads[0]
        ),
        "open"
    );
    assert_eq!(
        thread_outcome(
            &Provider {
                comparison: comparison(2, 1)
            },
            &unknown,
            &unknown.threads[0]
        ),
        "unknown"
    );
}
