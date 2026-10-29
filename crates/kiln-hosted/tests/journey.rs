//! Consumer journeys through the durable hosted service and its real local QUIC shell.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use iroh::{Endpoint, EndpointAddr};
use kiln_hosted::{
    MAX_FRAME_BYTES, Request, Response, SCHEMA,
    operations::{self, OperationsProfile},
    service::{Profile, Service},
    transport::{self, NetworkProfile},
};
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, sync::oneshot, time::timeout};

const ORG: &str = "journey-org";
const BOUND: Duration = Duration::from_secs(10);

struct Fixture {
    directory: tempfile::TempDir,
    profile: Profile,
    owner: String,
    owner_network: NetworkProfile,
}

fn private_directory(path: &Path) -> PathBuf {
    fs::create_dir(path)
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    path.to_path_buf()
}

fn identity(directory: &Path, name: &str) -> (String, NetworkProfile) {
    let key_file = directory.join(format!("{name}.key"));
    let peer = transport::keygen(&key_file)
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    (
        peer,
        NetworkProfile {
            key_file,
            bind_address: "127.0.0.1:0"
                .parse()
                .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}")),
            relay_urls: Vec::new(),
        },
    )
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir()
            .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
        let (owner, owner_network) = identity(directory.path(), "owner");
        let (_, network) = identity(directory.path(), "server");
        let operations = OperationsProfile {
            service_id: "journey-service".into(),
            backup_root: private_directory(&directory.path().join("backups")),
            restore_root: private_directory(&directory.path().join("restores")),
            content_root: private_directory(&directory.path().join("content")),
            replay_roots: Vec::new(),
            maximum_database_bytes: 16 * 1024 * 1024,
            maximum_content_bytes: 1024,
            maximum_content_records: 10,
            maximum_backup_ms: 10_000,
        };
        let profile = Profile {
            schema: SCHEMA.into(),
            service_id: operations.service_id.clone(),
            state_directory: directory.path().join("state"),
            network,
            bootstrap_peer: owner.clone(),
            bootstrap_organization: ORG.into(),
            operator_peers: vec![owner.clone()],
            source_profiles: Vec::new(),
            operations: Some(operations),
            secrets: None,
            execution: None,
            fleet: None,
        };
        Self {
            directory,
            profile,
            owner,
            owner_network,
        }
    }

    fn peer(&self, name: &str) -> String {
        identity(self.directory.path(), name).0
    }
}

fn request(id: &str, action: &str, input: Value) -> Request {
    Request {
        schema: SCHEMA.into(),
        organization: ORG.into(),
        request_id: id.into(),
        action: action.into(),
        input,
    }
}

fn accepted(response: Response) -> Value {
    assert!(
        response.error.is_none(),
        "unexpected service rejection: {:?}",
        response.error
    );
    response
        .result
        .unwrap_or_else(|| unreachable!("successful response has a result"))
}

fn rejected(response: Response) {
    assert!(
        response.error.is_some(),
        "unexpected acceptance: {:?}",
        response.result
    );
    assert!(response.result.is_none());
}

fn call(service: &mut Service, peer: &str, id: &str, action: &str, input: Value) -> Value {
    accepted(service.handle(peer, request(id, action, input)))
}

fn enroll(service: &mut Service, owner: &str, peer: &str, role: &str) {
    let invitation = call(
        service,
        owner,
        "invite-guest",
        "accounts.invite",
        json!({
            "peer": peer, "role": role, "projects": ["alpha"],
            "expires_at_ms": kiln_hosted::now_ms().unwrap_or_else(|error| unreachable!("clock unavailable: {error:?}")) + 60_000,
            "nonce": "guest-invitation"
        }),
    );
    let membership = call(
        service,
        peer,
        "accept-guest",
        "accounts.accept",
        json!({
            "invitation": invitation.get("invitation").unwrap_or(&Value::Null)
        }),
    );
    assert_eq!(membership.get("role").unwrap_or(&Value::Null), role);
    assert_eq!(membership.get("revision").unwrap_or(&Value::Null), 1);
}

