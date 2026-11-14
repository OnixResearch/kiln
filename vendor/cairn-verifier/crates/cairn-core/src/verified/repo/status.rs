use super::changes::build_change_status;
use super::{ChangeSource, RepoStatus};

pub fn build_repo_status(
    root: String,
    mut specs: Vec<String>,
    changes: Vec<ChangeSource>,
) -> RepoStatus {
    specs.sort();
    let mut changes = changes
        .into_iter()
        .map(build_change_status)
        .collect::<Vec<_>>();
    changes.sort_by(|left, right| left.name.cmp(&right.name));
    RepoStatus {
        root,
        specs,
        changes,
    }
}
