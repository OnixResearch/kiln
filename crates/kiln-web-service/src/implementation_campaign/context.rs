//! Exact, bounded decision projections; files remain inputs, never authority.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path},
};

use kiln_core::{
    implementation::campaign::{
        CampaignState, Freshness, MAX_CONTEXT_ITEMS, MAX_CONTEXT_MANIFEST_BYTES, MemberBinding,
        PendingEffect, RetainedAttemptContext,
    },
    types::ContentId,
};
use kiln_implementer::campaign::{CampaignObservation, CampaignStore};
use serde_json::{Value, json};

use super::{ImplementationCampaignDriveProfile, id, selected_member_binding, text};

pub(super) type Projections = BTreeMap<ContentId, Vec<u8>>;

pub(super) fn validate(profile: &ImplementationCampaignDriveProfile) -> Result<(), String> {
    if profile.decision_contexts.len() > MAX_CONTEXT_ITEMS
        || profile.decision_contexts.iter().any(|(identity, path)| {
            identity.validate().is_err()
                || !path.is_absolute()
                || !path.starts_with("/nix/store")
                || path
                    .components()
                    .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        })
    {
        return Err(
            "decision projections require bounded identities and immutable absolute store paths"
                .into(),
        );
    }
    Ok(())
}

fn read_projection(path: &Path, identity: &ContentId) -> Result<Vec<u8>, String> {
    let resolved = fs::canonicalize(path).map_err(text)?;
    if !resolved.starts_with("/nix/store") {
        return Err("decision projection resolves outside the immutable store".into());
    }
    let metadata = fs::metadata(&resolved).map_err(text)?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_CONTEXT_MANIFEST_BYTES as u64
    {
        return Err("decision projection is empty or exceeds its byte bound".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(resolved)
        .map_err(text)?
        .take(MAX_CONTEXT_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() > MAX_CONTEXT_MANIFEST_BYTES
        || id(blake3::hash(&bytes).to_hex().as_str())? != *identity
    {
        return Err("decision projection differs from its exact declared identity".into());
    }
    Ok(bytes)
}

pub(super) fn load(profile: &ImplementationCampaignDriveProfile) -> Result<Projections, String> {
    profile
        .decision_contexts
        .iter()
        .map(|(identity, path)| {
            read_projection(path, identity).map(|bytes| (identity.clone(), bytes))
        })
        .collect()
}

pub(super) fn freshness(binding: &MemberBinding, projections: &Projections) -> Freshness {
    if binding.context.is_empty() {
        return Freshness::NotApplicable;
    }
    let required = binding.context.iter().any(|entry| entry.required);
    if binding
        .context
        .iter()
        .filter(|entry| !required || entry.required)
        .all(|entry| projections.contains_key(&entry.projection_identity))
    {
        Freshness::Current
    } else {
        Freshness::Unknown
    }
}

pub(super) fn retain(
    store: &mut CampaignStore,
    campaign_id: &str,
    mut version: u64,
    state: &CampaignState,
    member: &str,
    attempt: &kiln_core::implementation::ImplementationAttempt,
    projections: &Projections,
) -> Result<u64, String> {
    let binding = selected_member_binding(&state.contract, member)?;
    let identities = binding
        .context
        .iter()
        .map(|entry| &entry.projection_identity)
        .collect::<BTreeSet<_>>();
    let retained = state.attempt_contexts.get(&attempt.identity);
    let mut contexts = Vec::new();
    for identity in identities {
        let Some(bytes) = projections.get(identity) else {
            continue;
        };
        let context = RetainedAttemptContext {
            attempt_identity: attempt.identity.clone(),
            work_identity: attempt.work_identity.clone(),
            selection_identity: state.selection_identity.clone(),
            context_identity: identity.clone(),
            exact_manifest: bytes.clone(),
            decision_revisions: binding
                .context
                .iter()
                .filter(|entry| &entry.projection_identity == identity)
                .map(|entry| entry.revision.clone())
                .collect(),
        };
        if let Some(existing) = retained.and_then(|entries| entries.get(identity)) {
            if existing != &context {
                return Err("active attempt cannot replace its frozen decision projection".into());
            }
        } else {
            contexts.push(context);
        }
    }
    if !contexts.is_empty() {
        store.retain_attempt_contexts(campaign_id, version, contexts)?;
        version = store
            .version(campaign_id)?
            .ok_or("campaign disappeared after retaining decision context")?;
    }
    Ok(version)
}

pub(super) fn prompt(
    profile: &ImplementationCampaignDriveProfile,
    effect: &PendingEffect,
) -> Result<Value, String> {
    use base64::Engine;
    let observer = CampaignObservation::open(&profile.campaign_database)?;
    let (state, _) = observer
        .inspect(&profile.campaign_id)?
        .ok_or("campaign decision context is unavailable")?;
    if effect.selection_identity != state.selection_identity {
        return Err("decision projection belongs to another frozen selection".into());
    }
    let binding = selected_member_binding(&state.contract, &effect.member)?;
    let member = state
        .observations
        .get(&effect.member)
        .ok_or("decision context member is absent")?;
    if member.context_identity != effect.context_identity {
        return Err("decision context changed after effect intent".into());
    }
    let retained = effect
        .attempt_identity
        .as_ref()
        .and_then(|attempt| state.attempt_contexts.get(attempt));
    let identities = binding
        .context
        .iter()
        .map(|entry| &entry.projection_identity)
        .collect::<BTreeSet<_>>();
    let mut manifests = Vec::new();
    for identity in identities {
        let Some(projection) = retained.and_then(|entries| entries.get(identity)) else {
            if binding
                .context
                .iter()
                .any(|entry| entry.required && &entry.projection_identity == identity)
            {
                return Err(
                    "required decision projection was not retained for the exact attempt".into(),
                );
            }
            manifests.push(json!({"identity": identity, "status": "unavailable"}));
            continue;
        };
        if projection.selection_identity != effect.selection_identity
            || Some(&projection.work_identity) != effect.work_identity.as_ref()
        {
            return Err(
                "retained decision projection belongs to another selection or work version".into(),
            );
        }
        let manifest = match std::str::from_utf8(&projection.exact_manifest) {
            Ok(text) => {
                json!({"identity": identity, "status": "retained", "encoding": "utf8", "manifest": text})
            }
            Err(_) => {
                json!({"identity": identity, "status": "retained", "encoding": "base64", "manifest": base64::engine::general_purpose::STANDARD.encode(&projection.exact_manifest)})
            }
        };
        manifests.push(manifest);
    }
    Ok(
        json!({"bindings": binding.context, "projections": manifests, "non_claim": "decision guidance is not verification, resource or delivery authority"}),
    )
}