fn change_membership(
    service: &mut Service,
    owner: &str,
    peer: &str,
    revision: u64,
    role: &str,
    projects: &[&str],
    active: bool,
) {
    let result = call(
        service,
        owner,
        &format!("membership-{revision}"),
        "accounts.set_membership",
        json!({
            "peer": peer, "expected_revision": revision, "role": role,
            "projects": projects, "active": active
        }),
    );
    assert_eq!(result.get("revision").unwrap_or(&Value::Null), revision + 1);
    assert_eq!(result.get("active").unwrap_or(&Value::Null), active);
}

fn pipeline(command: &str) -> Value {
    json!({
        "schema": "kiln.hosted.pipeline.v1", "name": "Checks", "triggers": ["manual"],
        "jobs": {"build": {"runner": "linux", "steps": [{"kind": "run", "run": command}]}}
    })
}

fn review(service: &mut Service, owner: &str, revision: u64, command: &str) -> Value {
    call(
        service,
        owner,
        &format!("review-{revision}"),
        "pipelines.review",
        json!({
            "project": "alpha", "name": "checks", "expected_revision": revision,
            "config": pipeline(command)
        }),
    )
}

fn configure_finance(service: &mut Service, owner: &str) -> Value {
    let account = call(
        service,
        owner,
        "configure-account",
        "finance.account.configure",
        json!({
            "request_id": "account-configuration", "expected_revision": 0,
            "currency": "usd", "cap_minor": 100, "request_max_minor": 100, "max_concurrent": 2
        }),
    );
    assert_eq!(account.get("available_minor").unwrap_or(&Value::Null), 100);
    call(
        service,
        owner,
        "admit-price",
        "finance.price.admit",
        json!({
            "class": "linux", "currency": "usd", "unit_minor": 10, "unit_ms": 1000, "max_units": 10
        }),
    )
    .get("price_id")
    .unwrap_or(&Value::Null)
    .clone()
}

fn reservation(attempt: &str, price: &Value, plan: &Value, units: u64) -> Value {
    json!({
        "attempt_id": attempt, "price_id": price, "class": "linux", "max_units": units,
        "binding": {"project": "alpha", "pipeline": plan, "job": "build"}
    })
}

// r[verify kiln.hosted.accounts.scenario.membership]
#[test]
fn current_membership_and_project_authority_precede_retained_responses() {
    let fixture = Fixture::new();
    let guest = fixture.peer("guest");
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    enroll(&mut service, &fixture.owner, &guest, "admin");
    let reviewed = review(&mut service, &fixture.owner, 0, "printf original");
    let lookup = request(
        "cached-plan",
        "pipelines.get",
        json!({"project": "alpha", "identity": reviewed.get("identity").unwrap_or(&Value::Null)}),
    );
    let original = accepted(service.handle(&guest, lookup.clone()));
    assert_eq!(
        original
            .pointer("/jobs/0/steps/0/run")
            .unwrap_or(&Value::Null),
        "printf original"
    );
    assert_eq!(accepted(service.handle(&guest, lookup.clone())), original);
    let privileged = request("cached-members", "accounts.list", json!({}));
    let members = accepted(service.handle(&guest, privileged.clone()));
    assert!(
        members
            .get("members")
            .unwrap_or(&Value::Null)
            .as_array()
            .unwrap_or_else(|| unreachable!("expected members array"))
            .iter()
            .any(|member| member.get("peer").and_then(Value::as_str)
                == Some(fixture.owner.as_str())
                && member.get("role").unwrap_or(&Value::Null) == "owner")
    );

    // A downgraded role must not retrieve a response produced under admin authority.
    change_membership(
        &mut service,
        &fixture.owner,
        &guest,
        1,
        "member",
        &["alpha"],
        true,
    );
    rejected(service.handle(&guest, lookup.clone()));
    rejected(service.handle(&guest, privileged));
    rejected(service.handle(
        &guest,
        request("fresh-members-denied", "accounts.list", json!({})),
    ));
    let member_lookup = request("member-plan", "pipelines.get", lookup.input.clone());
    assert_eq!(
        accepted(service.handle(&guest, member_lookup.clone())),
        original
    );

    // Changing only the project revision also invalidates previously retained reads.
    change_membership(
        &mut service,
        &fixture.owner,
        &guest,
        2,
        "member",
        &["beta"],
        true,
    );
    rejected(service.handle(&guest, member_lookup.clone()));
    rejected(service.handle(
        &guest,
        request("denied-alpha", "pipelines.get", lookup.input.clone()),
    ));
    change_membership(
        &mut service,
        &fixture.owner,
        &guest,
        3,
        "member",
        &["alpha"],
        true,
    );
    rejected(service.handle(&guest, member_lookup));
    assert_eq!(
        call(
            &mut service,
            &guest,
            "restored-scope",
            "pipelines.get",
            lookup.input.clone()
        ),
        original
    );

    change_membership(
        &mut service,
        &fixture.owner,
        &guest,
        4,
        "member",
        &["alpha"],
        false,
    );
    rejected(service.handle(&guest, lookup.clone()));
    drop(service);
    let mut reopened = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    rejected(reopened.handle(&guest, lookup));
    let members = call(
        &mut reopened,
        &fixture.owner,
        "membership-after-restart",
        "accounts.list",
        json!({}),
    );
    let guest_membership = members
        .get("members")
        .unwrap_or(&Value::Null)
        .as_array()
        .unwrap_or_else(|| unreachable!("expected journey array"))
        .iter()
        .find(|m| m.get("peer").and_then(Value::as_str) == Some(guest.as_str()))
        .unwrap_or_else(|| unreachable!("expected journey entry"));
    assert_eq!(
        guest_membership.get("active").unwrap_or(&Value::Null),
        false
    );
    assert_eq!(guest_membership.get("revision").unwrap_or(&Value::Null), 5);
}

