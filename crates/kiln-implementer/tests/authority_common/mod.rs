//! Shared verified-profile setup for the authority integration tests.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kiln_adapter_basalt::test_support::{self, Fixture};
use kiln_core::fair_admission::*;
use kiln_core::implementation::authority::{
    AttemptCurrentness, AuthorityDecision, AuthorityProfile, AuthorityRequest, AuthoritySelection,
    AuthoritySessionId, HolderInvocationSigner, ProtectedAuthority,
};
use kiln_core::implementation::grant::GrantRole;
use kiln_core::implementation::*;
use kiln_core::operations::{FairnessStrategy, TenantUsage, UsageValue};
use kiln_core::types::{Alias, ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::Implementer;
use kiln_implementer::admission::{AdmissionCommand as Cmd, AdmissionResult};
use kiln_implementer::authority_config::ImplementationAuthorityConfig;
use kiln_implementer::authority_state::{
    RecordingAuthority, SqliteAuthorityState, SqliteReplayLedger, SqliteRevocationView,
    authority_state_path,
};

/// Shell time inside the fixture grant window, in milliseconds.
pub const T: u64 = test_support::VERIFY_AT_S * 1_000;
/// Revocation view maximum age.
pub const MAX_AGE_MS: u64 = 600_000;

pub fn id(s: &str) -> ContentId {
    ContentId::from_blake3_hex(blake3::hash(s.as_bytes()).to_hex().to_string())
        .unwrap_or_else(|_| unreachable!())
}
fn a(s: &str) -> Alias {
    Alias::new(s).unwrap_or_else(|_| unreachable!())
}
pub fn p() -> ProviderProfileId {
    ProviderProfileId::new("serving-v1").unwrap_or_else(|_| unreachable!())
}
fn amounts(n: u64) -> Amounts {
    BTreeMap::from([(a("requests"), n)])
}
pub fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from(["onix/kiln".into()]),
        actors: BTreeSet::from(["operator".into()]),
    }
}
pub fn work(n: &str) -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "operator".into(),
        repository: "onix/kiln".into(),
        project: "kiln".into(),
        base: None,
        title: n.into(),
        requested_outcome: "implement admitted slice".into(),
        acceptance: vec![AcceptanceObligation {
            id: "a".into(),
            statement: "tested".into(),
        }],
        dependencies: vec![],
        permitted_paths: vec!["crates".into()],
        permitted_effects: BTreeSet::from(["write_source".into()]),
        verification_profile: id("verification"),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", n).unwrap_or_else(|_| unreachable!()),
            native_id: n.into(),
            version: id(n),
        }],
    }
}
fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        version: 1,
        accounting_basis: id("basis"),
        max_items: 32,
        max_bytes: 100_000,
        max_wait_ms: 1_000_000,
        max_observation_age_ms: 100_000,
        max_retry_after_ms: 50,
        max_children: 4,
        max_active: 2,
        strategy: FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(a("a"), 1)]),
        group_limits: vec![],
        serving: None,
    }
}
pub fn offer(w: &ImplementationWork) -> EligibilityInput {
    EligibilityInput {
        work_version: w.identity().unwrap_or_else(|_| unreachable!()),
        reason: AdmissionReason::Eligible,
        provider: p(),
        generation: 1,
        observed_at_ms: T,
        valid_until_ms: T + 500_000,
    }
}
fn item(w: &ImplementationWork) -> QueueItem {
    QueueItem {
        work_version: w.identity().unwrap_or_else(|_| unreachable!()),
        account: id("a"),
        tenant: a("tenant"),
        campaign: a("a"),
        repository: a("repo"),
        priority: 255,
        bytes: 1,
        enqueued_at_ms: T,
        deadline_ms: T + 500_000,
        eligible_at_ms: T,
        claim: None,
        phase: AdmissionPhase::PreClaim,
        reason: AdmissionReason::Waiting,
        order: 999,
        required_owners: BTreeMap::from([(p(), ChargeKind::WorkerRequest)]),
        shared_allocations: vec![],
        budget: amounts(30),
    }
}

