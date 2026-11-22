//! Test support: the shared restricted corpus and a fake verified
//! source-authority port standing in for the Basalt `kiln://source/v1`
//! adapter until it exists.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use serde::Deserialize;

use super::authority::{
    CapabilityContext, EffectiveScopeFacts, EffectiveSourceScope, SessionCapability, SnapshotRule,
    SourceAuthority, SourceAuthorityDecision, SourceAuthorityRequest, SourceCurrentness,
    SourceDenialReason, SourceGrant, check_request,
};
use super::disclosure::{
    CairnItem, CairnObservation, HandleKey, ScopedEnumeration, ScopedSelection, ViewCeiling,
};
use super::sharing::{SharePlan, ShareRequest};
use super::{
    AdmittedDefinition, AdmittedPartialView, AdmittedRepository, ComponentVersion, Coverage,
    DependencyFact, DependencyStatus, EntryKind, ProjectionDefinition, ProjectionFacts,
    ProviderBinding, RepositoryEnrollment, RepositoryLocator, SourceCapability, SourceEntry,
    admit_definition, admit_repositories, resolve,
};
use crate::implementation::authority::{
    AuthorityCredential, AuthorityProfile, AuthorityProvenance, AuthoritySessionId,
};
use crate::types::{ContentId, ExternalRef, ProviderProfileId};

/// `nickel export --format json config/fixtures/source-projection-restricted-positive.ncl`.
const CORPUS_JSON: &str =
    include_str!("../../../../fixtures/source-projection/restricted/corpus.json");

pub(super) const CORPUS_SCHEMA: &str = "kiln.source-projection.restricted-corpus.v1";
pub(super) const GENERATION: u64 = 1;
pub(super) const REVOCATION_GENERATION: u64 = 0;

pub(super) fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}

pub(super) fn digest(label: &str) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(label.as_bytes()).to_hex().to_string(),
    ))
}