// r[verify kiln.hosted.finance.scenario.cap]
#[test]
fn reviewed_plan_and_exact_reservations_survive_restart_without_releasing_exposure() {
    let fixture = Fixture::new();
    let member = fixture.peer("member");
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    enroll(&mut service, &fixture.owner, &member, "member");
    let plan = review(&mut service, &fixture.owner, 0, "printf reviewed");
    let price = configure_finance(&mut service, &fixture.owner);
    let input = reservation(
        "attempt-one",
        &price,
        plan.get("identity").unwrap_or(&Value::Null),
        6,
    );
    let reserve = request("reserve-one", "finance.reserve", input.clone());
    let first = accepted(service.handle(&member, reserve.clone()));
    assert_eq!(first.get("maximum_minor").unwrap_or(&Value::Null), 60);
    assert_eq!(
        first.pointer("/binding/pipeline").unwrap_or(&Value::Null),
        plan.get("identity").unwrap_or(&Value::Null)
    );
    assert_eq!(accepted(service.handle(&member, reserve.clone())), first);

    let mut changed = reserve.clone();
    *changed
        .input
        .get_mut("max_units")
        .unwrap_or_else(|| unreachable!("expected mutable request field")) = json!(5);
    rejected(service.handle(&member, changed.clone()));
    changed.request_id = "conflicting-attempt".into();
    rejected(service.handle(&member, changed));
    // Another correctly priced attempt still cannot exceed the retained spend cap.
    rejected(service.handle(
        &member,
        request(
            "over-cap",
            "finance.reserve",
            reservation(
                "attempt-too-large",
                &price,
                plan.get("identity").unwrap_or(&Value::Null),
                5,
            ),
        ),
    ));
    let account = call(
        &mut service,
        &member,
        "account-before-restart",
        "finance.account.get",
        json!({}),
    );
    assert_eq!(account.get("reserved_minor").unwrap_or(&Value::Null), 60);
    assert_eq!(account.get("available_minor").unwrap_or(&Value::Null), 40);

    drop(service);
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    assert_eq!(accepted(service.handle(&member, reserve)), first);
    let account = call(
        &mut service,
        &member,
        "account-after-restart",
        "finance.account.get",
        json!({}),
    );
    assert_eq!(account.get("reserved_minor").unwrap_or(&Value::Null), 60);
    let read = call(
        &mut service,
        &member,
        "reservation-after-restart",
        "finance.reservation.get",
        json!({"attempt_id": "attempt-one"}),
    );
    assert_eq!(read, first);

    // Plan identities are project-bound and a new review retires the previous plan.
    rejected(service.handle(
        &fixture.owner,
        request(
            "wrong-project-plan",
            "pipelines.get",
            json!({"project": "beta", "identity": plan.get("identity").unwrap_or(&Value::Null)}),
        ),
    ));
    let next = review(&mut service, &fixture.owner, 1, "printf replacement");
    assert_ne!(
        next.get("identity").unwrap_or(&Value::Null),
        plan.get("identity").unwrap_or(&Value::Null)
    );
    rejected(service.handle(
        &member,
        request(
            "retired-plan",
            "pipelines.get",
            json!({"project": "alpha", "identity": plan.get("identity").unwrap_or(&Value::Null)}),
        ),
    ));
    let current = call(
        &mut service,
        &member,
        "current-plan",
        "pipelines.get",
        json!({"project": "alpha", "identity": next.get("identity").unwrap_or(&Value::Null)}),
    );
    assert_eq!(
        current
            .pointer("/jobs/0/steps/0/run")
            .unwrap_or(&Value::Null),
        "printf replacement"
    );
    // Retiring a plan never erases the financial obligation for an accepted attempt.
    assert_eq!(
        call(
            &mut service,
            &member,
            "retained-attempt",
            "finance.reservation.get",
            json!({"attempt_id": "attempt-one"})
        ),
        first
    );
}

