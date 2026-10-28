//! Read-only local source admission. Provider writes remain in the existing
//! durable Create/Upload journal; this module never updates Git references.
// r[impl kiln.review.parity.intake]
use super::*;
use git2::{Repository, RepositoryOpenFlags};

const SOURCE_PREFIX: &str = "refs/heads/review/";
const MAX_SOURCE_REF_BYTES: usize = 1024;
const MAX_SOURCE_MAPPINGS: usize = 128;
const MAX_CONTRIBUTIONS: usize = MAX_CHANGES;
const MAX_SOURCE_ROWS: usize = 256;
const MAX_SOURCE_SCAN: usize = 4096;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeProfile {
    pub default_target_ref: String,
    #[serde(default)]
    pub mappings: Vec<SourceMapping>,
}

/// An operator-admitted exact branch/owner binding outside (or overriding) the
/// default review namespace. These bindings never confer target-write authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMapping {
    pub source_ref: String,
    pub target_ref: String,
    pub principal: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Contribution {
    pub principal: String,
    pub change_id: String,
    pub target_ref: String,
    /// Last successfully registered commit, not a claim from an incoming event.
    pub commit: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Plan {
    Create {
        contribution: Contribution,
        commit: String,
    },
    Upload {
        contribution: Contribution,
        commit: String,
        expected_version: u64,
    },
    Replay {
        change_id: String,
    },
}

fn valid_ref(value: &str) -> bool {
    value.len() <= MAX_SOURCE_REF_BYTES
        && value.starts_with("refs/heads/")
        && git2::Reference::is_valid_name(value)
}

fn valid_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl IntakeProfile {
    pub fn admit(
        &self,
        provider: &LocalGitReviewProfile,
        writers: &[String],
    ) -> Result<(), String> {
        if !provider.target_refs.contains(&self.default_target_ref)
            || self.mappings.len() > MAX_SOURCE_MAPPINGS
        {
            return Err("review intake default target or mapping bound rejected".into());
        }
        let mut sources = BTreeSet::new();
        for mapping in &self.mappings {
            if !valid_ref(&mapping.source_ref)
                || provider.target_refs.contains(&mapping.source_ref)
                || !provider.target_refs.contains(&mapping.target_ref)
                || !writers.contains(&mapping.principal)
                || !sources.insert(&mapping.source_ref)
            {
                return Err(
                    "review intake source, target, owner or duplicate mapping rejected".into(),
                );
            }
        }
        Ok(())
    }

    fn target<'a>(
        &'a self,
        provider: &LocalGitReviewProfile,
        actor: &str,
        source_ref: &str,
    ) -> Result<&'a str, Failure> {
        if !valid_ref(source_ref) {
            return reject(400, "review_source_ref_invalid");
        }
        if provider
            .target_refs
            .iter()
            .any(|target| target == source_ref)
        {
            return reject(403, "review_source_ref_protected");
        }
        if let Some(mapping) = self.mappings.iter().find(|m| m.source_ref == source_ref) {
            if mapping.principal != actor {
                return reject(403, "review_source_owner_denied");
            }
            return Ok(&mapping.target_ref);
        }
        if !source_ref.starts_with(SOURCE_PREFIX) {
            return reject(403, "review_source_ref_not_admitted");
        }
        Ok(&self.default_target_ref)
    }
}

fn repository(provider: &LocalGitReviewProfile) -> Result<Repository, Failure> {
    let repository = Repository::open_ext(
        &provider.repository_path,
        RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&Path>(),
    )
    .map_err(|_| Failure::Api(409, "review_source_repository_unavailable".into()))?;
    let actual = repository
        .workdir()
        .unwrap_or(repository.path())
        .canonicalize()
        .map_err(|_| Failure::Unavailable)?;
    let expected = provider
        .repository_path
        .canonicalize()
        .map_err(|_| Failure::Unavailable)?;
    if actual != expected || repository.is_shallow() {
        return reject(409, "review_source_repository_rejected");
    }
    Ok(repository)
}

