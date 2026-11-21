//! The first launch's files are prepared before the owner-grant gate, so a
//! failed preparation never strands committed owner reservations.

use super::{WorkerTransportProfile, prepare_launch_files};
use kiln_core::fair_admission::{
    AdmissionPhase, AdmissionPolicy, AdmissionReason, Amounts, BudgetAccount, ChargeKind, ClaimRef,
    EligibilityInput, OwnerReply, OwnerReservation, QueueItem, ReservationState, ServingBounds,
    UsageObservation,
};
use kiln_core::implementation::{
    AcceptanceObligation, Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationAttempt,
    ImplementationWork, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
};
use kiln_core::operations::{FairnessStrategy, TenantUsage, UsageValue};
use kiln_core::types::{Alias, ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::Implementer;
use kiln_implementer::admission::{AdmissionCommand, AdmissionResult};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const PROVIDER: &str = "scripted-local-omp";
const LEASE_MS: u64 = 1_200_000;
const WINDOW_MS: u64 = 1_800_000;

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn alias(value: &str) -> Alias {
    Alias::new(value).unwrap_or_else(|_| unreachable!())
}

fn provider() -> ProviderProfileId {
    ProviderProfileId::new(PROVIDER).unwrap_or_else(|_| unreachable!())
}

fn amounts() -> Amounts {
    BTreeMap::from([(alias("worker_requests"), 1)])
}

fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from(["onix/kiln".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    }
}

fn work() -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "operator".to_string(),
        repository: "onix/kiln".to_string(),
        project: "kiln".to_string(),
        base: Some(
            ExternalRef::new("git", "commit", &"a".repeat(40)).unwrap_or_else(|_| unreachable!()),
        ),
        title: "Launch preparation".to_string(),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: content('9'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "onix/kiln#45")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "onix/kiln#45".to_string(),
            version: content('1'),
        }],
    }
}

fn apply(owner: &mut Implementer, command: AdmissionCommand, now: u64) -> AdmissionResult {
    owner
        .admission_apply(command, now)
        .unwrap_or_else(|error| unreachable!("{error}"))
}

/// Admit one work, queue it under a local owner, claim it and commit that
/// owner's grant, as the composed qualification fixture does.
fn committed_attempt(owner: &mut Implementer, now: u64) -> ImplementationAttempt {
    let work = work();
    owner
        .admit(&work, None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let identity = work.identity().unwrap_or_else(|_| unreachable!());
    let deadline = now + WINDOW_MS;
    let tenant = alias("scripted_local");
    let policy = AdmissionPolicy {
        version: 1,
        accounting_basis: identity.clone(),
        max_items: 1,
        max_bytes: 1_048_576,
        max_wait_ms: WINDOW_MS,
        max_observation_age_ms: WINDOW_MS,
        max_retry_after_ms: 30_000,
        max_children: 1,
        max_active: 1,
        strategy: FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(tenant.clone(), 1)]),
        group_limits: Vec::new(),
        serving: None,
    };
    apply(
        owner,
        AdmissionCommand::Configure {
            policy: policy.clone(),
        },
        now,
    );
    apply(
        owner,
        AdmissionCommand::Root {
            account: BudgetAccount {
                identity: identity.clone(),
                parent: None,
                parent_attempt: None,
                campaign: tenant.clone(),
                tenant: tenant.clone(),
                deadline_ms: deadline,
                available: amounts(),
                consumed: Amounts::new(),
                children: 0,
            },
        },
        now,
    );
    apply(
        owner,
        AdmissionCommand::Observe {
            observation: UsageObservation {
                usage: TenantUsage {
                    tenant: tenant.clone(),
                    share: 1,
                    provider: provider(),
                    interval_start: now.saturating_sub(180_000),
                    interval_end: now,
                    observed_at: now,
                    resources: BTreeMap::from([(
                        alias("worker_requests"),
                        UsageValue {
                            amount: 0,
                            capacity: Some(1),
                        },
                    )]),
                },
                generation: 1,
                accounting_basis: policy.accounting_basis,
                covered: BTreeMap::new(),
                complete: true,
            },
        },
        now,
    );
    apply(
        owner,
        AdmissionCommand::Enqueue {
            item: QueueItem {
                work_version: identity.clone(),
                account: identity.clone(),
                tenant: tenant.clone(),
                campaign: tenant,
                repository: alias("onix-kiln"),
                priority: work.priority,
                bytes: 1,
                enqueued_at_ms: now,
                deadline_ms: deadline,
                eligible_at_ms: now,
                claim: None,
                phase: AdmissionPhase::PreClaim,
                reason: AdmissionReason::Waiting,
                order: 0,
                required_owners: BTreeMap::from([(provider(), ChargeKind::WorkerRequest)]),
                shared_allocations: Vec::new(),
                budget: amounts(),
            },
        },
        now,
    );
    let eligible = EligibilityInput {
        work_version: identity,
        reason: AdmissionReason::Eligible,
        provider: provider(),
        generation: 1,
        observed_at_ms: now,
        valid_until_ms: deadline,
    };
    let attempt = match apply(
        owner,
        AdmissionCommand::Claim {
            inputs: vec![eligible.clone()],
            lease_ms: LEASE_MS,
        },
        now,
    ) {
        AdmissionResult::Claimed {
            attempt: Some(attempt),
        } => *attempt,
        other => unreachable!("claim did not produce an attempt: {other:?}"),
    };
    let ledger = owner
        .admission_snapshot()
        .unwrap_or_else(|error| unreachable!("{error}"));
    let item = ledger
        .items
        .get(&attempt.work_identity)
        .unwrap_or_else(|| unreachable!());
    let claim = ClaimRef {
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
    };
    // A finite local owner-aggregate bound; no serving owner is contacted.
    let bounds = ServingBounds {
        scope: content('e'),
        max_ongoing: 1,
        max_queued: 0,
    };
    let reservation = OwnerReservation {
        version: 1,
        operation: attempt.identity.clone(),
        provider: provider(),
        generation: 1,
        work_version: attempt.work_identity.clone(),
        claim,
        account: item.account.clone(),
        kind: ChargeKind::WorkerRequest,
        subject: content('c'),
        accounting_basis: ledger
            .policy
            .as_ref()
            .unwrap_or_else(|| unreachable!())
            .accounting_basis
            .clone(),
        serving_bounds: Some(bounds.clone()),
        amounts: amounts(),
        expires_at_ms: item.deadline_ms.min(attempt.lease_until_ms),
        state: ReservationState::Tentative,
        receipt: None,
    };
    apply(
        owner,
        AdmissionCommand::Request {
            reservation: reservation.clone(),
            inputs: vec![eligible],
        },
        now,
    );
    apply(
        owner,
        AdmissionCommand::Reply {
            reply: OwnerReply {
                version: 1,
                operation: reservation.operation.clone(),
                provider: provider(),
                generation: 1,
                receipt: reservation.operation,
                state: ReservationState::Committed,
                retry_at_ms: None,
                serving_bounds: Some(bounds),
            },
        },
        now,
    );
    attempt
}

