use kiln_core::change_review::{Anchor, Dependency, ReviewerRole, SearchQuery, VoteValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub request_id: String,
    pub change_id: Option<String>,
    pub expected_version: Option<u64>,
    pub action: Action,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Action {
    Contribute {
        source_ref: String,
        expected_commit: Option<String>,
        title: String,
        description: String,
        wip: bool,
    },
    ApplySuggestions {
        thread_ids: Vec<String>,
    },
    Subscribe {
        subscribed: bool,
    },
    Enqueue {
        expected_target: String,
    },
    Dequeue {},
    Create {
        commit: String,
        target_ref: String,
        title: String,
        description: String,
        wip: bool,
    },
    Upload {
        commit: String,
    },
    Edit {
        title: String,
        description: String,
    },
    SetWip {
        wip: bool,
    },
    Abandon {},
    Restore {},
    SetReviewer {
        principal: String,
        role: Option<ReviewerRole>,
    },
    Vote {
        value: VoteValue,
    },
    StartThread {
        anchor: Anchor,
        body: String,
        blocking: bool,
    },
    Reply {
        thread: String,
        body: String,
    },
    Resolve {
        thread: String,
        resolved: bool,
    },
    SaveDraft {
        id: Option<String>,
        anchor: Anchor,
        thread: Option<String>,
        body: String,
        blocking: bool,
    },
    DeleteDraft {
        id: String,
    },
    Publish {
        draft_ids: Vec<String>,
        vote: Option<VoteValue>,
    },
    MarkReviewed {
        file: String,
        reviewed: bool,
    },
    SetAttention {
        principal: String,
        attention: bool,
    },
    SetDependencies {
        dependencies: Vec<Dependency>,
    },
    SaveView {
        id: Option<String>,
        name: String,
        query: SearchQuery,
    },
    DeleteView {
        id: String,
    },
    ReadNotification {
        id: String,
        read: bool,
    },
    PrepareSubmit {},
    Submit {
        candidate_id: String,
    },
    Rebase {
        expected_target: String,
    },
    Revert {
        expected_target: String,
        title: String,
    },
    CherryPick {
        expected_target: String,
        target_ref: String,
        title: String,
    },
}
