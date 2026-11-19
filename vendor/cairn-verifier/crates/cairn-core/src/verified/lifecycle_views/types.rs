pub const LIFECYCLE_VIEW_SCHEMA_V1: &str = "cairn.lifecycle-view.v1";
pub const LIFECYCLE_VIEW_BOUNDARY: &str = "lifecycle views project declared Cairn state and linkage; they do not prove requirement satisfaction, implementation correctness, verification soundness, lifecycle readiness, or release eligibility";
pub const LIFECYCLE_VIEW_NON_CLAIMS: &[&str] = &[
    "not requirement satisfaction",
    "not implementation correctness",
    "not verification soundness",
    "not lifecycle readiness",
    "not release eligibility",
];
const KANBAN_COLUMN_COUNT: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleViewInput {
    pub repository: String,
    pub changes: Vec<LifecycleViewChangeInput>,
    pub specs: Vec<crate::verified::repo::SpecSource>,
    pub dependency_graph: crate::verified::change_dependencies::ChangeDependencyGraph,
    pub traceability: Option<crate::verified::traceability::TraceabilityCoverageReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleViewChangeInput {
    pub status: crate::verified::repo::ChangeStatus,
    pub tasks_markdown: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LifecycleViewNodeKind {
    Repository,
    Change,
    Spec,
    Requirement,
    Scenario,
    Task,
    SourceArtifact,
    MissingRequirement,
}

impl LifecycleViewNodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Change => "change",
            Self::Spec => "spec",
            Self::Requirement => "requirement",
            Self::Scenario => "scenario",
            Self::Task => "task",
            Self::SourceArtifact => "source_artifact",
            Self::MissingRequirement => "missing_requirement",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LifecycleViewNode {
    pub id: String,
    pub kind: LifecycleViewNodeKind,
    pub label: String,
    pub reference: Option<String>,
    pub state: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LifecycleViewEdgeKind {
    Contains,
    Changes,
    Covers,
    DependsOn,
    Blocks,
    Implements,
    Verifies,
    TraceabilityDepends,
    Related,
}

impl LifecycleViewEdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Changes => "changes",
            Self::Covers => "covers",
            Self::DependsOn => "depends_on",
            Self::Blocks => "blocks",
            Self::Implements => "implements",
            Self::Verifies => "verifies",
            Self::TraceabilityDepends => "traceability_depends",
            Self::Related => "related",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LifecycleViewEdge {
    pub source: String,
    pub target: String,
    pub kind: LifecycleViewEdgeKind,
    pub state: Option<String>,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LifecycleViewIssue {
    pub code: String,
    pub message: String,
    pub source: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KanbanColumnKind {
    Planned,
    Ready,
    InProgress,
    Blocked,
    ReadyToArchive,
}

impl KanbanColumnKind {
    pub const ORDERED: [Self; KANBAN_COLUMN_COUNT] = [
        Self::Planned,
        Self::Ready,
        Self::InProgress,
        Self::Blocked,
        Self::ReadyToArchive,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Ready => "ready",
            Self::InProgress => "in_progress",
            Self::Blocked => "blocked",
            Self::ReadyToArchive => "ready_to_archive",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanCard {
    pub change: String,
    pub metadata_status: Option<String>,
    pub task_counts: Option<crate::verified::repo::TaskCounts>,
    pub dependencies: Vec<String>,
    pub blocked_by: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanColumn {
    pub kind: KanbanColumnKind,
    pub cards: Vec<KanbanCard>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanBoard {
    pub columns: Vec<KanbanColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleMindmapNode {
    pub id: String,
    pub kind: LifecycleViewNodeKind,
    pub label: String,
    pub children: Vec<LifecycleMindmapNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleViewModel {
    pub schema: String,
    pub repository: String,
    pub valid: bool,
    pub nodes: Vec<LifecycleViewNode>,
    pub edges: Vec<LifecycleViewEdge>,
    pub issues: Vec<LifecycleViewIssue>,
    pub kanban: KanbanBoard,
    pub mindmap: LifecycleMindmapNode,
    pub boundary: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ParsedTaskState {
    Todo,
    InProgress,
    Done,
}

impl ParsedTaskState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Done => "done",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedTask {
    pub id: String,
    pub label: String,
    pub state: ParsedTaskState,
    pub covers: Vec<String>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedScenario {
    pub id: String,
    pub reference: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedRequirement {
    pub id: String,
    pub reference: Option<String>,
    pub label: String,
    pub scenarios: Vec<ParsedScenario>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedSpec {
    pub id: String,
    pub label: String,
    pub path: String,
    pub owner_change: Option<String>,
    pub requirements: Vec<ParsedRequirement>,
}