// r[verify kiln.hosted.operations.scenario.restore]
#[test]
fn incident_hold_denies_new_consumption_but_preserves_recovery_and_exact_retry() {
    let fixture = Fixture::new();
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    let price = configure_finance(&mut service, &fixture.owner);
    let plan = review(&mut service, &fixture.owner, 0, "printf held");
    let prior = request(
        "reserve-before-hold",
        "finance.reserve",
        reservation(
            "existing",
            &price,
            plan.get("identity").unwrap_or(&Value::Null),
            6,
        ),
    );
    let retained = accepted(service.handle(&fixture.owner, prior.clone()));
    call(
        &mut service,
        &fixture.owner,
        "hold",
        "operations.hold.set",
        json!({"held": true, "reason": "investigate execution", "expected_revision": 0}),
    );
    rejected(service.handle(
        &fixture.owner,
        request(
            "reserve-during-hold",
            "finance.reserve",
            reservation(
                "new",
                &price,
                plan.get("identity").unwrap_or(&Value::Null),
                1,
            ),
        ),
    ));
    assert_eq!(accepted(service.handle(&fixture.owner, prior)), retained);
    assert_eq!(
        call(
            &mut service,
            &fixture.owner,
            "read-existing",
            "finance.reservation.get",
            json!({"attempt_id": "existing"})
        ),
        retained
    );
    let account = call(
        &mut service,
        &fixture.owner,
        "held-account",
        "finance.account.get",
        json!({}),
    );
    assert_eq!(account.get("reserved_minor").unwrap_or(&Value::Null), 60);
    assert_eq!(account.get("spent_minor").unwrap_or(&Value::Null), 0);
    assert_eq!(
        call(
            &mut service,
            &fixture.owner,
            "read-hold",
            "operations.hold.get",
            json!({})
        )
        .get("held")
        .unwrap_or(&Value::Null),
        true
    );
    let snapshot = call(
        &mut service,
        &fixture.owner,
        "backup-during-hold",
        "operations.backup",
        json!({"backup_id": "held"}),
    );
    assert_eq!(
        snapshot
            .pointer("/manifest/service_id")
            .and_then(Value::as_str),
        Some(fixture.profile.service_id.as_str())
    );
    call(
        &mut service,
        &fixture.owner,
        "release-hold",
        "operations.hold.set",
        json!({"held": false, "reason": "recovery complete", "expected_revision": 1}),
    );
    let after = call(
        &mut service,
        &fixture.owner,
        "reserve-after-release",
        "finance.reserve",
        reservation(
            "new",
            &price,
            plan.get("identity").unwrap_or(&Value::Null),
            1,
        ),
    );
    assert_eq!(after.get("maximum_minor").unwrap_or(&Value::Null), 10);
    assert_eq!(
        call(
            &mut service,
            &fixture.owner,
            "released-account",
            "finance.account.get",
            json!({})
        )
        .get("reserved_minor")
        .unwrap_or(&Value::Null),
        70
    );
}

