//! The first launch's files are prepared before the owner-grant gate, so a
//! failed preparation never strands committed owner reservations.

use super::{WorkerTransportProfile, prepare_launch_files};
use kiln_core::fair_admission::{
    AdmissionPhase, AdmissionPolicy, AdmissionReason, Amounts, BudgetAccount, ChargeKind, ClaimRef,
    EligibilityInput, OwnerReply, OwnerReservation, QueueItem, ReservationState, ServingBounds,
    UsageObservation,
};
use kiln_core::implementation::{
    AcceptanceObligation, DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, Enrollment,
    IMPLEMENTATION_WORK_SCHEMA, ImplementationAttempt, ImplementationDispatchPlan,
    ImplementationWork, SourceRef, TerminalPolicy, TransportStart, WorkBudget, WorkSourceKind,
    WorkspaceIsolation,
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

fn profile(root: &Path, output: &str, state: &Path) -> WorkerTransportProfile {
    WorkerTransportProfile {
        schema: "kiln.lattice-implementation-cli.v2".into(),
        lattice_program: "/nix/store/00000000000000000000000000000000-lattice/bin/lattice".into(),
        executable_blake3: content('b'),
        state_directory: state.to_path_buf(),
        output_directory: root.join(output),
    }
}

/// One claimed attempt with committed owner grants, a reserved plan and a
/// sealed request, ready for the owner-grant gate and transport intent.
fn ready(root: &Path) -> (Implementer, ImplementationAttempt, ContentId, u64) {
    let mut owner = Implementer::open_with_local_operator(
        &root.join("kiln.sqlite"),
        enrollment(),
        "test-operator",
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let now = super::clock().unwrap_or_else(|_| unreachable!());
    let attempt = committed_attempt(&mut owner, now);
    let work = work();
    let plan = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: attempt.work_identity.clone(),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        workspace: DispatchWorkspace {
            identity: content('e'),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: None,
        },
        provider_profile: provider(),
        permitted_paths: work.permitted_paths.clone(),
        permitted_effects: work.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 1,
        token_budget: 1000,
        invocation_budget: 1,
        expected_output: content('f'),
        execution: None,
    };
    let digest = owner
        .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, now, &plan)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .plan;
    owner
        .bind_campaign_request(&work, &attempt.identity, attempt.fence, now, b"request")
        .unwrap_or_else(|error| unreachable!("{error}"));
    (owner, attempt, digest, now)
}

fn fresh_observations(owner: &mut Implementer) -> Vec<EligibilityInput> {
    owner
        .admission_snapshot()
        .unwrap_or_else(|error| unreachable!("{error}"))
        .eligibility
        .into_values()
        .collect()
}

fn entries(path: &Path) -> usize {
    std::fs::read_dir(path).map_or(0, Iterator::count)
}

// r[verify kiln.implementation.lattice_cli.scenario.bound_root]
#[test]
fn failed_preparation_leaves_owner_grants_committed_and_a_retry_proceeds() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let mut owner = Implementer::open_with_local_operator(
        &root.join("kiln.sqlite"),
        enrollment(),
        "test-operator",
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let now = super::clock().unwrap_or_else(|_| unreachable!());
    let attempt = committed_attempt(&mut owner, now);
    let committed = grants(&mut owner, &attempt);
    assert_eq!(committed.0, vec![ReservationState::Committed]);
    let request_id = content('d');
    let prepare = |profile: &WorkerTransportProfile| {
        prepare_launch_files(profile, &request_id, b"exact request bytes", true)
    };

    // An unwritable state root: its parent is an ordinary file.
    let blocker = root.join("blocker");
    std::fs::write(&blocker, b"not a directory").unwrap_or_else(|_| unreachable!());
    let root_a = blocker.join("state");
    assert!(prepare(&profile(root, "transport", &root_a)).is_err());
    assert_eq!(grants(&mut owner, &attempt), committed);

    // Removing the cause lets the same first launch prepare its root.
    std::fs::remove_file(&blocker).unwrap_or_else(|_| unreachable!());
    prepare(&profile(root, "transport", &root_a)).unwrap_or_else(|error| unreachable!("{error}"));
    // Other bytes under the same frozen request identity are refused, not replaced.
    assert!(
        prepare_launch_files(
            &profile(root, "transport", &root_a),
            &request_id,
            b"substituted bytes",
            true
        )
        .is_err()
    );
    assert!(root_a.is_dir());
    assert_eq!(grants(&mut owner, &attempt), committed);

    // A crash after preparation but before the gate: the retry prepares
    // idempotently and the owner-grant gate still admits it.
    prepare(&profile(root, "transport", &root_a)).unwrap_or_else(|error| unreachable!("{error}"));
    let fresh = fresh_observations(&mut owner);
    owner
        .admission_before_transport(&attempt.identity, attempt.fence, &fresh, now)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let consumed = grants(&mut owner, &attempt);
    assert_eq!(consumed.0, vec![ReservationState::Unknown]);
    assert_eq!(consumed.1, AdmissionPhase::DispatchIntent);
}