/// Observe only the exact configured local repository and a direct branch ref.
/// No revparse expressions, symbolic refs, remote URLs, fetches or shell commands
/// are accepted. A moved ref after this observation cannot change the pinned
/// commit carried into the existing provider operation journal.
fn observe(provider: &LocalGitReviewProfile, source_ref: &str) -> Result<String, Failure> {
    let repository = repository(provider)?;
    let reference = repository
        .find_reference(source_ref)
        .map_err(|_| Failure::Api(404, "review_source_ref_missing".into()))?;
    let commit = reference
        .target()
        .ok_or_else(|| Failure::Api(400, "review_source_ref_symbolic".into()))?;
    repository
        .find_commit(commit)
        .map_err(|_| Failure::Api(400, "review_source_ref_not_commit".into()))?;
    Ok(commit.to_string())
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SourceView {
    source_ref: String,
    target_ref: String,
    commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    change_id: Option<String>,
}

/// Actor-filtered, bounded local branch discovery for the authenticated submit
/// surface. Its commit is advisory until plan() reobserves it under the command
/// transaction; clients may supply it as expected_commit for optimistic safety.
pub(super) fn sources(
    profile: &Profile,
    state: &State,
    actor: &str,
) -> Result<Vec<SourceView>, Failure> {
    let Some(intake) = &profile.intake else {
        return Ok(Vec::new());
    };
    if !profile
        .write_principals
        .iter()
        .any(|principal| principal == actor)
    {
        return Ok(Vec::new());
    }
    let repository = repository(&profile.provider)?;
    let mut candidates = BTreeSet::new();
    let references = repository
        .references_glob("refs/heads/review/*")
        .map_err(|_| Failure::Unavailable)?;
    for (index, reference) in references.enumerate() {
        if index >= MAX_SOURCE_SCAN {
            return reject(409, "review_source_scan_bound");
        }
        let reference = reference.map_err(|_| Failure::Unavailable)?;
        if let Some(name) = reference.name() {
            candidates.insert(name.to_owned());
        }
    }
    candidates.extend(
        intake
            .mappings
            .iter()
            .filter(|mapping| mapping.principal == actor)
            .map(|mapping| mapping.source_ref.clone()),
    );
    let mut result = Vec::new();
    for source_ref in candidates {
        let Ok(target_ref) = intake.target(&profile.provider, actor, &source_ref) else {
            continue;
        };
        let contribution = state.contributions.get(&source_ref);
        if contribution.is_some_and(|mapping| mapping.principal != actor) {
            continue;
        }
        let Ok(reference) = repository.find_reference(&source_ref) else {
            continue;
        };
        let Some(commit) = reference.target() else {
            continue;
        };
        if repository.find_commit(commit).is_err() {
            continue;
        }
        if result.len() >= MAX_SOURCE_ROWS {
            return reject(409, "review_source_list_bound");
        }
        result.push(SourceView {
            source_ref,
            target_ref: target_ref.into(),
            commit: commit.to_string(),
            change_id: contribution.map(|mapping| mapping.change_id.clone()),
        });
    }
    Ok(result)
}

/// Resolve an admitted local source into one stable review. The caller must
/// persist the returned mapping in its existing operation reservation before
/// provider execution, and set `commit` only on successful Register completion.
/// Pending mappings are deliberately retained on failure so another principal
/// cannot claim a source branch during reconciliation or a later retry.
pub(super) fn plan(
    profile: &Profile,
    state: &State,
    actor: &str,
    source_ref: &str,
    expected_commit: Option<&str>,
) -> Result<Plan, Failure> {
    let target_ref = admit_source(profile, state, actor, source_ref, expected_commit)?;
    let commit = observe(&profile.provider, source_ref)?;
    admit_commit(
        profile,
        state,
        actor,
        source_ref,
        target_ref,
        commit,
        expected_commit,
    )
}

fn admit_source<'a>(
    profile: &'a Profile,
    state: &State,
    actor: &str,
    source_ref: &str,
    expected_commit: Option<&str>,
) -> Result<&'a str, Failure> {
    let intake = profile
        .intake
        .as_ref()
        .ok_or_else(|| Failure::Api(403, "review_source_intake_disabled".into()))?;
    if !profile
        .write_principals
        .iter()
        .any(|principal| principal == actor)
    {
        return reject(403, "review_source_owner_denied");
    }
    let target_ref = intake.target(&profile.provider, actor, source_ref)?;
    if expected_commit.is_some_and(|commit| !valid_commit(commit)) {
        return reject(400, "review_source_commit_invalid");
    }
    let prior = state.contributions.get(source_ref);
    if let Some(contribution) = prior {
        if contribution.principal != actor {
            return reject(403, "review_source_owner_denied");
        }
        if contribution.target_ref != target_ref {
            return reject(409, "review_source_mapping_changed");
        }
        if state.operations.iter().any(|operation| {
            matches!(operation.state.as_str(), "pending" | "uncertain")
                && (operation.change == contribution.change_id
                    || operation.result_change.as_ref() == Some(&contribution.change_id))
        }) {
            return reject(409, "review_change_reserved");
        }
    } else if state.contributions.len() >= MAX_CONTRIBUTIONS {
        return reject(409, "review_source_mapping_bound");
    }
    Ok(target_ref)
}

fn admit_commit(
    profile: &Profile,
    state: &State,
    actor: &str,
    source_ref: &str,
    target_ref: &str,
    commit: String,
    expected_commit: Option<&str>,
) -> Result<Plan, Failure> {
    let prior = state.contributions.get(source_ref);
    if expected_commit.is_some_and(|expected| expected != commit) {
        return reject(409, "review_source_commit_moved");
    }
    let change_id = hash(&json!([
        "kiln.review.contribution.v1",
        profile.provider.repository_id,
        source_ref
    ]))?;
    let contribution = prior.cloned().unwrap_or_else(|| Contribution {
        principal: actor.into(),
        change_id: change_id.clone(),
        target_ref: target_ref.into(),
        commit: None,
    });
    if contribution.change_id != change_id {
        return Err(Failure::Unavailable);
    }
    let Some(change) = state.review.changes.get(&change_id) else {
        if contribution.commit.is_some() {
            return Err(Failure::Unavailable);
        }
        return Ok(Plan::Create {
            contribution,
            commit,
        });
    };
    if prior.is_none()
        || change.owner != actor
        || change.project != profile.project
        || change.repository != profile.provider.repository_id
        || change.target_ref != target_ref
    {
        return reject(409, "review_source_change_mismatch");
    }
    // Replayed branch deliveries do not create revisions, reset approvals or
    // edit metadata, including after the exact revision has been submitted.
    if change.current()?.commit == commit {
        return Ok(Plan::Replay { change_id });
    }
    if change.state != domain::ChangeStatus::Open {
        return reject(409, "review_source_change_not_open");
    }
    if change
        .revisions
        .iter()
        .any(|revision| revision.commit == commit)
    {
        return reject(409, "review_source_commit_replayed");
    }
    Ok(Plan::Upload {
        contribution,
        commit,
        expected_version: state.versions.get(&change_id).copied().unwrap_or(0),
    })
}
