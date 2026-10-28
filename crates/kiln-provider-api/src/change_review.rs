//! Versioned, provider-neutral ports for exact-commit code review.
//! r[impl kiln.review.parity.ergonomics]
//! r[impl kiln.review.parity.queue]
use serde::{Deserialize, Serialize};

pub const REVIEW_PORT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewCapability {
    Inspect,
    Compare,
    Register,
    Submit,
    Rebase,
    Revert,
    CherryPick,
    ApplySuggestions,
    BuildIntegration,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewProviderInfo {
    pub version: u32,
    pub repository_id: String,
    pub capabilities: Vec<ReviewCapability>,
    pub target_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewErrorKind {
    InvalidRequest,
    Unsupported,
    NotFound,
    TargetMoved,
    Conflict,
    Storage,
    Uncertain,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewError {
    pub kind: ReviewErrorKind,
    pub message: String,
}
pub type ReviewResult<T> = Result<T, ReviewError>;

impl core::fmt::Display for ReviewError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for ReviewError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitInspection {
    pub repository_id: String,
    pub commit: String,
    pub parents: Vec<String>,
    pub target_ref: String,
    pub target_commit: String,
    pub ancestor_of_target: bool,
    pub target_is_ancestor: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffLimits {
    pub max_files: u32,
    pub max_lines: u32,
    pub max_bytes: u32,
}
impl Default for DiffLimits {
    fn default() -> Self {
        Self {
            max_files: 256,
            max_lines: 20_000,
            max_bytes: 2_000_000,
        }
    }
}

/// Presentation-only filtering; line coordinates always identify original blobs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhitespaceMode {
    #[default]
    Exact,
    IgnoreAll,
    IgnoreChange,
    IgnoreEol,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompareRequest {
    pub old_commit: String,
    pub new_commit: String,
    pub limits: DiffLimits,
    #[serde(default)]
    pub whitespace: WhitespaceMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
    NoNewline,
}

/// A single unified line. The same row's optional old/new coordinates are the
/// canonical side-by-side mapping; no position is inferred from display offsets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,
    pub lines: Vec<DiffLine>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffFile {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub old_blob: Option<String>,
    pub new_blob: Option<String>,
    pub old_line_count: Option<u32>,
    pub new_line_count: Option<u32>,
    pub renamed: bool,
    pub binary: bool,
    pub truncated: bool,
    pub hunks: Vec<DiffHunk>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitComparison {
    pub repository_id: String,
    pub old_commit: String,
    pub new_commit: String,
    pub files: Vec<DiffFile>,
    pub truncated: bool,
}

/// Identity and time are explicit so retries create the identical Git commit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewSignature {
    pub name: String,
    pub email: String,
    pub time_seconds: i64,
}

/// Exact inclusive, one-based whole-line replacement on the immutable source.
/// `expected_original` includes the original line endings, including absent EOF
/// newline. Empty replacement deletes the range. All ranges use source coordinates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionEdit {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub expected_original: String,
    pub replacement: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStrategy {
    Merge,
    Rebase,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewOperation {
    Register {
        commit: String,
    },
    Submit {
        commit: String,
        expected_target: String,
    },
    Rebase {
        commit: String,
        onto: String,
    },
    Revert {
        commit: String,
        onto: String,
    },
    CherryPick {
        commit: String,
        onto: String,
    },
    /// Creates one immutable child commit; never advances the target.
    ApplySuggestions {
        commit: String,
        suggestions: Vec<SuggestionEdit>,
    },
    /// Builds a separately CI-qualified candidate. Original source review
    /// authority must remain bound to `commit`, not transferred to this result.
    BuildIntegration {
        commit: String,
        expected_target: String,
        strategy: IntegrationStrategy,
    },
}
impl ReviewOperation {
    #[must_use]
    pub fn capability(&self) -> ReviewCapability {
        match self {
            Self::Register { .. } => ReviewCapability::Register,
            Self::Submit { .. } => ReviewCapability::Submit,
            Self::Rebase { .. } => ReviewCapability::Rebase,
            Self::Revert { .. } => ReviewCapability::Revert,
            Self::CherryPick { .. } => ReviewCapability::CherryPick,
            Self::ApplySuggestions { .. } => ReviewCapability::ApplySuggestions,
            Self::BuildIntegration { .. } => ReviewCapability::BuildIntegration,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewOperationRequest {
    pub version: u32,
    /// Shell-owned BLAKE3 identity (64 lowercase hexadecimal digits).
    pub operation_id: String,
    pub repository_id: String,
    pub target_ref: String,
    pub operation: ReviewOperation,
    pub signature: ReviewSignature,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewEffect {
    Applied {
        commit: String,
        target_commit: String,
    },
    NotApplied {
        target_commit: String,
    },
    Uncertain {
        message: String,
    },
}

/// Authorization/policy belongs to the shell. Every mutation requires its
/// durable exact request; providers must reject identity reuse and fail closed.
pub trait ChangeReviewPort {
    fn info(&self) -> ReviewProviderInfo;
    fn inspect(&self, commit: &str, target_ref: &str) -> ReviewResult<CommitInspection>;
    fn compare(&self, request: &CompareRequest) -> ReviewResult<CommitComparison>;
    fn execute(&self, request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect>;
    fn reconcile(&self, request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect>;
}
