//! Reviewed entry-point classification, `kiln.implementation-authority-coverage.v2`.
//!
//! Every `Implementer` public function, `kiln-implementer` CLI subcommand,
//! fleet API route and web campaign control route has at least one row. The
//! `authority_coverage` integration test of `kiln-implementer` scans those
//! sources and fails when an entry point has no row, a row names a missing
//! entry point, a `Protected` row does not reach its operation's authority
//! check, or a non-authority row reaches one.
//!
//! Classes, as applied here:
//! - `Protected(op)`: admits a fresh exact `op` invocation before its effect.
//! - `Reconcile(op)`: a fence-checked record of the progress or outcome of
//!   the attempt whose effect `op` admitted (worker run lifecycle under
//!   `Dispatch`, verifier lifecycle under `VerificationAdmit`, provider
//!   receipts under `PublicationPublish`). It starts no new external effect;
//!   where it adopts a provider outcome it presents a fresh invocation for
//!   the original effect identity.
//! - `ObservationOnly`: reads state; confers and consumes no authority. The
//!   `Implementer` readers that go through the shared ledger transaction
//!   (`work`, `attempts`, `attempt`, `plan_feedback`, `authorize_current`,
//!   `active_work`, `dispatch_plan`, `dispatch_requires_recovery`,
//!   `dispatch_reservation`, `swarm_snapshots`, `metrics` and the receipt
//!   readers) save the work and attempt documents back unchanged in an
//!   immediate transaction, bumping their row versions. Their closures only
//!   read (`&self` ledger methods), so the write re-serializes the loaded
//!   values: no attempt phase, lease, fence or claim changes, and authority
//!   decisions live in the separate authority state file they never open.
//! - `OperatorConfig`: composition, source intake, admission and fleet ledger
//!   maintenance, durable intents that cause no external effect, and CLI
//!   records or settlements that check no authority. The deployment
//!   authenticates these outside implementation authority; the effect an
//!   intent prepares is admitted at its own boundary.
//!
//! An entry point loads the holder seeds of the operations its `Protected`
//! and `Reconcile` rows name ([`super::entry_point_roles`]), so a row names
//! an operation only when the entry point checks it.

use super::{EntryClass, EntryPoint, ProtectedOperation as Op};

const LIB: &str = "crates/kiln-implementer/src/lib.rs";
const ADMISSION: &str = "crates/kiln-implementer/src/admission.rs";
const CONFIG: &str = "crates/kiln-implementer/src/authority_config.rs";
const CAMPAIGN: &str = "crates/kiln-implementer/src/campaign_consumer.rs";
const FLEET: &str = "crates/kiln-implementer/src/fleet.rs";
const HOST_OWNER: &str = "crates/kiln-implementer/src/host_owner.rs";
const NATIVE: &str = "crates/kiln-implementer/src/native_publication.rs";
const EFFECTS: &str = "crates/kiln-implementer/src/publication_effects.rs";
const STACK: &str = "crates/kiln-implementer/src/stack_publication.rs";
/// Path of the `kiln-implementer` CLI, whose subcommands are entry points.
pub const IMPLEMENTER_CLI: &str = "crates/kiln-adapter-github/src/bin/implementer.rs";
const CLI: &str = IMPLEMENTER_CLI;
/// Path of the fleet API, whose `METHOD /v1/...` routes are entry points.
pub const FLEET_API: &str = "crates/kiln-fleet/src/api.rs";
const CONTROLS: &str = "crates/kiln-web-service/src/campaign_controls.rs";

const fn protected(path: &'static str, item: &'static str, operation: Op) -> EntryPoint {
    EntryPoint {
        path,
        item,
        class: EntryClass::Protected(operation),
    }
}

const fn reconcile(path: &'static str, item: &'static str, operation: Op) -> EntryPoint {
    EntryPoint {
        path,
        item,
        class: EntryClass::Reconcile(operation),
    }
}

const fn observe(path: &'static str, item: &'static str) -> EntryPoint {
    EntryPoint {
        path,
        item,
        class: EntryClass::ObservationOnly,
    }
}

const fn operator(path: &'static str, item: &'static str) -> EntryPoint {
    EntryPoint {
        path,
        item,
        class: EntryClass::OperatorConfig,
    }
}