// r[verify kiln.implementation.lattice_cli.scenario.bound_root]
#[test]
fn a_gate_refused_profile_never_directs_recovery() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let (mut owner, attempt, digest, now) = ready(root);
    let (root_a, root_b) = (root.join("state-a"), root.join("state-b"));
    let (profile_a, profile_b) = (
        profile(root, "output-a", &root_a),
        profile(root, "output-b", &root_b),
    );
    // (1) Profile A without its fresh owner observation: preparation creates
    // root A, the gate refuses, and nothing records a launch.
    prepare_launch_files(&profile_a, &content('d'), b"request", true)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        owner
            .admission_before_transport(&attempt.identity, attempt.fence, &[], now)
            .is_err()
    );
    assert_eq!(
        owner
            .dispatch_launch(&attempt.identity)
            .unwrap_or_else(|error| unreachable!("{error}")),
        None
    );
    // (2) Profile B with the observation: the gate passes and the transport
    // intent records launch B.
    prepare_launch_files(&profile_b, &content('d'), b"request", true)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let fresh = fresh_observations(&mut owner);
    owner
        .admission_before_transport(&attempt.identity, attempt.fence, &fresh, now)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let launch_b = super::first_launch(&profile_b).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        owner
            .begin_dispatch_transport(
                &attempt.identity,
                attempt.fence,
                now,
                &digest,
                Some(&launch_b)
            )
            .unwrap_or_else(|error| unreachable!("{error}")),
        TransportStart::Fresh
    );
    // (3) A retry under profile A recovers with B's recorded launch; A's
    // never-launched root cannot direct recovery.
    let launch_a = super::first_launch(&profile_a).unwrap_or_else(|error| unreachable!("{error}"));
    let TransportStart::Recover(recorded) = owner
        .begin_dispatch_transport(
            &attempt.identity,
            attempt.fence,
            now,
            &digest,
            Some(&launch_a),
        )
        .unwrap_or_else(|error| unreachable!("{error}"))
    else {
        unreachable!("an existing intent must recover");
    };
    let chosen = super::binding::recovery_launch(
        recorded.clone(),
        recorded
            .as_ref()
            .is_some_and(|launch| super::binding::is_directory(Path::new(&launch.state_directory))),
        true,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(chosen, launch_b);
    assert_eq!(entries(&root_a), 0);
}

