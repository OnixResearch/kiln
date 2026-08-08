use kiln_codec::identity_from_bytes;
use kiln_core::source::{JujutsuBackend, JujutsuFacts, SourceResolution, validate_resolution};
use kiln_core::types::{Alias, CoreResult, Diagnostic, ExternalRef, ProviderProfileId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JujutsuObservation {
    pub repository: ExternalRef,
    pub backend: JujutsuBackend,
    pub operation: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JujutsuResolution {
    pub symbolic: ExternalRef,
    pub change_id: ExternalRef,
    pub commits: Vec<ExternalRef>,
    pub bookmark: Option<Alias>,
    pub conflicted_bookmark: bool,
    pub divergent_change: bool,
}

pub trait JujutsuRepository {
    fn observe(&self) -> CoreResult<JujutsuObservation>;
    fn resolve(&self, symbolic: &ExternalRef) -> CoreResult<JujutsuResolution>;
    fn acquire_canonical_manifest(&self, commit: &ExternalRef) -> CoreResult<Vec<u8>>;
}

pub fn resolve_and_acquire<R: JujutsuRepository>(
    repository: &R,
    symbolic: &ExternalRef,
    provider_profile: ProviderProfileId,
) -> CoreResult<SourceResolution> {
    let observation = repository.observe()?;
    if observation.backend != JujutsuBackend::Git {
        return Err(Diagnostic::new(
            "unsupported_backend",
            "jujutsu.backend",
            "only the admitted Git backend is supported",
        ));
    }
    let resolved = repository.resolve(symbolic)?;
    let Some(commit) = resolved.commits.first() else {
        return Err(Diagnostic::new(
            "missing_revision",
            "jujutsu.commits",
            "Jujutsu reference did not resolve",
        ));
    };
    if resolved.commits.len() != 1 || resolved.conflicted_bookmark || resolved.divergent_change {
        return Err(Diagnostic::new(
            "ambiguous_revision",
            "jujutsu.commits",
            "Jujutsu reference is ambiguous, conflicted, or divergent",
        ));
    }
    let manifest = repository.acquire_canonical_manifest(commit)?;
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
        exact_revision: commit.clone(),
        canonical_tree: tree,
        jujutsu: Some(JujutsuFacts {
            backend: observation.backend,
            operation_identity: observation.operation,
            change_id: resolved.change_id,
            bookmark: resolved.bookmark,
            resolved_commits: resolved.commits,
            conflicted_bookmark: resolved.conflicted_bookmark,
            divergent_change: resolved.divergent_change,
        }),
    };
    validate_resolution(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixtureRepository {
        commits: Vec<ExternalRef>,
        divergent: bool,
    }

    impl JujutsuRepository for FixtureRepository {
        fn observe(&self) -> CoreResult<JujutsuObservation> {
            Ok(JujutsuObservation {
                repository: ExternalRef::new("jj", "repository", "repo")?,
                backend: JujutsuBackend::Git,
                operation: ExternalRef::new("jj", "operation", "op")?,
            })
        }

        fn resolve(&self, symbolic: &ExternalRef) -> CoreResult<JujutsuResolution> {
            Ok(JujutsuResolution {
                symbolic: symbolic.clone(),
                change_id: ExternalRef::new("jj", "change", "change")?,
                commits: self.commits.clone(),
                bookmark: Some(Alias::new("main")?),
                conflicted_bookmark: false,
                divergent_change: self.divergent,
            })
        }

        fn acquire_canonical_manifest(&self, _commit: &ExternalRef) -> CoreResult<Vec<u8>> {
            Ok(b"file\0identity".to_vec())
        }
    }

    #[test]
    fn resolves_exact_commit_and_tree() {
        let repository = FixtureRepository {
            commits: vec![
                ExternalRef::new("jj", "commit", "abc").unwrap_or_else(|_| unreachable!()),
            ],
            divergent: false,
        };
        let result = resolve_and_acquire(
            &repository,
            &ExternalRef::new("jj", "bookmark", "main").unwrap_or_else(|_| unreachable!()),
            ProviderProfileId::new("jj-v1").unwrap_or_else(|_| unreachable!()),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_divergent_change() {
        let repository = FixtureRepository {
            commits: vec![
                ExternalRef::new("jj", "commit", "abc").unwrap_or_else(|_| unreachable!()),
                ExternalRef::new("jj", "commit", "def").unwrap_or_else(|_| unreachable!()),
            ],
            divergent: true,
        };
        let result = resolve_and_acquire(
            &repository,
            &ExternalRef::new("jj", "change", "change").unwrap_or_else(|_| unreachable!()),
            ProviderProfileId::new("jj-v1").unwrap_or_else(|_| unreachable!()),
        );
        assert!(result.is_err());
    }
}
