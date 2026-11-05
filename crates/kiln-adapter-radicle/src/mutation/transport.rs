//! Native patch mutation: reduction and writes share the isolated upstream helper.
use super::model::{
    PATCH_SCHEMA_LINE, PATCH_TYPE_NAME, PatchVerdict, RadicleMutationProfile, admit_mutation_plan,
    admit_mutation_profile,
};
use crate::status::{ResolvedSigner, SignerResolver};
use kiln_core::cob::{CobEffectPlan, CobObservation};
use kiln_core::types::{CobSemanticStateId, ContentId, Diagnostic, ExternalRef};
use kiln_provider_api::{
    CollaborationMutationOutcome, CollaborationMutationPort, PortFailure, PortResult,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct RadicleMutationTransport {
    storage_root: PathBuf,
    profile: RadicleMutationProfile,
    resolver: SignerResolver,
    signer: Option<ResolvedSigner>,
    preview: Option<CobEffectPlan>,
}
impl std::fmt::Debug for RadicleMutationTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RadicleMutationTransport")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}
fn failure(message: impl AsRef<str>) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(
        "radicle_native_mutation",
        "radicle_mutation",
        message.as_ref(),
    ))
}
impl RadicleMutationTransport {
    pub fn new(storage_root: PathBuf, profile: RadicleMutationProfile) -> PortResult<Self> {
        admit_mutation_profile(&profile).map_err(PortFailure::Domain)?;
        Ok(Self::from_admitted(storage_root, profile))
    }
    fn from_admitted(storage_root: PathBuf, profile: RadicleMutationProfile) -> Self {
        Self {
            resolver: SignerResolver::new(
                profile.signer.clone(),
                profile.executable.path.clone(),
                profile.executable.blake3.clone(),
            ),
            storage_root,
            profile,
            signer: None,
            preview: None,
        }
    }
    pub const fn profile(&self) -> &RadicleMutationProfile {
        &self.profile
    }
    fn projection(&self, plan: &CobEffectPlan) -> PortResult<Value> {
        let started = Instant::now();
        admit_mutation_plan(&self.profile, plan).map_err(PortFailure::Domain)?;
        let file = std::fs::File::open(&self.profile.helper.path)
            .map_err(|_| failure("native helper unavailable"))?;
        if file
            .metadata()
            .map_err(|_| failure("native helper metadata unavailable"))?
            .len()
            > 134_217_728
        {
            return Err(failure("native helper executable bound exceeded"));
        }
        let mut hasher = blake3::Hasher::new();
        std::io::copy(&mut std::io::Read::take(file, 134_217_729), &mut hasher)
            .map_err(|_| failure("native helper hash failed"))?;
        if hasher.finalize().to_hex().as_str() != self.profile.helper.blake3 {
            return Err(failure("native helper identity drift"));
        }
        let projection = crate::patch_read::read_patch_with_limits(
            &self.profile.helper.path,
            &self.storage_root,
            plan.repository.native_id.as_str(),
            plan.object_id.as_str(),
            self.profile.max_history_entries,
            self.profile.max_history_bytes,
        )
        .map_err(failure)?;
        if started.elapsed() > Duration::from_millis(self.profile.deadlines.preflight_ms) {
            return Err(failure("native preflight deadline exceeded"));
        }
        Ok(projection)
    }
    fn observation(&self, plan: &CobEffectPlan, projection: &Value) -> PortResult<CobObservation> {
        let heads = projection["heads"]
            .as_array()
            .ok_or_else(|| failure("native head set missing"))?
            .iter()
            .map(|h| {
                ExternalRef::new(
                    "radicle",
                    "operation",
                    h.as_str().ok_or_else(|| failure("invalid native head"))?,
                )
                .map_err(PortFailure::Domain)
            })
            .collect::<PortResult<BTreeSet<_>>>()?;
        let bytes =
            serde_json::to_vec(projection).map_err(|_| failure("native projection invalid"))?;
        let identity = blake3::hash(&bytes).to_hex().to_string();
        Ok(CobObservation {
            profile_id: plan.profile_id.clone(),
            repository: plan.repository.clone(),
            type_name: ExternalRef::new("radicle", "cob-type", PATCH_TYPE_NAME)
                .map_err(PortFailure::Domain)?,
            schema_line: ExternalRef::new("radicle", "schema", PATCH_SCHEMA_LINE)
                .map_err(PortFailure::Domain)?,
            object_id: plan.object_id.clone(),
            heads,
            semantic_state: CobSemanticStateId::new(identity.clone())
                .map_err(PortFailure::Domain)?,
            projection_identity: ContentId::from_blake3_hex(identity)
                .map_err(PortFailure::Domain)?,
        })
    }
    fn own_review(&self, plan: &CobEffectPlan, projection: &Value) -> PortResult<Option<bool>> {
        let action = admit_mutation_plan(&self.profile, plan).map_err(PortFailure::Domain)?;
        let revisions = projection["revisions"]
            .as_array()
            .ok_or_else(|| failure("native revisions missing"))?;
        let revision = revisions
            .iter()
            .find(|r| r["id"].as_str() == Some(action.revision.as_str()))
            .ok_or_else(|| failure("review revision missing or redacted"))?;
        let reviews = revision["reviews"]
            .as_array()
            .ok_or_else(|| failure("native reviews missing"))?;
        let verdict = match action.verdict {
            PatchVerdict::Accept => "accept",
            PatchVerdict::Reject => "reject",
        };
        Ok(reviews
            .iter()
            .find(|r| r["author"].as_str() == Some(plan.actor.native_id.as_str()))
            .map(|r| {
                r["verdict"].as_str() == Some(verdict)
                    && r["summary"].as_str() == Some(action.summary.as_str())
            }))
    }
}
// r[impl kiln.radicle.shell_mutation]
impl CollaborationMutationPort for RadicleMutationTransport {
    fn observe(&mut self, plan: &CobEffectPlan) -> PortResult<CobObservation> {
        self.observation(plan, &self.projection(plan)?)
    }
    // r[impl kiln.radicle.shell_mutation.scenario.ambient]
    fn acquire_signer(&mut self, actor: &ExternalRef) -> PortResult<()> {
        self.signer = None;
        self.preview = None;
        let started = Instant::now();
        let signer = self.resolver.resolve("append own native Radicle review")?;
        if actor.provider_namespace.as_str() != "radicle"
            || actor.native_kind.as_str() != "node"
            || actor.native_id.as_str() != signer.public_key()
            || started.elapsed() > Duration::from_millis(self.profile.deadlines.signer_ms)
        {
            return Err(failure("signer identity or deadline mismatch"));
        }
        self.signer = Some(signer);
        Ok(())
    }
    // r[impl kiln.radicle.mutation_scope.scenario.owned]
    fn preview(&mut self, plan: &CobEffectPlan) -> PortResult<()> {
        self.preview = None;
        if self.signer.as_ref().map(ResolvedSigner::public_key)
            != Some(plan.actor.native_id.as_str())
        {
            return Err(failure("signer missing or substituted"));
        }
        let observed = self.observe(plan)?;
        if observed.heads != plan.expected_heads {
            return Err(failure("native head set changed since preview"));
        }
        self.own_review(plan, &self.projection(plan)?)?;
        self.preview = Some(plan.clone());
        Ok(())
    }
    // r[impl kiln.radicle.shell_mutation.scenario.authorized]
    // r[impl kiln.radicle.mutation_conformance.scenario.repeat]
    fn dispatch(&mut self, plan: &CobEffectPlan) -> PortResult<CollaborationMutationOutcome> {
        if self.preview.take().as_ref() != Some(plan) {
            return Err(failure("mutation lacks exact accepted preview"));
        }
        let signer = self
            .signer
            .take()
            .ok_or_else(|| failure("signer missing"))?;
        let projection = self.projection(plan)?;
        if let Some(exact) = self.own_review(plan, &projection)? {
            return if exact {
                Ok(CollaborationMutationOutcome::Applied(Box::new(
                    self.observation(plan, &projection)?,
                )))
            } else {
                Err(failure("own review already exists with different action"))
            };
        }
        if self.observation(plan, &projection)?.heads != plan.expected_heads {
            return Err(failure("native head set changed before signing"));
        }
        let action = admit_mutation_plan(&self.profile, plan).map_err(PortFailure::Domain)?;
        let heads = plan
            .expected_heads
            .iter()
            .map(|h| h.native_id.as_str().to_owned())
            .collect();
        let verdict = match action.verdict {
            PatchVerdict::Accept => "accept",
            PatchVerdict::Reject => "reject",
        };
        let started = Instant::now();
        let result = crate::patch_read::review_patch(
            &self.profile.helper.path,
            &self.storage_root,
            crate::patch_read::ReviewRequest {
                repository_id: plan.repository.native_id.as_str(),
                patch_id: plan.object_id.as_str(),
                revision_id: &action.revision,
                verdict,
                summary: &action.summary,
                expected_heads: &heads,
            },
            signer.helper_seed(),
        );
        // A child failure may occur after a local signed write: never blindly retry it.
        if result.is_err()
            || started.elapsed() > Duration::from_millis(self.profile.deadlines.write_ms)
        {
            return Ok(CollaborationMutationOutcome::Unknown);
        }
        self.reconcile(plan)
    }
    // r[impl kiln.radicle.mutation_bounds.scenario.reconcile]
    fn reconcile(&mut self, plan: &CobEffectPlan) -> PortResult<CollaborationMutationOutcome> {
        self.signer = None;
        self.preview = None;
        let projection = self.projection(plan)?;
        Ok(match self.own_review(plan, &projection)? {
            Some(true) => CollaborationMutationOutcome::Applied(Box::new(
                self.observation(plan, &projection)?,
            )),
            Some(false) => CollaborationMutationOutcome::ProviderError,
            None => CollaborationMutationOutcome::Unknown,
        })
    }
}