// r[verify kiln.hosted.operations.scenario.restore]
#[test]
fn whole_service_restore_preserves_membership_unknown_obligations_and_request_replay() {
    let fixture = Fixture::new();
    let member = fixture.peer("backup-member");
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    enroll(&mut service, &fixture.owner, &member, "member");
    let plan = review(&mut service, &fixture.owner, 0, "printf backup");
    let price = configure_finance(&mut service, &fixture.owner);
    let reserve = request(
        "reserved-before-backup",
        "finance.reserve",
        reservation(
            "unknown-attempt",
            &price,
            plan.get("identity").unwrap_or(&Value::Null),
            6,
        ),
    );
    let original = accepted(service.handle(&member, reserve.clone()));
    // Only the real host-side metering entry point can record uncertain execution.
    let now =
        kiln_hosted::now_ms().unwrap_or_else(|error| unreachable!("clock unavailable: {error:?}"));
    kiln_hosted::finance::observe_unknown_trusted(
        &mut service.db,
        ORG,
        "unknown-attempt",
        "lost-worker",
        "worker disconnected without trusted usage",
        now,
    )
    .unwrap_or_else(|error| unreachable!("trusted observation rejected: {error:?}"));
    let pending = call(
        &mut service,
        &member,
        "unknown-before-backup",
        "finance.reservation.get",
        json!({"attempt_id": "unknown-attempt"}),
    );
    assert_eq!(pending.get("state").unwrap_or(&Value::Null), "unknown");
    let backup = call(
        &mut service,
        &fixture.owner,
        "backup-whole-service",
        "operations.backup",
        json!({"backup_id": "whole"}),
    );
    let operations = fixture
        .profile
        .operations
        .as_ref()
        .unwrap_or_else(|| unreachable!("expected configured operations"));
    let pin = backup
        .get("identity")
        .unwrap_or(&Value::Null)
        .as_str()
        .unwrap_or_else(|| unreachable!("expected journey string"));
    assert!(
        operations::restore(operations, "whole", "untrusted", "wrong-manifest-identity").is_err()
    );
    let restored = operations::restore(operations, "whole", "recovered", pin)
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    assert!(operations::restore(operations, "whole", "recovered", pin).is_err());
    let state_directory = restored
        .parent()
        .unwrap_or_else(|| unreachable!("expected restored parent directory"))
        .to_path_buf();
    drop(service);
    let mut profile = fixture.profile.clone();
    profile.state_directory = state_directory;
    let mut recovered = Service::open(profile)
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    assert_eq!(
        call(
            &mut recovered,
            &member,
            "restored-role",
            "system.info",
            json!({})
        )
        .get("role")
        .unwrap_or(&Value::Null),
        "member"
    );
    assert_eq!(
        accepted(recovered.handle(&member, reserve.clone())),
        original
    );
    let mut conflict = reserve;
    *conflict
        .input
        .get_mut("max_units")
        .unwrap_or_else(|| unreachable!("expected mutable request field")) = json!(4);
    rejected(recovered.handle(&member, conflict));
    assert_eq!(
        call(
            &mut recovered,
            &member,
            "restored-reservation",
            "finance.reservation.get",
            json!({"attempt_id": "unknown-attempt"})
        ),
        pending
    );
    let account = call(
        &mut recovered,
        &member,
        "restored-account",
        "finance.account.get",
        json!({}),
    );
    assert_eq!(account.get("reserved_minor").unwrap_or(&Value::Null), 60);
    assert_eq!(account.get("available_minor").unwrap_or(&Value::Null), 40);
    rejected(recovered.handle(
        &member,
        request(
            "restored-over-cap",
            "finance.reserve",
            reservation(
                "next-attempt",
                &price,
                plan.get("identity").unwrap_or(&Value::Null),
                5,
            ),
        ),
    ));
    let current = call(
        &mut recovered,
        &member,
        "restored-reviewed-plan",
        "pipelines.get",
        json!({"project": "alpha", "identity": plan.get("identity").unwrap_or(&Value::Null)}),
    );
    assert_eq!(
        current
            .pointer("/jobs/0/steps/0/run")
            .unwrap_or(&Value::Null),
        "printf backup"
    );
}