/// Every reviewed externally reachable entry point and its authority class.
pub const ENTRY_POINTS: &[EntryPoint] = &[
    // ── Implementer: composition (lib.rs, authority_config.rs) ──
    operator(LIB, "open"),
    operator(LIB, "set_authority"),
    operator(LIB, "select_local_operator"),
    operator(LIB, "open_with_local_operator"),
    operator(LIB, "open_with_authority"),
    // A query-only connection under the absent profile: every mutating method
    // fails inside SQLite, and no protected operation can admit.
    observe(LIB, "open_read_only"),
    operator(CONFIG, "select_authority"),
    // A fixed clock only makes currentness reproducible; it never creates authority.
    operator(LIB, "set_clock_ms"),
    // The mirror runs after committed transitions; overlap can only defer a claim.
    operator(LIB, "set_mirror"),
    operator(LIB, "set_overlap"),
    observe(LIB, "persisted_authority_decisions"),
    observe(LIB, "authority_decisions"),
    // ── Implementer: source intake and operator ledger maintenance ──
    // Enrollment-gated intake; supersession only ends older attempts.
    operator(LIB, "admit"),
    operator(LIB, "admit_with_supersession"),
    operator(LIB, "supersede"),
    // Expires ended leases; it never extends or grants one.
    operator(LIB, "recover"),
    operator(LIB, "attach_work_needs"),
    operator(LIB, "plan_review"),
    // ── Implementer: claims ──
    protected(LIB, "claim_next", Op::ClaimAcquire),
    protected(LIB, "claim_next_deferred", Op::ClaimAcquire),
    protected(LIB, "renew", Op::ClaimRenew),
    protected(ADMISSION, "claim_exact_winner", Op::ClaimAcquire),
    protected(CAMPAIGN, "claim_campaign_work", Op::ClaimAcquire),
    // ── Implementer: admission ledger ──
    // Claim and ClaimExact admit ClaimAcquire, Request admits Dispatch, every
    // other command is operator ledger maintenance.
    protected(ADMISSION, "admission_apply", Op::ClaimAcquire),
    protected(ADMISSION, "admission_apply", Op::Dispatch),
    operator(ADMISSION, "admission_apply"),
    protected(ADMISSION, "admission_before_transport", Op::Dispatch),
    protected(ADMISSION, "admission_reserve_with_owner", Op::Dispatch),
    reconcile(ADMISSION, "admission_execution_observed", Op::Dispatch),
    reconcile(ADMISSION, "admission_settle_observed_draw", Op::Dispatch),
    // Withdraws a queued admission item through its owner; ending the attempt
    // itself is `cancel` (AttemptCancel).
    operator(ADMISSION, "admission_cancel_with_owner"),
    observe(ADMISSION, "admission_snapshot"),
    operator(HOST_OWNER, "host_owner_admit"),
    operator(HOST_OWNER, "host_owner_observe"),
    protected(HOST_OWNER, "host_owner_reserve", Op::Dispatch),
    reconcile(HOST_OWNER, "host_owner_release", Op::Dispatch),
    // ── Implementer: dispatch ──
    // Each admits the exact Dispatch request for the attempt's reservation
    // effect inside the transaction that writes its record, after the attempt
    // ledger's fence and phase checks; a denial saves nothing. The transport
    // boundary (`admission_before_transport`) rechecks currentness at the
    // effect.
    protected(LIB, "reserve_dispatch", Op::Dispatch),
    protected(LIB, "reserve_dispatch_plan", Op::Dispatch),
    // The reservation's request admitted before a shell reads a credential;
    // saves nothing, and the reservation revalidates its session.
    protected(LIB, "authorize_dispatch_plan", Op::Dispatch),
    // The durable launch record of a started workflow (CLI `start`, the
    // fleet tick, `lattice-dispatch`).
    protected(LIB, "start_execution", Op::Dispatch),
    operator(CAMPAIGN, "bind_campaign_request"),
    // Records the launch of the transport `admission_before_transport` admitted.
    reconcile(LIB, "begin_dispatch_transport", Op::Dispatch),
    reconcile(LIB, "begin_dispatch_transport_sending", Op::Dispatch),
    // Adopts an observed run: a fresh invocation for the original reservation.
    reconcile(LIB, "reconcile_dispatch", Op::Dispatch),
    reconcile(LIB, "settle_dispatch_without_execution", Op::Dispatch),
    reconcile(LIB, "reconcile_unknown", Op::Dispatch),
    reconcile(LIB, "record_worker_observation", Op::Dispatch),
    reconcile(LIB, "observe_metrics", Op::Dispatch),
    reconcile(LIB, "record_metrics", Op::Dispatch),
    reconcile(LIB, "produce_candidate", Op::Dispatch),
    reconcile(LIB, "fail", Op::Dispatch),
    reconcile(LIB, "block", Op::Dispatch),
    observe(LIB, "authorize_current"),
    observe(LIB, "active_work"),
    observe(LIB, "work_needs"),
    observe(LIB, "dispatch_launch"),
    observe(LIB, "dispatch_request"),
    observe(LIB, "dispatch_plan"),
    observe(LIB, "dispatch_requires_recovery"),
    observe(LIB, "dispatch_reservation"),
    observe(LIB, "swarm_snapshots"),
    observe(LIB, "metrics"),
    observe(LIB, "metrics_report"),
    // The sent-request report's single read transaction; it saves nothing.
    observe(LIB, "dispatch_record"),
    observe(CAMPAIGN, "campaign_request_binding"),
    // ── Implementer: candidate, verification and cancellation ──
    protected(LIB, "admit_candidate_handoff", Op::CandidateSubmit),
    protected(LIB, "record_verification", Op::VerificationAdmit),
    reconcile(LIB, "begin_verification", Op::VerificationAdmit),
    reconcile(LIB, "record_verification_outcome", Op::VerificationAdmit),
    protected(LIB, "cancel", Op::AttemptCancel),
    observe(LIB, "candidate_handoff"),
    observe(LIB, "verification_receipt"),
    // ── Implementer: publication, integration and lifecycle ──
    protected(LIB, "prepare_publication", Op::PublicationPublish),
    protected(LIB, "record_source_status", Op::PublicationPublish),
    protected(LIB, "record_branch_published", Op::PublicationPublish),
    protected(LIB, "record_pull_request_published", Op::PublicationPublish),
    protected(LIB, "record_integration", Op::IntegrationLand),
    protected(
        EFFECTS,
        "reserve_publication_effect",
        Op::PublicationPublish,
    ),
    protected(
        EFFECTS,
        "consume_publication_effect",
        Op::PublicationPublish,
    ),
    reconcile(
        EFFECTS,
        "acknowledge_publication_effect",
        Op::PublicationPublish,
    ),
    // Persists the exact plan; its effect is admitted when consumed.
    operator(NATIVE, "prepare_native_git"),
    protected(
        NATIVE,
        "consume_native_git_effect_with_current_authority",
        Op::PublicationPublish,
    ),
    protected(
        NATIVE,
        "consume_native_git_effect_with_current_authority",
        Op::IntegrationLand,
    ),
    protected(
        NATIVE,
        "record_native_cairn_lifecycle",
        Op::LifecycleComplete,
    ),
    observe(LIB, "publication_record"),
    observe(LIB, "integration_receipt"),
    observe(LIB, "source_status_receipt"),
    observe(EFFECTS, "read_publication_effect"),
    observe(EFFECTS, "read_cairn_lifecycle_effect"),
    observe(NATIVE, "native_cairn_lifecycle"),
    observe(NATIVE, "native_git_plan"),
    observe(NATIVE, "native_git_effect_reservation"),
    observe(NATIVE, "native_git_receipt"),
    protected(STACK, "prepare_stack_publication", Op::PublicationPublish),
    protected(STACK, "probe_publication_route", Op::PublicationPublish),
    protected(
        STACK,
        "consume_stack_publication_effect",
        Op::PublicationPublish,
    ),
    protected(
        STACK,
        "consume_stack_integration_effect",
        Op::IntegrationLand,
    ),
    observe(STACK, "stack_publication_record"),
    observe(STACK, "read_stack_effect"),
    // ── Implementer: work and attempt reads ──
    observe(LIB, "work"),
    observe(LIB, "attempts"),
    observe(LIB, "attempt"),
    observe(LIB, "plan_feedback"),
    // ── Implementer: fleet ledger ──
    // A fleet scope lease between authenticated fleet machines; it creates no
    // attempt. Attempt claims go through `claim_next_deferred`.
    operator(FLEET, "fleet_claim"),
    // The fleet launch: the bind is the durable launch record and admits the
    // launch's exact Dispatch request before it is saved; the tick admits it
    // again before the worktree and agent start, before it adopts an agent
    // found after a restart, and before every first prompt.
    protected(FLEET, "fleet_bind_session", Op::Dispatch),
    protected(FLEET, "fleet_authorize_launch", Op::Dispatch),
    operator(FLEET, "fleet_clear_placing"),
    operator(FLEET, "fleet_record_issued"),
    operator(FLEET, "fleet_record_issued_with_policy"),
    operator(FLEET, "fleet_answer_decision"),
    reconcile(FLEET, "fleet_observe", Op::Dispatch),
    reconcile(FLEET, "fleet_raise_decision", Op::Dispatch),
    observe(FLEET, "fleet_state"),
    observe(FLEET, "fleet_version"),
    observe(FLEET, "fleet_observation_version"),
    observe(FLEET, "fleet_attempt_scopes"),
    observe(FLEET, "fleet_pending_mirror"),
    observe(FLEET, "fleet_attempt_views"),
    observe(FLEET, "fleet_ready_work"),
    observe(FLEET, "fleet_plan"),
    observe(FLEET, "fleet_swarms"),
    observe(FLEET, "fleet_swarm"),
    operator(FLEET, "fleet_swarm_create"),
    operator(FLEET, "fleet_swarm_spawn"),
    operator(FLEET, "fleet_swarm_update_member"),
    operator(FLEET, "fleet_swarm_post"),
    observe(FLEET, "fleet_swarm_read_board"),
    operator(FLEET, "fleet_swarm_verdict"),
    operator(FLEET, "fleet_swarm_watchdog"),
    operator(FLEET, "fleet_swarm_failover"),
    observe(FLEET, "fleet_swarm_graph"),
    // ── kiln-implementer CLI ──
    // Source intake loops: they admit work and never claim or dispatch.
    operator(CLI, "drain"),
    operator(CLI, "run"),
    operator(CLI, "poll"),
    operator(CLI, "poll-run"),
    operator(CLI, "drain-runbooks"),
    operator(CLI, "poll-runbooks"),
    observe(CLI, "policy-check"),
    observe(CLI, "status"),
    protected(CLI, "claim", Op::ClaimAcquire),
    protected(CLI, "cancel", Op::AttemptCancel),
    protected(CLI, "renew", Op::ClaimRenew),
    protected(CLI, "reserve", Op::Dispatch),
    protected(CLI, "reserve-plan", Op::Dispatch),
    protected(CLI, "start", Op::Dispatch),
    reconcile(CLI, "reconcile", Op::Dispatch),
    protected(CLI, "handoff", Op::CandidateSubmit),
    protected(CLI, "capture-candidate", Op::CandidateSubmit),
    // Writes a verification profile file; touches no ledger.
    operator(CLI, "prepare-verification-profile"),
    operator(CLI, "prepare-needs"),
    protected(CLI, "prepare-publication", Op::PublicationPublish),
    protected(CLI, "publish", Op::PublicationPublish),
    protected(CLI, "prepare-stack-publication", Op::PublicationPublish),
    protected(CLI, "publish-stack", Op::PublicationPublish),
    protected(CLI, "recover-stack-integration", Op::IntegrationLand),
    observe(CLI, "stack-gate"),
    // Refuses unconditionally; performs nothing.
    observe(CLI, "land"),
    protected(CLI, "recover-integration", Op::PublicationPublish),
    protected(CLI, "close-source", Op::PublicationPublish),
    observe(CLI, "source-status"),
    // `metrics`, `reconcile-termination` and `host-owner-release` record a
    // fence-checked worker metric or provider termination, or settle an
    // already-ended owner reservation. They check no authority, so they load
    // no holder seed and work without one.
    operator(CLI, "metrics"),
    observe(CLI, "metrics-report"),
    operator(CLI, "recover"),
    operator(CLI, "reconcile-termination"),
    operator(CLI, "attach-needs"),
    observe(CLI, "needs"),
    protected(CLI, "admission", Op::ClaimAcquire),
    protected(CLI, "admission", Op::Dispatch),
    operator(CLI, "admission"),
    observe(CLI, "admission-status"),
    observe(CLI, "admission-explanation"),
    operator(CLI, "host-owner-admit"),
    operator(CLI, "host-owner-observe"),
    protected(CLI, "host-owner-reserve", Op::Dispatch),
    operator(CLI, "host-owner-release"),
    // Applies a typed command file to the campaign store; the campaign's
    // protected effects are admitted when the drive performs them.
    operator(CLI, "campaign"),
    protected(CLI, "observe-cairn-lifecycle", Op::PublicationPublish),
    observe(CLI, "inspect-work"),
    observe(CLI, "inspect-attempt"),
    operator(CLI, "plan-review"),
    observe(CLI, "plan-feedback"),
    observe(CLI, "plan-review-receipt"),
    // ── Fleet API ──
    observe(FLEET_API, "GET /v1/health"),
    observe(FLEET_API, "GET /v1/machines"),
    operator(FLEET_API, "POST /v1/machines/{name}/drain"),
    observe(FLEET_API, "GET /v1/repositories"),
    observe(FLEET_API, "GET /v1/version"),
    observe(FLEET_API, "GET /v1/claims"),
    operator(FLEET_API, "POST /v1/claims"),
    operator(FLEET_API, "POST /v1/claims/{id}/renew"),
    operator(FLEET_API, "POST /v1/claims/{id}/release"),
    observe(FLEET_API, "GET /v1/work"),
    observe(FLEET_API, "GET /v1/attempts"),
    observe(FLEET_API, "GET /v1/attempts/{id}/pane"),
    operator(FLEET_API, "POST /v1/attempts/{id}/prompt"),
    operator(FLEET_API, "POST /v1/attempts/{id}/interrupt"),
    reconcile(FLEET_API, "POST /v1/attempts/{id}/handoff", Op::Dispatch),
    protected(
        FLEET_API,
        "POST /v1/attempts/{id}/cancel",
        Op::AttemptCancel,
    ),
    observe(FLEET_API, "GET /v1/decisions"),
    observe(FLEET_API, "GET /v1/metrics"),
    operator(FLEET_API, "POST /v1/decisions/{id}/answer"),
    observe(FLEET_API, "GET /v1/dispatch/status"),
    operator(FLEET_API, "POST /v1/dispatch/pause"),
    operator(FLEET_API, "POST /v1/dispatch/resume"),
    observe(FLEET_API, "GET /v1/swarms"),
    operator(FLEET_API, "POST /v1/swarms"),
    observe(FLEET_API, "GET /v1/swarms/{id}"),
    operator(FLEET_API, "POST /v1/swarms/{id}/members"),
    observe(FLEET_API, "GET /v1/swarms/{id}/board"),
    operator(FLEET_API, "POST /v1/swarms/{id}/board"),
    operator(FLEET_API, "POST /v1/swarms/{id}/verdicts"),
    operator(FLEET_API, "POST /v1/swarms/{id}/watchdog"),
    operator(FLEET_API, "POST /v1/swarms/{id}/failover"),
    observe(FLEET_API, "GET /v1/swarms/{id}/graph"),
    // ── Web campaign controls ──
    observe(CONTROLS, "GET /api/implementation/campaign-definitions"),
    observe(CONTROLS, "GET /api/implementation/campaigns/inbox"),
    // Typed campaign intents under the reread control profile; the campaign's
    // protected effects are admitted when the drive performs them. The drive
    // launches its worker (`kiln-lattice-dispatch`), capture
    // (`capture-candidate`) and verifier (`verify-implementation`) roles with
    // exactly its own authority source (`--local-operator` or
    // `--authority-config`, or neither for the absent profile), so each role
    // decides on the drive's profile at its own protected rows above.
    operator(CONTROLS, "POST /api/implementation/campaigns/commands"),
];