// r[verify kiln.implementation.lattice_cli.scenario.bound_root]
#[test]
fn the_gate_loser_of_two_processes_leaves_no_launch() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let (mut owner, attempt, digest, now) = ready(root);
    let profile_x = profile(root, "output-x", &root.join("state-x"));
    let profile_y = profile(root, "output-y", &root.join("state-y"));
    // Both processes prepare their own output directory and root.
    for profile in [&profile_x, &profile_y] {
        prepare_launch_files(profile, &content('d'), b"request", true)
            .unwrap_or_else(|error| unreachable!("{error}"));
    }
    let fresh = fresh_observations(&mut owner);
    // X wins the owner-grant gate and records its launch with the intent.
    owner
        .admission_before_transport(&attempt.identity, attempt.fence, &fresh, now)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let launch_x = super::first_launch(&profile_x).unwrap_or_else(|error| unreachable!("{error}"));
    owner
        .begin_dispatch_transport(
            &attempt.identity,
            attempt.fence,
            now,
            &digest,
            Some(&launch_x),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    // Y loses the gate: its grants are no longer committed, so it never
    // reaches the transport intent and records nothing.
    assert!(
        owner
            .admission_before_transport(&attempt.identity, attempt.fence, &fresh, now)
            .is_err()
    );
    assert_eq!(
        owner
            .dispatch_launch(&attempt.identity)
            .unwrap_or_else(|error| unreachable!("{error}")),
        Some(launch_x.clone())
    );
    // Any later recovery, whichever profile it runs under, gets X's launch.
    let launch_y = super::first_launch(&profile_y).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        owner
            .begin_dispatch_transport(
                &attempt.identity,
                attempt.fence,
                now,
                &digest,
                Some(&launch_y)
            )
            .unwrap_or_else(|error| unreachable!("{error}")),
        TransportStart::Recover(Some(launch_x))
    );
    assert_eq!(entries(&root.join("state-y")), 0);
}

// r[verify kiln.admission.lattice_lineage]
#[test]
fn the_worker_route_freezes_the_derived_lineage_before_the_grant_gate() {
    use crate::lineage::{LineageRefusal, Route, dispatched_request, verify_frozen};
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path();
    let (mut owner, attempt, _, now) = ready(root);
    let admitted: &[u8] =
        b"{\"schema\":\"lattice.implementation-attempt-run.v3\",\"network\":false}\n";
    let request_id = ContentId::from_blake3_hex(blake3::hash(admitted).to_hex().to_string())
        .unwrap_or_else(|_| unreachable!());
    let claim = ClaimRef {
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
    };
    let send = |owner: &mut Implementer, route: Route| {
        let ledger = owner
            .admission_snapshot()
            .unwrap_or_else(|error| unreachable!("{error}"));
        dispatched_request(
            route,
            &ledger,
            &attempt.work_identity,
            &claim,
            now,
            admitted,
        )
        .map(|bytes| bytes.into_owned())
    };
    // The committed grant from the durable store is carried on the worker route.
    let dispatched =
        send(&mut owner, Route::Worker).unwrap_or_else(|error| unreachable!("{error}"));
    let lineage = verify_frozen(admitted, &dispatched)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!("no lineage"));
    assert_eq!(lineage.account, attempt.work_identity.as_str());
    assert_eq!((lineage.parent, lineage.depth), (None, 0));
    assert_eq!(
        lineage.remaining,
        BTreeMap::from([("worker_requests".to_owned(), 1)])
    );
    // The exchange route checks the same ledger and keeps its bytes.
    assert_eq!(send(&mut owner, Route::Exchange), Ok(admitted.to_vec()));
    // The frozen request Lattice reads is the dispatched one, under the
    // admitted request's identity.
    let transport = profile(root, "transport", &root.join("state"));
    let frozen = prepare_launch_files(&transport, &request_id, &dispatched, true)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        frozen,
        super::dispatched_request_path(&transport, &request_id)
    );
    assert_eq!(std::fs::read(&frozen).ok(), Some(dispatched.clone()));
    // The gate moves the grant to unknown: both routes now refuse to derive
    // again, and recovery reuses only the frozen request.
    let fresh = fresh_observations(&mut owner);
    owner
        .admission_before_transport(&attempt.identity, attempt.fence, &fresh, now)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let unsettled = Err(LineageRefusal::ReservationUnsettled {
        operation: attempt.identity.as_str().to_owned(),
    });
    assert_eq!(send(&mut owner, Route::Worker), unsettled);
    assert_eq!(send(&mut owner, Route::Exchange), unsettled);
    assert_eq!(
        verify_frozen(admitted, &dispatched).map(|lineage| lineage.is_some()),
        Ok(true)
    );
}