fn local_address(endpoint: &Endpoint) -> EndpointAddr {
    let sockets = endpoint.bound_sockets();
    assert!(sockets.iter().all(|socket| socket.ip().is_loopback()));
    EndpointAddr::new(endpoint.id()).with_ip_addr(
        *sockets
            .iter()
            .find(|socket| socket.is_ipv4())
            .unwrap_or_else(|| unreachable!("expected journey entry")),
    )
}

async fn rejected_frame(endpoint: &Endpoint, remote: &EndpointAddr, bytes: &[u8]) {
    timeout(BOUND, async {
        let connection = endpoint
            .connect(remote.clone(), transport::ALPN)
            .await
            .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
        let (mut send, mut receive) = connection
            .open_bi()
            .await
            .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
        AsyncWriteExt::write_all(&mut send, bytes)
            .await
            .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
        send.finish()
            .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
        // Rejection resets/ends the stream; it must not return an accepted response.
        let result = receive.read_to_end(MAX_FRAME_BYTES).await;
        assert!(
            result.as_ref().map_or(true, Vec::is_empty),
            "invalid frame produced a response: {result:?}"
        );
        connection.close(0_u32.into(), b"negative frame observed");
    })
    .await
    .unwrap_or_else(|error| unreachable!("invalid frames must be rejected promptly: {error:?}"));
}

async fn rpc(endpoint: &Endpoint, remote: &EndpointAddr, request: &Request) -> Response {
    timeout(BOUND, transport::call(endpoint, remote, request))
        .await
        .unwrap_or_else(|error| unreachable!("RPC deadline elapsed: {error:?}"))
        .unwrap_or_else(|error| unreachable!("RPC exchange failed: {error:?}"))
}

// r[verify kiln.hosted.iroh.scenario.peers]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_iroh_authenticates_peer_and_rejects_invalid_frames_without_dispatch() {
    let fixture = Fixture::new();
    let (_, unknown_network) = identity(fixture.directory.path(), "unknown");
    let service = Arc::new(Mutex::new(
        Service::open(fixture.profile.clone())
            .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}")),
    ));
    let server = transport::bind(&fixture.profile.network)
        .await
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    let remote = local_address(&server);
    let owner = transport::bind(&fixture.owner_network)
        .await
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    let unknown = transport::bind(&unknown_network)
        .await
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    let (shutdown, receive_shutdown) = oneshot::channel();
    let serving = tokio::spawn(transport::serve(server, service, async {
        let _ = receive_shutdown.await;
    }));

    // Identical application input differs only in the real QUIC-authenticated peer.
    let info = request("identity-exchange", "system.info", json!({}));
    rejected(rpc(&unknown, &remote, &info).await);
    let response = accepted(rpc(&owner, &remote, &info).await);
    assert_eq!(
        response.get("peer").and_then(Value::as_str),
        Some(fixture.owner.as_str())
    );
    assert_eq!(response.get("role").unwrap_or(&Value::Null), "owner");
    let mut other_org = request("cross-organization", "system.info", json!({}));
    other_org.organization = "unrelated-org".into();
    rejected(rpc(&owner, &remote, &other_org).await);

    let oversized_length = u32::try_from(MAX_FRAME_BYTES + 1)
        .unwrap_or_else(|error| unreachable!("frame limit exceeds wire integer: {error:?}"));
    rejected_frame(&owner, &remote, &oversized_length.to_be_bytes()).await;
    rejected_frame(&owner, &remote, &0_u32.to_be_bytes()).await;
    let mut malformed = 1_u32.to_be_bytes().to_vec();
    malformed.push(b'{');
    rejected_frame(&owner, &remote, &malformed).await;
    let support = request(
        "valid-after-trailing",
        "operations.support.record",
        json!({"subject": "incident", "observation": "retained once"}),
    );
    let body = serde_json::to_vec(&support)
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    let body_length = u32::try_from(body.len())
        .unwrap_or_else(|error| unreachable!("fixture body exceeds wire integer: {error:?}"));
    let mut trailing = body_length.to_be_bytes().to_vec();
    trailing.extend_from_slice(&body);
    trailing.push(b'x');
    rejected_frame(&owner, &remote, &trailing).await;
    let result = accepted(rpc(&owner, &remote, &support).await);
    assert_eq!(
        result.get("sequence").unwrap_or(&Value::Null),
        1,
        "rejected trailing frame must not dispatch the support operation"
    );
    let audit = request(
        "inspect-frame-effects",
        "operations.audit.list",
        json!({"after": 0, "limit": 10}),
    );
    let audit = accepted(rpc(&owner, &remote, &audit).await);
    assert_eq!(
        audit
            .get("observations")
            .unwrap_or(&Value::Null)
            .as_array()
            .unwrap_or_else(|| unreachable!("expected audit array"))
            .iter()
            .map(|row| row
                .get("observation")
                .unwrap_or(&Value::Null)
                .as_str()
                .unwrap_or_else(|| unreachable!("expected audit text")))
            .collect::<Vec<_>>(),
        vec!["retained once"]
    );

    owner.close().await;
    unknown.close().await;
    shutdown
        .send(())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    timeout(BOUND, serving)
        .await
        .unwrap_or_else(|error| unreachable!("server shutdown deadline elapsed: {error:?}"))
        .unwrap_or_else(|error| unreachable!("server task failed: {error:?}"))
        .unwrap_or_else(|error| unreachable!("server failed: {error:?}"));
}

