use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{Alias, ContentId, CoreResult, Diagnostic, ExternalRef, ProviderProfileId};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SourceCapability {
    Discover,
    ResolveExact,
    Acquire,
    ObserveIdentity,
    Collaborate,
    ReceiveTrigger,
    PublishStatus,
    UseCredentials,
    Mutate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceProviderProfile {
    pub id: ProviderProfileId,
    pub native_system: ExternalRef,
    pub capabilities: BTreeSet<SourceCapability>,
    pub identity_namespaces: BTreeSet<Alias>,
    pub immutable_resolution: bool,
}

pub fn admit_source_capability(
    profile: &SourceProviderProfile,
    capability: SourceCapability,
    expected_profile: &ProviderProfileId,
) -> CoreResult<()> {
    if &profile.id != expected_profile {
        return Err(Diagnostic::new(
            "source_provider_substitution",
            "source_provider.id",
            "source provider differs from the selected profile",
        ));
    }
    profile.native_system.validate()?;
    if !profile.capabilities.contains(&capability) {
        return Err(Diagnostic::new(
            "source_capability_missing",
            "source_provider.capabilities",
            "source provider does not declare the requested narrow capability",
        ));
    }
    if matches!(
        capability,
        SourceCapability::ResolveExact | SourceCapability::Acquire
    ) && !profile.immutable_resolution
    {
        return Err(Diagnostic::new(
            "moving_source_resolution",
            "source_provider.immutable_resolution",
            "source acquisition requires an immutable exact resolution",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceTriggerObservation {
    pub provider: ProviderProfileId,
    pub native_event: ExternalRef,
    pub actor: ExternalRef,
    pub exact_revision: ExternalRef,
    pub signature_observed: bool,
    pub signature_valid: bool,
    pub replayed: bool,
}

pub fn admit_source_trigger(
    profile: &SourceProviderProfile,
    trigger: &SourceTriggerObservation,
) -> CoreResult<()> {
    admit_source_capability(profile, SourceCapability::ReceiveTrigger, &trigger.provider)?;
    trigger.native_event.validate()?;
    trigger.actor.validate()?;
    trigger.exact_revision.validate()?;
    if trigger.replayed {
        return Err(Diagnostic::new(
            "replayed_source_trigger",
            "source_trigger.native_event",
            "source trigger was already admitted",
        ));
    }
    if trigger.signature_observed && !trigger.signature_valid {
        return Err(Diagnostic::new(
            "invalid_source_signature",
            "source_trigger.signature",
            "observed source trigger signature is invalid",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum JujutsuBackend {
    Git,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct JujutsuFacts {
    pub backend: JujutsuBackend,
    pub operation_identity: ExternalRef,
    pub change_id: ExternalRef,
    pub bookmark: Option<Alias>,
    pub resolved_commits: Vec<ExternalRef>,
    pub conflicted_bookmark: bool,
    pub divergent_change: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceResolution {
    pub source_provider: ProviderProfileId,
    pub repository: ExternalRef,
    pub symbolic_reference: ExternalRef,
    pub exact_revision: ExternalRef,
    pub canonical_tree: ContentId,
    pub jujutsu: Option<JujutsuFacts>,
}

pub fn validate_resolution(resolution: &SourceResolution) -> CoreResult<()> {
    resolution.repository.validate()?;
    resolution.symbolic_reference.validate()?;
    resolution.exact_revision.validate()?;
    if let Some(facts) = &resolution.jujutsu {
        validate_jujutsu(facts, &resolution.exact_revision)?;
    }
    Ok(())
}

// r[impl kiln.providers.policy_effect_split]
pub fn admit_jujutsu_facts(facts: &JujutsuFacts) -> CoreResult<&ExternalRef> {
    if facts.backend != JujutsuBackend::Git {
        return Err(Diagnostic::new(
            "unsupported_backend",
            "jujutsu.backend",
            "Jujutsu backend is not admitted",
        ));
    }
    if facts.conflicted_bookmark {
        return Err(Diagnostic::new(
            "conflicted_bookmark",
            "jujutsu.bookmark",
            "Jujutsu bookmark is conflicted",
        ));
    }
    if facts.divergent_change {
        return Err(Diagnostic::new(
            "divergent_change",
            "jujutsu.change_id",
            "Jujutsu change is divergent",
        ));
    }
    if facts.resolved_commits.len() != 1 {
        return Err(Diagnostic::new(
            "ambiguous_revision",
            "jujutsu.resolved_commits",
            "Jujutsu reference must resolve to exactly one commit",
        ));
    }
    let Some(resolved) = facts.resolved_commits.first() else {
        return Err(Diagnostic::new(
            "missing_revision",
            "jujutsu.resolved_commits",
            "Jujutsu reference did not resolve",
        ));
    };
    Ok(resolved)
}

fn validate_jujutsu(facts: &JujutsuFacts, exact_revision: &ExternalRef) -> CoreResult<()> {
    let resolved = admit_jujutsu_facts(facts)?;
    if resolved != exact_revision {
        return Err(Diagnostic::new(
            "revision_mismatch",
            "jujutsu.resolved_commits",
            "resolved Jujutsu commit differs from the exact source revision",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProposedChangeCandidate {
    pub base: SourceResolution,
    pub head: SourceResolution,
    pub strategy: Alias,
    pub result: SourceResolution,
}

pub fn validate_candidate(candidate: &ProposedChangeCandidate) -> CoreResult<()> {
    validate_resolution(&candidate.base)?;
    validate_resolution(&candidate.head)?;
    validate_resolution(&candidate.result)?;
    if candidate.base.source_provider != candidate.result.source_provider
        || candidate.head.source_provider != candidate.result.source_provider
    {
        return Err(Diagnostic::new(
            "provider_mismatch",
            "candidate.result",
            "candidate snapshots use different source providers",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MirrorDisagreementPolicy {
    Deny,
    PreferCanonical,
}

pub fn compare_mirrors(
    canonical: &SourceResolution,
    mirror: &SourceResolution,
    policy: MirrorDisagreementPolicy,
) -> CoreResult<bool> {
    validate_resolution(canonical)?;
    validate_resolution(mirror)?;
    if canonical.exact_revision == mirror.exact_revision
        && canonical.canonical_tree == mirror.canonical_tree
    {
        return Ok(true);
    }
    match policy {
        MirrorDisagreementPolicy::Deny => Err(Diagnostic::new(
            "mirror_disagreement",
            "source_mirror",
            "source mirrors disagree",
        )),
        MirrorDisagreementPolicy::PreferCanonical => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn external(namespace: &str, kind: &str, value: &str) -> ExternalRef {
        ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!())
    }

    fn tree() -> ContentId {
        ContentId::from_blake3_hex(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn admits_exact_jujutsu_commit() {
        let commit = external("jj", "commit", "abc");
        let resolution = SourceResolution {
            source_provider: ProviderProfileId::new("jj-source-v1")
                .unwrap_or_else(|_| unreachable!()),
            repository: external("jj", "repository", "repo"),
            symbolic_reference: external("jj", "bookmark", "main"),
            exact_revision: commit.clone(),
            canonical_tree: tree(),
            jujutsu: Some(JujutsuFacts {
                backend: JujutsuBackend::Git,
                operation_identity: external("jj", "operation", "op"),
                change_id: external("jj", "change", "change"),
                bookmark: Some(Alias::new("main").unwrap_or_else(|_| unreachable!())),
                resolved_commits: alloc::vec![commit],
                conflicted_bookmark: false,
                divergent_change: false,
            }),
        };
        assert_eq!(validate_resolution(&resolution), Ok(()));
    }

    #[test]
    fn narrow_source_profile_does_not_gain_forge_authority() {
        let profile = SourceProviderProfile {
            id: ProviderProfileId::new("jj-source-v1").unwrap_or_else(|_| unreachable!()),
            native_system: external("jj", "source-provider", "git-backend"),
            capabilities: BTreeSet::from([
                SourceCapability::Discover,
                SourceCapability::ResolveExact,
                SourceCapability::Acquire,
                SourceCapability::ObserveIdentity,
            ]),
            identity_namespaces: BTreeSet::from([
                Alias::new("jj-commit").unwrap_or_else(|_| unreachable!()),
                Alias::new("jj-change").unwrap_or_else(|_| unreachable!()),
            ]),
            immutable_resolution: true,
        };
        assert_eq!(
            admit_source_capability(&profile, SourceCapability::Acquire, &profile.id),
            Ok(())
        );
        assert!(
            admit_source_capability(&profile, SourceCapability::Collaborate, &profile.id).is_err()
        );
    }

    #[test]
    fn native_source_profiles_keep_distinct_identity_namespaces() {
        for (provider_name, namespace) in [
            ("git-source-v1", "git-commit"),
            ("pijul-source-v1", "pijul-change"),
            ("jj-source-v1", "jj-change"),
            ("github-collaboration-v1", "github-pull-request"),
            ("radicle-collaboration-v1", "radicle-patch"),
        ] {
            let profile = SourceProviderProfile {
                id: ProviderProfileId::new(provider_name).unwrap_or_else(|_| unreachable!()),
                native_system: external(provider_name, "provider", "fixture"),
                capabilities: BTreeSet::from([SourceCapability::ObserveIdentity]),
                identity_namespaces: BTreeSet::from([
                    Alias::new(namespace).unwrap_or_else(|_| unreachable!())
                ]),
                immutable_resolution: true,
            };
            assert!(
                profile
                    .identity_namespaces
                    .iter()
                    .any(|value| value.as_str() == namespace)
            );
        }
    }

    #[test]
    fn replayed_or_invalidly_signed_trigger_fails_closed() {
        let profile = SourceProviderProfile {
            id: ProviderProfileId::new("radicle-trigger-v1").unwrap_or_else(|_| unreachable!()),
            native_system: external("radicle", "trigger-provider", "node"),
            capabilities: BTreeSet::from([SourceCapability::ReceiveTrigger]),
            identity_namespaces: BTreeSet::new(),
            immutable_resolution: true,
        };
        let mut trigger = SourceTriggerObservation {
            provider: profile.id.clone(),
            native_event: external("radicle", "event", "event"),
            actor: external("radicle", "actor", "did:key:actor"),
            exact_revision: external("radicle", "revision", "revision"),
            signature_observed: true,
            signature_valid: false,
            replayed: false,
        };
        assert!(admit_source_trigger(&profile, &trigger).is_err());
        trigger.signature_valid = true;
        trigger.replayed = true;
        assert!(admit_source_trigger(&profile, &trigger).is_err());
    }

    #[test]
    fn rejects_ambiguous_jujutsu_revision() {
        let first = external("jj", "commit", "a");
        let second = external("jj", "commit", "b");
        let facts = JujutsuFacts {
            backend: JujutsuBackend::Git,
            operation_identity: external("jj", "operation", "op"),
            change_id: external("jj", "change", "change"),
            bookmark: None,
            resolved_commits: alloc::vec![first.clone(), second],
            conflicted_bookmark: false,
            divergent_change: true,
        };
        assert!(validate_jujutsu(&facts, &first).is_err());
    }
}
