use std::collections::BTreeSet;

use kiln_codec::identity_from_bytes;
use kiln_core::source::{
    PijulFacts, SourceCapability, SourceNativeFacts, SourceResolution, admit_pijul_facts,
    admit_source_capability, build_pijul_snapshot_manifest, compute_pijul_observation_identity,
    validate_resolution,
};
use kiln_core::types::{Diagnostic, ExternalRef, ProviderProfileId};
use kiln_provider_api::{PijulReadRequest, PijulRepositoryPort, PortFailure, PortResult};

use crate::profile::{PINNED_FORMAT_COHORT, PijulSourceProfile, admit_profile};

const OBSERVATION_IDENTITY_PLACEHOLDER_INPUT: &[u8] = b"pijul-observation-pending";

// r[impl kiln.source.pijul_adapter]
// r[impl kiln.source.pijul_capabilities]
pub fn discover_channels<R: PijulRepositoryPort>(
    repository: &R,
    profile: &PijulSourceProfile,
) -> PortResult<Vec<ExternalRef>> {
    let admitted = admit_profile(profile)?;
    admit_source_capability(&admitted, SourceCapability::Discover, &profile.id)?;
    let channels = repository.discover_channels(profile.max_channels)?;
    if channels.len() > profile.max_channels {
        return Err(Diagnostic::new(
            "pijul_channel_bound",
            "pijul.channels",
            "Pijul channel count exceeds its named bound",
        )
        .into());
    }
    let mut unique = BTreeSet::new();
    for channel in &channels {
        channel.validate()?;
        if channel.provider_namespace.as_str() != "pijul"
            || channel.native_kind.as_str() != "channel"
        {
            return Err(Diagnostic::new(
                "pijul_channel_identity",
                "pijul.channels",
                "Pijul channel uses the wrong native identity namespace",
            )
            .into());
        }
        if !unique.insert(channel.clone()) {
            return Err(Diagnostic::new(
                "pijul_duplicate_channel",
                "pijul.channels",
                "Pijul discovery returned a duplicate channel",
            )
            .into());
        }
    }
    Ok(channels)
}

// r[impl kiln.source.pijul_adapter]
// r[impl kiln.source.pijul_state_identity]
// r[impl kiln.source.pijul_snapshot]
pub fn resolve_and_acquire<R: PijulRepositoryPort>(
    repository: &R,
    channel: ExternalRef,
    selected_profile: ProviderProfileId,
    profile: &PijulSourceProfile,
) -> PortResult<SourceResolution> {
    if selected_profile != profile.id {
        return Err(Diagnostic::new(
            "pijul_provider_substitution",
            "pijul.profile.id",
            "Pijul provider differs from the selected profile",
        )
        .into());
    }
    let admitted = admit_profile(profile)?;
    for capability in [
        SourceCapability::ResolveExact,
        SourceCapability::Acquire,
        SourceCapability::ObserveIdentity,
    ] {
        admit_source_capability(&admitted, capability, &selected_profile)?;
    }
    channel.validate()?;
    if channel.provider_namespace.as_str() != "pijul"
        || channel.native_kind.as_str() != "channel"
        || channel.native_id.as_str().len() > profile.limits.max_path_bytes
    {
        return Err(Diagnostic::new(
            "pijul_channel_identity",
            "pijul.channel",
            "Pijul channel request is invalid or exceeds its named bound",
        )
        .into());
    }
    let request = PijulReadRequest {
        channel: channel.clone(),
        limits: profile.limits.clone(),
    };
    let observation = repository.resolve_and_acquire(&request)?;
    if observation.channel != channel {
        return Err(Diagnostic::new(
            "pijul_channel_substitution",
            "pijul.channel",
            "Pijul observation differs from the requested channel",
        )
        .into());
    }
    if observation.format_cohort.provider_namespace.as_str() != "pijul"
        || observation.format_cohort.native_kind.as_str() != "format-cohort"
        || observation.format_cohort.native_id.as_str() != PINNED_FORMAT_COHORT
    {
        return Err(Diagnostic::new(
            "pijul_format_cohort",
            "pijul.format_cohort",
            "Pijul repository format differs from the selected cohort",
        )
        .into());
    }
    let mut facts = PijulFacts {
        provider_profile: selected_profile.clone(),
        repository: observation.repository,
        channel: observation.channel,
        state: observation.observed_state_before.clone(),
        observed_state_before: observation.observed_state_before,
        observed_state_after: observation.observed_state_after,
        change_sequence: observation.change_sequence,
        change_bound: profile.limits.max_changes,
        format_cohort: observation.format_cohort,
        observation_identity: identity_from_bytes(OBSERVATION_IDENTITY_PLACEHOLDER_INPUT).map_err(
            |_| {
                PortFailure::Domain(Diagnostic::new(
                    "pijul_observation_identity",
                    "pijul.observation_identity",
                    "Pijul observation identity could not be initialized",
                ))
            },
        )?,
    };
    facts.observation_identity = compute_pijul_observation_identity(&facts)?;
    admit_pijul_facts(&facts)?;
    let manifest = build_pijul_snapshot_manifest(&observation.entries, &profile.limits)?;
    let canonical_tree = identity_from_bytes(&manifest).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "pijul_tree_identity",
            "pijul.snapshot",
            "Pijul canonical tree identity could not be computed",
        ))
    })?;
    let resolution = SourceResolution {
        source_provider: selected_profile,
        repository: facts.repository.clone(),
        symbolic_reference: facts.channel.clone(),
        exact_revision: facts.state.clone(),
        canonical_tree,
        native_facts: SourceNativeFacts::Pijul(facts),
    };
    validate_resolution(&resolution)?;
    Ok(resolution)
}