/// Configure admission, enqueue `w`. Authority must already be selected.
pub fn queue(i: &mut Implementer, w: &ImplementationWork) {
    i.admission_apply(Cmd::Configure { policy: policy() }, T)
        .unwrap_or_else(|error| unreachable!("{error}"));
    i.admission_apply(
        Cmd::Root {
            account: BudgetAccount {
                identity: id("a"),
                parent: None,
                parent_attempt: None,
                campaign: a("a"),
                tenant: a("tenant"),
                deadline_ms: T + 1_000_000,
                available: amounts(100),
                consumed: Amounts::new(),
                children: 0,
            },
        },
        T,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let observation = UsageObservation {
        usage: TenantUsage {
            tenant: a("a"),
            share: 1,
            provider: p(),
            interval_start: 0,
            interval_end: T,
            observed_at: T,
            resources: BTreeMap::from([(
                a("requests"),
                UsageValue {
                    amount: 0,
                    capacity: Some(100),
                },
            )]),
        },
        generation: 1,
        accounting_basis: id("basis"),
        covered: BTreeMap::new(),
        complete: true,
    };
    i.admission_apply(Cmd::Observe { observation }, T)
        .unwrap_or_else(|error| unreachable!("{error}"));
    i.admit_with_supersession(w, None, SupersessionRule::AbortActive, T)
        .unwrap_or_else(|error| unreachable!("{error}"));
    i.admission_apply(Cmd::Enqueue { item: item(w) }, T)
        .unwrap_or_else(|error| unreachable!("{error}"));
}

pub fn claim(i: &mut Implementer, w: &ImplementationWork) -> Result<ImplementationAttempt, String> {
    match i.admission_apply(
        Cmd::Claim {
            inputs: vec![offer(w)],
            lease_ms: 400_000,
        },
        T,
    )? {
        AdmissionResult::Claimed { attempt: Some(a) } => Ok(*a),
        other => Err(format!("{other:?}")),
    }
}

pub fn reservation(w: &ImplementationWork, attempt: &ImplementationAttempt) -> OwnerReservation {
    OwnerReservation {
        version: 1,
        operation: id("op"),
        provider: p(),
        generation: 1,
        work_version: w.identity().unwrap_or_else(|_| unreachable!()),
        claim: ClaimRef {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
        },
        account: id("a"),
        kind: ChargeKind::WorkerRequest,
        subject: id("op"),
        accounting_basis: id("basis"),
        amounts: amounts(10),
        expires_at_ms: T + 400_000,
        state: ReservationState::Tentative,
        receipt: None,
        serving_bounds: Some(ServingBounds {
            scope: id("aggregate-service"),
            max_ongoing: 2,
            max_queued: 8,
        }),
    }
}

pub fn confirm(i: &mut Implementer, r: &OwnerReservation) {
    i.admission_apply(
        Cmd::Reply {
            reply: OwnerReply {
                version: 1,
                operation: r.operation.clone(),
                provider: p(),
                generation: 1,
                receipt: id("grant"),
                state: ReservationState::Committed,
                retry_at_ms: None,
                serving_bounds: r.serving_bounds.clone(),
            },
        },
        T + 1,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Write `revocations.json` under `dir`: its producer observed this state at
/// `observed_at_ms`. Returns the feed path.
pub fn write_feed(dir: &Path, generation: u64, observed_at_ms: u64, revoked: &[String]) -> PathBuf {
    let feed = dir.join("revocations.json");
    std::fs::write(
        &feed,
        serde_json::to_vec(&serde_json::json!({
            "schema": "kiln.implementation-authority-revocations.v1",
            "generation": generation,
            "observed_at_ms": observed_at_ms,
            "revoked": revoked,
        }))
        .unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    feed
}

/// Write the fixture's seeds, grants, a revocation feed observed at `T` and
/// the exported config under `dir`, returning the config path.
pub fn write_config(dir: &Path, fixture: &Fixture, revoked: &[[u8; 32]]) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let proofs = dir.join("proofs");
    std::fs::create_dir_all(&proofs).unwrap_or_else(|error| unreachable!("{error}"));
    let mut tokens: Vec<&str> = fixture
        .grants
        .iter()
        .map(|(_, token)| token.as_str())
        .collect();
    tokens.extend(fixture.proofs.iter().map(String::as_str));
    for (index, token) in tokens.iter().enumerate() {
        std::fs::write(proofs.join(format!("{index}.ucan")), token)
            .unwrap_or_else(|error| unreachable!("{error}"));
    }
    let mut holders = BTreeMap::new();
    for (name, role) in [
        ("coordinator", GrantRole::Coding),
        ("verifier", GrantRole::Verification),
        ("publisher", GrantRole::Publication),
    ] {
        let path = dir.join(format!("{name}.seed"));
        std::fs::write(&path, test_support::seed(test_support::role_seed(role)))
            .unwrap_or_else(|error| unreachable!("{error}"));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .unwrap_or_else(|error| unreachable!("{error}"));
        holders.insert(name, serde_json::json!({ "seed_path": path }));
    }
    let revoked: Vec<String> = revoked.iter().map(|reference| hex(reference)).collect();
    let feed = write_feed(dir, 1, T, &revoked);
    let keys: Vec<_> = [
        test_support::ROOT_SEED,
        test_support::DELEGATE_SEED,
        test_support::COORDINATOR_SEED,
        test_support::VERIFIER_SEED,
        test_support::PUBLISHER_SEED,
    ]
    .into_iter()
    .map(|seed| {
        serde_json::json!({
            "did": test_support::did(seed),
            "ed25519": hex(&test_support::public_key(seed)),
        })
    })
    .collect();
    let config = serde_json::json!({
        "schema": "kiln.implementation-authority-config.v1",
        "profile": { "verified_ucan": {
            "audience": test_support::AUDIENCE,
            "generation": test_support::GENERATION,
            "policy": "packaged",
            "trusted_roots": fixture.roots,
            "keys": keys,
            "proofs_dir": proofs,
            "revocation": { "path": feed, "max_age_ms": MAX_AGE_MS },
            "replay": { "retention_ms": 60_000 },
            "holders": holders,
            "projection_enabled": false,
        } },
    });
    let path = dir.join("authority.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&config).unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    path
}

/// Open a ledger and select the verified profile through the composition
/// helper as a coordinator entry point: only the coordinator seed is loaded.
pub fn open_verified(db: &Path, config: &Path) -> Implementer {
    let mut i = Implementer::open(db, enrollment()).unwrap_or_else(|error| unreachable!("{error}"));
    i.set_clock_ms(T);
    i.select_authority(
        &ImplementationAuthorityConfig::load(config)
            .unwrap_or_else(|error| unreachable!("{error}")),
        &BTreeSet::from([GrantRole::Coding]),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    i
}

/// Callback run inside one protected-authority call.
pub type Hook = Box<dyn FnMut(&AuthorityRequest)>;

/// Verified port over the durable SQLite state with hooks around admission
/// and revalidation, for interleaving tests.
pub struct Hooked {
    pub inner: Box<dyn ProtectedAuthority>,
    pub on_admit: Option<Hook>,
    pub on_revalidate: Option<Hook>,
}

impl ProtectedAuthority for Hooked {
    fn admit(
        &mut self,
        request: &AuthorityRequest,
        profile: &AuthorityProfile,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> Result<AuthorityDecision, AuthorityDecision> {
        if let Some(hook) = self.on_admit.as_mut() {
            hook(request);
        }
        self.inner.admit(request, profile, currentness, now_unix_s)
    }

    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &AuthorityRequest,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> Result<(), AuthorityDecision> {
        let verdict = self
            .inner
            .revalidate(session, request, currentness, now_unix_s);
        if let Some(hook) = self.on_revalidate.as_mut() {
            hook(request);
        }
        verdict
    }

    fn close(&mut self, session: AuthoritySessionId) {
        self.inner.close(session);
    }

    fn decisions(&self) -> &[AuthorityDecision] {
        self.inner.decisions()
    }
}

/// Verified port over the ledger's durable authority file.
pub fn durable_verified(db: &Path, fixture: &Fixture) -> Box<dyn ProtectedAuthority> {
    let state_path = authority_state_path(db).unwrap_or_else(|error| unreachable!("{error}"));
    let state =
        SqliteAuthorityState::open(&state_path).unwrap_or_else(|error| unreachable!("{error}"));
    state
        .observe_revocation_view(1, T)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let mut services = fixture.services();
    services.replay = Box::new(
        SqliteReplayLedger::open(&state_path).unwrap_or_else(|error| unreachable!("{error}")),
    );
    services.revocations = Box::new(
        SqliteRevocationView::open(&state_path, MAX_AGE_MS)
            .unwrap_or_else(|error| unreachable!("{error}")),
    );
    let authority = kiln_adapter_basalt::BasaltAuthority::verified(fixture.config(), services)
        .unwrap_or_else(|error| unreachable!("{error}"));
    Box::new(RecordingAuthority::new(Box::new(authority), state))
}

/// Install the verified port with hooks and a signer.
pub fn install(
    i: &mut Implementer,
    inner: Box<dyn ProtectedAuthority>,
    signer: Box<dyn HolderInvocationSigner>,
    on_admit: Option<Hook>,
    on_revalidate: Option<Hook>,
) {
    i.set_authority(
        Box::new(Hooked {
            inner,
            on_admit,
            on_revalidate,
        }),
        AuthoritySelection::Verified(signer),
    );
}
