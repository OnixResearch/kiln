use kiln_codec::identity_from_bytes;
use kiln_core::source::{
    JujutsuBackend, JujutsuFacts, SourceNativeFacts, SourceResolution, admit_jujutsu_facts,
    validate_resolution,
};
use kiln_core::types::{Diagnostic, ExternalRef, ProviderProfileId};
use kiln_provider_api::{PortFailure, PortResult, SourceRepositoryBackend, SourceRepositoryPort};

pub fn resolve_and_acquire<R: SourceRepositoryPort>(
    repository: &R,
    symbolic: &ExternalRef,
    provider_profile: ProviderProfileId,
) -> PortResult<SourceResolution> {
    let observation = repository.observe()?;
    let resolved = repository.resolve(symbolic)?;
    let operation_identity = observation.operation.ok_or_else(|| {
        PortFailure::Domain(Diagnostic::new(
            "jujutsu_operation_missing",
            "jujutsu.operation",
            "Jujutsu repository observation has no operation identity",
        ))
    })?;
    let change_id = resolved.native_change.ok_or_else(|| {
        PortFailure::Domain(Diagnostic::new(
            "jujutsu_change_missing",
            "jujutsu.change_id",
            "Jujutsu resolution has no change identity",
        ))
    })?;
    let jujutsu = JujutsuFacts {
        backend: match observation.backend {
            SourceRepositoryBackend::Git => JujutsuBackend::Git,
            SourceRepositoryBackend::Other => JujutsuBackend::Other,
        },
        operation_identity,
        change_id,
        bookmark: resolved.alias,
        resolved_commits: resolved.exact_revisions,
        conflicted_bookmark: resolved.conflicted_alias,
        divergent_change: resolved.divergent_change,
    };
    let commit = admit_jujutsu_facts(&jujutsu)
        .map_err(PortFailure::Domain)?
        .clone();
    let manifest = repository.acquire_canonical_manifest(&commit)?;
    let tree = identity_from_bytes(&manifest).map_err(|_| {
        Diagnostic::new(
            "tree_identity",
            "jujutsu.manifest",
            "canonical tree identity failed",
        )
    })?;
    let result = SourceResolution {
        source_provider: provider_profile,
        repository: observation.repository,
        symbolic_reference: resolved.symbolic,
        exact_revision: commit,
        canonical_tree: tree,
        native_facts: SourceNativeFacts::Jujutsu(jujutsu),
    };
    validate_resolution(&result).map_err(PortFailure::Domain)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use kiln_core::types::Alias;
    use kiln_provider_api::{SourceRepositoryObservation, SourceResolutionObservation};

    use super::*;

    struct FixtureRepository {
        commits: Vec<ExternalRef>,
        divergent: bool,
        acquisitions: Cell<usize>,
    }

    impl FixtureRepository {
        fn new(commits: Vec<ExternalRef>, divergent: bool) -> Self {
            Self {
                commits,
                divergent,
                acquisitions: Cell::new(0),
            }
        }
    }

    impl SourceRepositoryPort for FixtureRepository {
        fn observe(&self) -> PortResult<SourceRepositoryObservation> {
            Ok(SourceRepositoryObservation {
                repository: ExternalRef::new("jj", "repository", "repo")?,
                backend: SourceRepositoryBackend::Git,
                operation: Some(ExternalRef::new("jj", "operation", "op")?),
            })
        }

        fn resolve(&self, symbolic: &ExternalRef) -> PortResult<SourceResolutionObservation> {
            Ok(SourceResolutionObservation {
                symbolic: symbolic.clone(),
                native_change: Some(ExternalRef::new("jj", "change", "change")?),
                exact_revisions: self.commits.clone(),
                alias: Some(Alias::new("main")?),
                conflicted_alias: false,
                divergent_change: self.divergent,
            })
        }

        fn acquire_canonical_manifest(&self, _commit: &ExternalRef) -> PortResult<Vec<u8>> {
            self.acquisitions.set(self.acquisitions.get() + 1);
            Ok(b"file\0identity".to_vec())
        }
    }

    #[test]
    fn resolves_exact_commit_and_tree() {
        let repository = FixtureRepository::new(
            vec![ExternalRef::new("jj", "commit", "abc").unwrap_or_else(|_| unreachable!())],
            false,
        );
        let result = resolve_and_acquire(
            &repository,
            &ExternalRef::new("jj", "bookmark", "main").unwrap_or_else(|_| unreachable!()),
            ProviderProfileId::new("jj-v1").unwrap_or_else(|_| unreachable!()),
        );
        assert!(result.is_ok());
        assert_eq!(repository.acquisitions.get(), 1);
    }

    // r[verify kiln.providers.policy_effect_split]
    // r[verify kiln.providers.boundary_conformance]
    #[test]
    fn rejects_divergent_change_before_acquisition() {
        let repository = FixtureRepository::new(
            vec![
                ExternalRef::new("jj", "commit", "abc").unwrap_or_else(|_| unreachable!()),
                ExternalRef::new("jj", "commit", "def").unwrap_or_else(|_| unreachable!()),
            ],
            true,
        );
        let result = resolve_and_acquire(
            &repository,
            &ExternalRef::new("jj", "change", "change").unwrap_or_else(|_| unreachable!()),
            ProviderProfileId::new("jj-v1").unwrap_or_else(|_| unreachable!()),
        );
        assert!(result.is_err());
        assert_eq!(repository.acquisitions.get(), 0);
    }
}
