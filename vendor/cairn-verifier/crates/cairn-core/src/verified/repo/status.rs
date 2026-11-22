pub fn build_repo_status(
    root: String,
    mut specs: Vec<String>,
    changes: Vec<super::ChangeSource>,
) -> super::RepoStatus {
    specs.sort();
    let mut changes = changes
        .into_iter()
        .map(super::changes::build_change_status)
        .collect::<Vec<_>>();
    changes.sort_by(|left, right| left.name.cmp(&right.name));
    super::RepoStatus {
        root,
        specs,
        changes,
    }
}
