use crate::verified::cross_repo_dependencies::CrossRepoDependency;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoStatus {
    pub root: String,
    pub specs: Vec<String>,
    pub changes: Vec<ChangeStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecSource {
    pub id: String,
    pub path: String,
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecSummary {
    pub id: String,
    pub path: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecDetail {
    pub id: String,
    pub path: String,
    pub title: Option<String>,
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeList {
    pub changes: Vec<ChangeStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeDetail {
    pub change: ChangeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecList {
    pub specs: Vec<SpecSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSource {
    pub name: String,
    pub path: String,
    pub has_proposal: bool,
    pub has_design: bool,
    pub has_tasks: bool,
    pub delta_specs: Vec<String>,
    pub tasks_markdown: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeMetadata {
    pub groups: Vec<String>,
    pub target_date: Option<String>,
    pub status: Option<String>,
    pub depends_on: Vec<String>,
    pub cross_repo_dependencies: Vec<CrossRepoDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeStatus {
    pub name: String,
    pub path: String,
    pub has_proposal: bool,
    pub has_design: bool,
    pub has_tasks: bool,
    pub delta_specs: Vec<String>,
    pub task_counts: Option<TaskCounts>,
    pub metadata: ChangeMetadata,
    pub metadata_issues: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskCounts {
    pub total: usize,
    pub done: usize,
    pub in_progress: usize,
    pub todo: usize,
    pub unmarked: usize,
}