/// The view binding corpus scopes were admitted through.
pub(super) fn corpus_view_binding() -> ContentId {
    digest("corpus-view")
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CorpusRepository {
    pub handle: String,
    pub provider: String,
    pub native: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CorpusEntry {
    pub path: String,
    pub blob: String,
    pub kind: EntryKind,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CorpusDependency {
    pub name: String,
    pub identity_label: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CorpusHolder {
    pub audience: String,
    pub grants: Vec<SourceGrant>,
    pub not_after_s: u64,
    pub depth: u8,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CorpusExpect {
    pub visible: Vec<String>,
    pub cairn: Vec<String>,
    pub hidden_prerequisites: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CorpusView {
    pub name: String,
    pub definition: ProjectionDefinition,
    pub holder: CorpusHolder,
    pub expect: CorpusExpect,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CorpusShare {
    pub name: String,
    pub view: String,
    pub sharer: String,
    pub request: ShareRequest,
    pub expect: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Corpus {
    pub schema: String,
    pub repository: CorpusRepository,
    pub revision: ExternalRef,
    pub now_s: u64,
    pub entries: Vec<CorpusEntry>,
    pub dependencies: Vec<CorpusDependency>,
    pub cairn: Vec<CairnItem>,
    pub views: Vec<CorpusView>,
    pub shares: Vec<CorpusShare>,
}

impl Corpus {
    pub fn load() -> Self {
        let corpus: Self = ok(serde_json::from_str(CORPUS_JSON));
        assert_eq!(corpus.schema, CORPUS_SCHEMA);
        corpus
    }

    pub fn view(&self, name: &str) -> &CorpusView {
        self.views
            .iter()
            .find(|view| view.name == name)
            .unwrap_or_else(|| unreachable!("corpus view {name}"))
    }

    pub fn repository(&self) -> AdmittedRepository {
        let enrollment = RepositoryEnrollment {
            repository_id: self.repository.handle.clone(),
            binding: ProviderBinding {
                source_provider: ok(ProviderProfileId::new(self.repository.provider.as_str())),
                native_repository: ok(ExternalRef::new(
                    "github",
                    "repository",
                    &self.repository.native,
                )),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/srv/cache/restricted-corpus".into(),
            }],
        };
        ok(admit_repositories(&[enrollment], &[]))
            .pop()
            .unwrap_or_else(|| unreachable!("one admitted repository"))
    }

    /// Facts of the corpus snapshot at `revision`.
    pub fn facts_at(&self, revision: &ExternalRef) -> ProjectionFacts {
        let repository = self.repository();
        ProjectionFacts {
            repository: repository.identity().clone(),
            exact_revision: revision.clone(),
            canonical_tree: digest(&alloc::format!("tree:{}", revision.native_id.as_str())),
            evaluator: ComponentVersion {
                name: "kiln-projection".into(),
                version: "1".into(),
            },
            adapter: ComponentVersion {
                name: "git-source".into(),
                version: "1".into(),
            },
            source_capabilities: vec![
                SourceCapability::ExactTree,
                SourceCapability::ExecutableBit,
                SourceCapability::Symlinks,
            ],
            coverage: Coverage::Complete,
            entries: self
                .entries
                .iter()
                .map(|entry| SourceEntry {
                    path: entry.path.clone(),
                    object: ok(ExternalRef::new("git", "blob", &entry.blob)),
                    kind: entry.kind,
                })
                .collect(),
            dependencies: self
                .dependencies
                .iter()
                .map(|dependency| DependencyFact {
                    name: dependency.name.clone(),
                    status: DependencyStatus::Present {
                        identity: digest(&dependency.identity_label),
                    },
                })
                .collect(),
            generated: Vec::new(),
        }
    }

    pub fn facts(&self) -> ProjectionFacts {
        self.facts_at(&self.revision)
    }

    pub fn definition(&self, view: &str) -> AdmittedDefinition {
        ok(admit_definition(&self.view(view).definition))
    }

    pub fn ceiling(&self, view: &str) -> ViewCeiling {
        ViewCeiling::from_definition(&self.definition(view))
    }

    /// The enumeration of a corpus view, scoped to that view's holder.
    pub fn trusted(&self, view: &str) -> ScopedEnumeration {
        self.trusted_with(view, &self.definition(view), &self.facts())
    }

    /// The enumeration of `facts` under `definition`, scoped to the named
    /// view's holder through the definition's ceiling.
    pub fn trusted_with(
        &self,
        view: &str,
        definition: &AdmittedDefinition,
        facts: &ProjectionFacts,
    ) -> ScopedEnumeration {
        self.enumerate(
            definition,
            facts,
            &self.scope(view),
            &ViewCeiling::from_definition(definition),
        )
    }

    /// The enumeration of `facts` under `definition` over the selection of
    /// `scope` through `ceiling`, as the source adapter would make it.
    pub fn enumerate(
        &self,
        definition: &AdmittedDefinition,
        facts: &ProjectionFacts,
        scope: &EffectiveSourceScope,
        ceiling: &ViewCeiling,
    ) -> ScopedEnumeration {
        let selection = ScopedSelection::for_holder(scope, ceiling);
        ok(ScopedEnumeration::new(
            &selection,
            self.resolved(definition, &ok(selection.filter_facts(facts))),
        ))
    }

    /// The unscoped trusted resolution of `facts`; never restricted directly.
    pub fn resolved(
        &self,
        definition: &AdmittedDefinition,
        facts: &ProjectionFacts,
    ) -> AdmittedPartialView {
        ok(resolve(definition, &self.repository(), facts))
    }

    /// The effective scope the fake verified port admits for a view's holder.
    pub fn scope(&self, view: &str) -> EffectiveSourceScope {
        let holder = &self.view(view).holder;
        self.scope_with(holder.grants.clone(), &holder.audience, &self.revision)
    }

    pub fn scope_with(
        &self,
        grants: Vec<SourceGrant>,
        audience: &str,
        revision: &ExternalRef,
    ) -> EffectiveSourceScope {
        let holder = self
            .views
            .iter()
            .find(|view| view.holder.audience == audience)
            .map(|view| &view.holder);
        let mut facts = EffectiveScopeFacts {
            audience: audience.into(),
            repository: self.repository().identity().clone(),
            snapshot: SnapshotRule::Exact {
                revision: revision.clone(),
            },
            revision: revision.clone(),
            view_binding: corpus_view_binding(),
            // Each corpus grant stands for one individually admitted request.
            admitted: grants.clone(),
            capabilities: Vec::new(),
            not_after_s: holder.map_or(self.now_s + 1_000, |holder| holder.not_after_s),
            depth: holder.map_or(0, |holder| holder.depth),
            generation: GENERATION,
            revocation_generation: REVOCATION_GENERATION,
            provenance: AuthorityProvenance::VerifiedUcan,
        };
        facts.capabilities = SessionCapability::under(&facts.context(), &grants);
        ok(EffectiveSourceScope::new(facts))
    }

    pub fn observation(&self) -> CairnObservation {
        ok(CairnObservation::new(
            self.repository().identity().clone(),
            self.revision.clone(),
            self.cairn.clone(),
        ))
    }

    pub fn current(&self) -> SourceCurrentness {
        SourceCurrentness {
            generation: GENERATION,
            revocation_generation: REVOCATION_GENERATION,
            snapshot: super::authority::SnapshotObservation {
                revision: self.revision.clone(),
                ref_name: None,
                previous: None,
                fast_forward: false,
            },
            attempt: crate::implementation::authority::AttemptCurrentness::Current,
        }
    }
}

pub(super) fn key() -> HandleKey {
    HandleKey::from_bytes([7; 32])
}

pub(super) fn other_key() -> HandleKey {
    HandleKey::from_bytes([9; 32])
}

pub(super) fn verified(subject: &str) -> AuthorityProfile {
    AuthorityProfile::VerifiedUcan(ok(AuthorityCredential::new(
        subject,
        "fake.compact.token",
        [1; 32],
        [2; 64],
    )))
}

/// What the fake verified port holds for one audience.
#[derive(Clone, Debug)]
pub(super) struct FakeGrant {
    pub repository: ContentId,
    pub snapshot: SnapshotRule,
    pub grants: Vec<SourceGrant>,
    pub not_after_s: u64,
    pub depth: u8,
    /// The authority generation the grant was issued under.
    pub generation: u64,
}

/// A fake verified port: a verified subject maps to a recorded grant, and
/// every check after the principal is the shared [`check_request`]. Like the
/// Basalt source session before its fix, it can also report leaf grants in
/// the session's capability set that the delegation chain rule never
/// checked; a session's scope still holds only the admitted request.
#[derive(Debug, Default)]
pub(super) struct FakeVerifiedSourceAuthority {
    holders: BTreeMap<String, FakeGrant>,
    unchecked: BTreeMap<String, Vec<SourceGrant>>,
    raw: BTreeMap<String, Vec<SessionCapability>>,
    revoked: BTreeSet<String>,
    sessions: BTreeMap<u64, (String, EffectiveSourceScope)>,
    next_session: u64,
    decisions: Vec<SourceAuthorityDecision>,
}

impl FakeVerifiedSourceAuthority {
    pub fn grant(&mut self, audience: &str, grant: FakeGrant) {
        self.holders.insert(audience.into(), grant);
    }

    /// Report a leaf grant in the holder's capability set that no chain
    /// rule checked.
    pub fn report_unchecked(&mut self, audience: &str, grant: SourceGrant) {
        self.unchecked
            .entry(audience.into())
            .or_default()
            .push(grant);
    }

    /// Report a raw capability with the exact context it was observed under.
    pub fn report_raw(&mut self, audience: &str, capability: SessionCapability) {
        self.raw
            .entry(audience.into())
            .or_default()
            .push(capability);
    }

    /// Issue an admitted share to its recipient.
    pub fn issue(&mut self, plan: &SharePlan) {
        self.grant(
            plan.recipient(),
            FakeGrant {
                repository: plan.repository().clone(),
                snapshot: plan.snapshot().clone(),
                grants: plan.grants().to_vec(),
                not_after_s: plan.not_after_s(),
                depth: plan.depth(),
                generation: plan.generation(),
            },
        );
    }

    pub fn revoke(&mut self, audience: &str) {
        self.revoked.insert(audience.into());
    }

    fn deny(
        &mut self,
        request: &SourceAuthorityRequest,
        reason: SourceDenialReason,
    ) -> SourceAuthorityDecision {
        let decision = SourceAuthorityDecision::deny(request, reason);
        self.decisions.push(decision.clone());
        decision
    }
}

impl SourceAuthority for FakeVerifiedSourceAuthority {
    fn admit(
        &mut self,
        request: &SourceAuthorityRequest,
        profile: &AuthorityProfile,
        current: &SourceCurrentness,
        now_unix_s: u64,
    ) -> Result<SourceAuthorityDecision, SourceAuthorityDecision> {
        let subject = match profile {
            AuthorityProfile::Missing => {
                return Err(self.deny(request, SourceDenialReason::MissingEvidence));
            }
            AuthorityProfile::LocalOperator { .. } => {
                return Err(self.deny(request, SourceDenialReason::VerifierUnavailable));
            }
            AuthorityProfile::VerifiedUcan(credential) => credential.subject(),
        };
        let Some(grant) = self.holders.get(subject).cloned() else {
            return Err(self.deny(request, SourceDenialReason::CredentialDenied));
        };
        if self.revoked.contains(subject) {
            return Err(self.deny(request, SourceDenialReason::Revoked));
        }
        let context = |generation: u64| CapabilityContext {
            audience: subject.into(),
            repository: grant.repository.clone(),
            snapshot: grant.snapshot.clone(),
            view_binding: request.view_binding.clone(),
            generation,
            revocation_generation: current.revocation_generation,
        };
        // The holder's grants carry the generation they were issued under;
        // other raw capabilities carry whatever context they were seen in.
        let mut capabilities = SessionCapability::under(&context(grant.generation), &grant.grants);
        capabilities.extend(self.raw.get(subject).cloned().unwrap_or_default());
        let facts = |admitted: Vec<SourceGrant>, capabilities: Vec<SessionCapability>| {
            ok(EffectiveSourceScope::new(EffectiveScopeFacts {
                audience: subject.into(),
                repository: grant.repository.clone(),
                snapshot: grant.snapshot.clone(),
                revision: current.snapshot.revision.clone(),
                view_binding: request.view_binding.clone(),
                admitted,
                capabilities,
                not_after_s: grant.not_after_s,
                depth: grant.depth,
                generation: current.generation,
                revocation_generation: current.revocation_generation,
                provenance: AuthorityProvenance::VerifiedUcan,
            }))
        };
        // The verifier checks the request against the checked chain only.
        let verified_chain = facts(grant.grants.clone(), capabilities.clone());
        if let Err(reason) = check_request(&verified_chain, request, current, now_unix_s) {
            return Err(self.deny(request, reason));
        }
        let unchecked = self.unchecked.get(subject).cloned().unwrap_or_default();
        capabilities.extend(SessionCapability::under(
            &context(current.generation),
            &unchecked,
        ));
        let scope = facts(
            request.requested_grant().into_iter().collect(),
            capabilities,
        );
        self.next_session += 1;
        let session = AuthoritySessionId(self.next_session);
        let decision =
            SourceAuthorityDecision::allowed(request, session, &scope, "fake-verified".into());
        self.sessions
            .insert(self.next_session, (request.digest_hex(), scope));
        self.decisions.push(decision.clone());
        Ok(decision)
    }

    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &SourceAuthorityRequest,
        current: &SourceCurrentness,
        now_unix_s: u64,
    ) -> Result<(), SourceAuthorityDecision> {
        let Some((identity, scope)) = self.sessions.get(&session.0).cloned() else {
            return Err(self.deny(request, SourceDenialReason::SessionNotFound));
        };
        let outcome = if identity != request.digest_hex() {
            Err(SourceDenialReason::BindingMismatch)
        } else if self.revoked.contains(scope.audience()) {
            Err(SourceDenialReason::Revoked)
        } else {
            check_request(&scope, request, current, now_unix_s)
        };
        outcome.map_err(|reason| {
            self.sessions.remove(&session.0);
            self.deny(request, reason)
        })
    }

    fn close(&mut self, session: AuthoritySessionId) {
        self.sessions.remove(&session.0);
    }

    fn effective_scope(&self, session: AuthoritySessionId) -> Option<&EffectiveSourceScope> {
        self.sessions.get(&session.0).map(|(_, scope)| scope)
    }

    fn decisions(&self) -> &[SourceAuthorityDecision] {
        &self.decisions
    }
}