#[test]
fn organization_owner_without_host_authority_cannot_create_whole_service_backup() {
    let fixture = Fixture::new();
    let mut profile = fixture.profile.clone();
    profile.operator_peers.clear();
    let backup_root = profile
        .operations
        .as_ref()
        .unwrap_or_else(|| unreachable!("expected configured operations"))
        .backup_root
        .clone();
    let mut service = Service::open(profile)
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    rejected(service.handle(
        &fixture.owner,
        request(
            "owner-global-backup",
            "operations.backup",
            json!({"backup_id": "forbidden"}),
        ),
    ));
    assert!(
        !backup_root.join("forbidden").exists(),
        "denied global backup must not create snapshot artifacts"
    );
    // Lack of host-wide authority must not remove ordinary organization administration.
    let members = call(
        &mut service,
        &fixture.owner,
        "owner-local-members",
        "accounts.list",
        json!({}),
    );
    assert!(
        members
            .get("members")
            .unwrap_or(&Value::Null)
            .as_array()
            .unwrap_or_else(|| unreachable!("expected members array"))
            .iter()
            .any(|member| member.get("peer").and_then(Value::as_str)
                == Some(fixture.owner.as_str())
                && member.get("active").unwrap_or(&Value::Null) == true)
    );
}

#[test]
fn rejected_enrollment_does_not_consume_effect_replay_capacity() {
    let fixture = Fixture::new();
    let stranger = fixture.peer("stranger");
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    for id in ["invalid-invitation-a", "invalid-invitation-b"] {
        let mut attempt = request(id, "accounts.accept", json!({"invitation":"absent"}));
        attempt.organization = "never-enrolled".into();
        assert_eq!(
            service.handle(&stranger, attempt).error.as_deref(),
            Some("invitation unavailable")
        );
    }
    drop(service);
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    for table in ["hosted_requests", "hosted_audit"] {
        let count: i64 = service
            .db
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap_or_else(|error| unreachable!("retained state unavailable: {error:?}"));
        assert_eq!(
            count, 0,
            "rejected unaffiliated requests retained in {table}"
        );
    }
    let info = call(
        &mut service,
        &fixture.owner,
        "owner-after-invalid",
        "system.info",
        json!({}),
    );
    assert_eq!(info.get("organization").and_then(Value::as_str), Some(ORG));
}