/// Every owner reservation of the work and its queue phase.
fn grants(
    owner: &mut Implementer,
    attempt: &ImplementationAttempt,
) -> (Vec<ReservationState>, AdmissionPhase) {
    let ledger = owner
        .admission_snapshot()
        .unwrap_or_else(|error| unreachable!("{error}"));
    let states = ledger
        .reservations
        .values()
        .filter(|reservation| reservation.work_version == attempt.work_identity)
        .map(|reservation| reservation.state)
        .collect();
    let phase = ledger
        .items
        .get(&attempt.work_identity)
        .unwrap_or_else(|| unreachable!())
        .phase;
    (states, phase)
}

fn profile(root: &Path, state: &Path) -> WorkerTransportProfile {
    WorkerTransportProfile {
        schema: "kiln.lattice-implementation-cli.v2".into(),
        lattice_program: "/nix/store/00000000000000000000000000000000-lattice/bin/lattice".into(),
        executable_blake3: content('b'),
        state_directory: state.to_path_buf(),
        output_directory: root.join("transport"),
    }
}

// r[verify kiln.implementation.lattice_cli.scenario.bound_root]
#[test]
fn failed_preparation_leaves_owner_grants_committed_and_a_retry_proceeds() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let mut owner = Implementer::open(&root.join("kiln.sqlite"), enrollment())
        .unwrap_or_else(|error| unreachable!("{error}"));
    let now = super::clock().unwrap_or_else(|_| unreachable!());
    let attempt = committed_attempt(&mut owner, now);
    let committed = grants(&mut owner, &attempt);
    assert_eq!(committed.0, vec![ReservationState::Committed]);
    let request_id = content('d');
    let prepare = |profile: &WorkerTransportProfile| {
        prepare_launch_files(
            profile,
            &attempt.identity,
            attempt.fence,
            &request_id,
            b"exact request bytes",
            true,
        )
    };

    // An unwritable state root: its parent is an ordinary file.
    let blocker = root.join("blocker");
    std::fs::write(&blocker, b"not a directory").unwrap_or_else(|_| unreachable!());
    let root_a = blocker.join("state");
    assert!(prepare(&profile(root, &root_a)).is_err());
    assert_eq!(grants(&mut owner, &attempt), committed);

    // Removing the cause lets the same first launch prepare its root and binding.
    std::fs::remove_file(&blocker).unwrap_or_else(|_| unreachable!());
    prepare(&profile(root, &root_a)).unwrap_or_else(|error| unreachable!("{error}"));
    assert!(root_a.is_dir());
    assert_eq!(grants(&mut owner, &attempt), committed);

    // A conflicting first-launch binding is refused, never replaced, and the
    // grants stay committed.
    let root_b = root.join("state-b");
    assert!(prepare(&profile(root, &root_b)).is_err());
    let bound = super::binding::load(&super::binding::TransportBinding::path(
        &root.join("transport"),
        &attempt.identity,
        attempt.fence,
    ))
    .unwrap_or_else(|error| unreachable!("{error}"))
    .unwrap_or_else(|| unreachable!());
    assert_eq!(bound.state_directory, root_a);
    assert_eq!(grants(&mut owner, &attempt), committed);

    // A crash after the binding was retained but before the gate: the retry
    // prepares idempotently and the owner-grant gate still admits it.
    prepare(&profile(root, &root_a)).unwrap_or_else(|error| unreachable!("{error}"));
    let fresh: Vec<EligibilityInput> = owner
        .admission_snapshot()
        .unwrap_or_else(|error| unreachable!("{error}"))
        .eligibility
        .into_values()
        .collect();
    owner
        .admission_before_transport(&attempt.identity, attempt.fence, &fresh, now)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let consumed = grants(&mut owner, &attempt);
    assert_eq!(consumed.0, vec![ReservationState::Unknown]);
    assert_eq!(consumed.1, AdmissionPhase::DispatchIntent);
}
