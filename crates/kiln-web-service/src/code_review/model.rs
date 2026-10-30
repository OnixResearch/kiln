use kiln_core::change_review::{
    Anchor, Authorship, Dependency, ReviewerRole, SearchQuery, VoteValue,
};
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
        /// Self-declared and unverified; omitted when undeclared so request
        /// digests of pre-authorship requests stay byte-identical.
        // r[impl kiln.review.authorship.scenario.legacy]
        #[serde(default, skip_serializing_if = "Authorship::is_undeclared")]
        authorship: Authorship,
    },
    /// Change-ID intake: exact jj change IDs (bottom to top) observed through
    /// the configured jj workspace, or an admitted branch whose commits carry
    /// the git `change-id` header. Exactly one source is admitted.
    // r[impl kiln.review.jj_intake]
    JjContribute {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        change_ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_ref: Option<String>,
        #[serde(default)]
        stack: bool,
        #[serde(default)]
        wip: bool,
        #[serde(default, skip_serializing_if = "Authorship::is_undeclared")]
        authorship: Authorship,
    },
    /// Internal, service-planned member step of a change-ID intake. Never
    /// deserialized, so clients cannot bypass jj admission with it.
    #[serde(skip_deserializing)]
    JjMember {
        member: Box<super::jj_intake::Member>,
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
        #[serde(default, skip_serializing_if = "Authorship::is_undeclared")]
        authorship: Authorship,
    },
    Upload {
        commit: String,
        #[serde(default, skip_serializing_if = "Authorship::is_undeclared")]
        authorship: Authorship,
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
    MarkHunk {
        file: String,
        hunk_digest: String,
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
    /// Non-binding approval of the current revision; admitted for read principals.
    AdvisoryApprove {},
    AdvisoryWithdraw {},
}

impl Action {
    /// The only actions a read-only principal may send.
    // r[impl kiln.review.advisory.scenario.forbidden]
    pub(super) fn read_admitted(&self) -> bool {
        matches!(self, Self::AdvisoryApprove {} | Self::AdvisoryWithdraw {})
    }
}