#[test]
fn issuer_metadata_reads_preserve_replay_capacity_and_current_authority() {
    let mut fixture = Fixture::new();
    let key = fixture.directory.path().join("issuer.seed");
    fs::write(&key, [17u8; 32])
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600))
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    fixture.profile.secrets = Some(kiln_hosted::secrets::Config {
        issuer: "https://issuer.example.test".into(),
        signing_key_path: key,
        audiences: vec!["https://resource.example.test".into()],
        token_lifetime_seconds: 120,
        resolvers: Vec::new(),
    });
    let guest = fixture.peer("metadata-reader");
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    enroll(&mut service, &fixture.owner, &guest, "viewer");
    let before: i64 = service
        .db
        .query_row("SELECT COUNT(*) FROM hosted_requests", [], |r| r.get(0))
        .unwrap_or_else(|error| unreachable!("retained state unavailable: {error:?}"));
    for id in ["metadata-a", "metadata-b"] {
        let discovery = call(&mut service, &guest, id, "secrets.discovery", json!({}));
        assert_eq!(
            discovery.get("issuer").and_then(Value::as_str),
            Some("https://issuer.example.test")
        );
        let keys = call(
            &mut service,
            &guest,
            &format!("{id}-jwks"),
            "secrets.jwks",
            json!({}),
        );
        assert_eq!(
            keys.pointer("/keys/0/kty").and_then(Value::as_str),
            Some("OKP")
        );
    }
    let after: i64 = service
        .db
        .query_row("SELECT COUNT(*) FROM hosted_requests", [], |r| r.get(0))
        .unwrap_or_else(|error| unreachable!("retained state unavailable: {error:?}"));
    assert_eq!(
        after, before,
        "read-only metadata consumed effect replay capacity"
    );
    change_membership(
        &mut service,
        &fixture.owner,
        &guest,
        1,
        "viewer",
        &["alpha"],
        false,
    );
    rejected(service.handle(&guest, request("metadata-a", "secrets.jwks", json!({}))));
}

// r[verify kiln.hosted.accounts.scenario.membership]
#[test]
fn enrollment_receipt_failure_rolls_back_membership_and_invitation_consumption() {
    let fixture = Fixture::new();
    let guest = fixture.peer("atomic-guest");
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    let invitation = call(
        &mut service,
        &fixture.owner,
        "atomic-invite",
        "accounts.invite",
        json!({
            "peer": guest, "role": "viewer", "projects": ["alpha"],
            "expires_at_ms": kiln_hosted::now_ms().unwrap_or_else(|error| unreachable!("clock unavailable: {error:?}")) + 60_000,
            "nonce": "atomic-invitation"
        }),
    );
    let invitation_id = invitation
        .get("invitation")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("invitation identity missing"));
    let acceptance = request(
        "atomic-accept",
        "accounts.accept",
        json!({"invitation": invitation_id}),
    );
    service.db.execute_batch("CREATE TRIGGER deny_acceptance_audit BEFORE INSERT ON hosted_audit WHEN NEW.action='accounts.accept' BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;")
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    rejected(service.handle(&guest, acceptance.clone()));
    drop(service);
    let mut service = Service::open(fixture.profile.clone())
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    assert!(kiln_hosted::accounts::authorize(&service.db, &guest, ORG).is_err());
    let consumed: i64 = service
        .db
        .query_row(
            "SELECT consumed FROM hosted_invitations WHERE id=?1",
            [invitation_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|error| unreachable!("retained invitation unavailable: {error:?}"));
    assert_eq!(consumed, 0);
    let retained: i64 = service
        .db
        .query_row(
            "SELECT COUNT(*) FROM hosted_requests WHERE request_id='atomic-accept'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|error| unreachable!("retained request unavailable: {error:?}"));
    assert_eq!(retained, 0);
    service
        .db
        .execute_batch("DROP TRIGGER deny_acceptance_audit")
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    let original = accepted(service.handle(&guest, acceptance.clone()));
    assert_eq!(original.get("revision").and_then(Value::as_u64), Some(1));
    drop(service);
    let mut service = Service::open(fixture.profile)
        .unwrap_or_else(|error| unreachable!("journey prerequisite failed: {error:?}"));
    assert_eq!(accepted(service.handle(&guest, acceptance)), original);
    assert_eq!(
        kiln_hosted::accounts::authorize(&service.db, &guest, ORG)
            .unwrap_or_else(|error| unreachable!("accepted membership unavailable: {error:?}"))
            .role,
        "viewer"
    );
}
