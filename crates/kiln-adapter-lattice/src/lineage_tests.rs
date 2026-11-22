// r[verify kiln.admission.lattice_lineage]
//! The budget lineage Kiln builds into Lattice requests: golden bytes,
//! acceptance by Lattice's published schema, and refusals before transport.

use super::{
    Backpressure, Blocker, BudgetLineage, CHARGED_DIMENSIONS, LATTICE_REVISION, LINEAGE_SCHEMA,
    LineageRefusal, Route, admitting_gate, derive, dispatched_request, verify_frozen,
};
use kiln_core::fair_admission::{
    AdmissionLedger, AdmissionPhase, AdmissionReason, Amounts, BudgetAccount, ChargeKind, ClaimRef,
    HostOwnerProfile, OwnerReply, OwnerReservation, QueueItem, ReservationState,
    ServingAdmissionPolicy, ServingCapacityRule, ServingExplanation,
};
use kiln_core::types::{Alias, ContentId, ProviderProfileId};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::BTreeMap;

const NOW: u64 = 1_800_000_000_000;
const ADMITTED: &[u8] =
    include_bytes!("../../../fixtures/lattice/kiln-budget-lineage/admitted-request.json");
const DISPATCHED: &[u8] =
    include_bytes!("../../../fixtures/lattice/kiln-budget-lineage/dispatched-request.json");
const DISPATCHED_NESTED: &[u8] =
    include_bytes!("../../../fixtures/lattice/kiln-budget-lineage/dispatched-nested-request.json");
const SCHEMA: &str = include_str!(
    "../../../fixtures/lattice/kiln-budget-lineage/5a8e1d51c2e36100424b761c4fef65e6cd4c68dc/schema.json"
);
const HOST_OWNER: &str = include_str!("../../../fixtures/fair-admission/host-owner.json");

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn alias(value: &str) -> Alias {
    Alias::new(value).unwrap_or_else(|_| unreachable!())
}

fn amounts(entries: &[(&str, u64)]) -> Amounts {
    entries
        .iter()
        .map(|(name, amount)| (alias(name), *amount))
        .collect()
}

fn profile() -> HostOwnerProfile {
    serde_json::from_str(HOST_OWNER).unwrap_or_else(|error| unreachable!("{error}"))
}

fn work() -> ContentId {
    content('a')
}

fn claim() -> ClaimRef {
    ClaimRef {
        attempt: content('b'),
        fence: 1,
    }
}

fn operation() -> ContentId {
    content('c')
}

/// The host-owner registration's ledger with one claimed queue row on
/// `account` whose work budget is `budget`, and no reservation yet.
fn queued(profile: &HostOwnerProfile, account: &ContentId, budget: Amounts) -> AdmissionLedger {
    let mut ledger = AdmissionLedger::default();
    let policy = profile.policy().unwrap_or_else(|_| unreachable!());
    ledger
        .configure(policy)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    ledger
        .add_root(profile.root_account().unwrap_or_else(|_| unreachable!()))
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    ledger.items.insert(
        work(),
        QueueItem {
            work_version: work(),
            account: account.clone(),
            tenant: profile.tenant.clone(),
            campaign: profile.campaign.clone(),
            repository: alias("onixresearch/implementer-sandbox"),
            priority: 1,
            bytes: 1,
            enqueued_at_ms: NOW,
            deadline_ms: profile.deadline_ms,
            eligible_at_ms: NOW,
            claim: Some(claim()),
            phase: AdmissionPhase::PostClaim,
            reason: AdmissionReason::Eligible,
            order: 0,
            required_owners: BTreeMap::from([(
                profile.owner.provider.clone(),
                ChargeKind::Execution,
            )]),
            shared_allocations: Vec::new(),
            budget,
        },
    );
    ledger
}

/// Persist the tentative request for `amounts`, as `host-owner-reserve` does.
fn request(
    ledger: &mut AdmissionLedger,
    profile: &HostOwnerProfile,
    account: &ContentId,
    amounts: Amounts,
) {
    ledger
        .request(
            OwnerReservation {
                version: 1,
                operation: operation(),
                provider: profile.owner.provider.clone(),
                generation: profile.owner.generation,
                work_version: work(),
                claim: claim(),
                account: account.clone(),
                kind: ChargeKind::Execution,
                subject: content('f'),
                accounting_basis: profile
                    .accounting_basis()
                    .unwrap_or_else(|_| unreachable!()),
                serving_bounds: None,
                amounts,
                expires_at_ms: NOW + 3_600_000,
                state: ReservationState::Tentative,
                receipt: None,
            },
            NOW,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
}

fn commit(ledger: &mut AdmissionLedger, profile: &HostOwnerProfile) {
    ledger
        .owner_reply(
            &OwnerReply {
                version: 1,
                operation: operation(),
                provider: profile.owner.provider.clone(),
                generation: profile.owner.generation,
                receipt: operation(),
                state: ReservationState::Committed,
                retry_at_ms: None,
                serving_bounds: None,
            },
            NOW,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
}

/// The registered host owner's campaign root with one committed per-work
/// reservation: the fixture `fixtures/fair-admission/host-owner.json` drives.
fn host_owner_reserved() -> AdmissionLedger {
    let profile = profile();
    let root = profile
        .account_identity()
        .unwrap_or_else(|_| unreachable!());
    let mut ledger = queued(&profile, &root, profile.per_work.clone());
    request(&mut ledger, &profile, &root, profile.per_work.clone());
    commit(&mut ledger, &profile);
    ledger
}

const NESTED: [(&str, u64); 3] = [
    ("gpu-seconds", 30),
    ("model_requests", 4),
    ("tool_invocations", 8),
];

/// A work account one level below the campaign root, holding Lattice's
/// charged dimensions and one it cannot charge.
fn nested_reserved(account: &ContentId) -> AdmissionLedger {
    let mut profile = profile();
    profile.quota = amounts(&[
        ("gpu-seconds", 100),
        ("model_requests", 10),
        ("tool_invocations", 20),
    ]);
    profile.per_work = amounts(&NESTED);
    let root = profile
        .account_identity()
        .unwrap_or_else(|_| unreachable!());
    let mut ledger = queued(&profile, account, amounts(&NESTED));
    ledger.accounts.insert(
        account.clone(),
        BudgetAccount {
            identity: account.clone(),
            parent: Some(root),
            parent_attempt: None,
            campaign: profile.campaign.clone(),
            tenant: profile.tenant.clone(),
            deadline_ms: profile.deadline_ms,
            available: amounts(&NESTED),
            consumed: Amounts::new(),
            children: 0,
        },
    );
    request(&mut ledger, &profile, account, amounts(&NESTED));
    commit(&mut ledger, &profile);
    ledger
}

fn worker(ledger: &AdmissionLedger) -> Result<Cow<'static, [u8]>, LineageRefusal> {
    dispatched_request(Route::Worker, ledger, &work(), &claim(), NOW, ADMITTED)
}

fn emitted(bytes: &[u8]) -> Value {
    serde_json::from_slice::<Value>(bytes)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .get("lineage")
        .cloned()
        .unwrap_or_else(|| unreachable!("no lineage"))
}

#[test]
fn requests_without_admission_keep_their_bytes() {
    let unconfigured = AdmissionLedger::default();
    let unreserved = queued(&profile(), &content('9'), profile().per_work);
    for ledger in [&unconfigured, &unreserved] {
        for route in [Route::Worker, Route::Exchange] {
            let bytes = dispatched_request(route, ledger, &work(), &claim(), NOW, ADMITTED);
            assert!(matches!(bytes, Ok(Cow::Borrowed(sent)) if std::ptr::eq(sent, ADMITTED)));
        }
    }
}

// r[verify kiln.admission.lattice_lineage.scenario.carried]
#[test]
fn the_worker_request_carries_the_golden_lineage() {
    let bytes = worker(&host_owner_reserved()).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        String::from_utf8_lossy(&bytes),
        String::from_utf8_lossy(DISPATCHED),
        "golden dispatched request"
    );
    // Every admitted byte is kept, in order, around one inserted member.
    let end = ADMITTED.len() - 3;
    assert_eq!(bytes.get(..end), ADMITTED.get(..end));
    assert_eq!(bytes.get(bytes.len() - 3..), ADMITTED.get(end..));
    let root = profile()
        .account_identity()
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        emitted(&bytes),
        json!({"schema": LINEAGE_SCHEMA, "root": root.as_str(), "account": root.as_str(),
               "parent": null, "depth": 0, "remaining": {"worker_slots": 1},
               "blocker": null, "backpressure": "none"})
    );
    assert_eq!(
        verify_frozen(ADMITTED, &bytes).map(|lineage| lineage.is_some()),
        Ok(true)
    );
    // Compact and empty objects take the same one member.
    let snapshot = serde_json::from_value::<BudgetLineage>(emitted(&bytes))
        .ok()
        .and_then(|lineage| serde_json::to_string(&lineage).ok())
        .unwrap_or_default();
    let ledger = host_owner_reserved();
    for (admitted, expected) in [
        (
            &br#"{"schema":"x"}"#[..],
            format!(r#"{{"schema":"x","lineage":{snapshot}}}"#),
        ),
        (&b"{ }\n"[..], format!("{{\"lineage\":{snapshot} }}\n")),
    ] {
        let sent = dispatched_request(Route::Worker, &ledger, &work(), &claim(), NOW, admitted)
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(String::from_utf8_lossy(&sent), expected);
        assert_eq!(
            verify_frozen(admitted, &sent).map(|lineage| lineage.is_some()),
            Ok(true)
        );
    }
}

// r[verify kiln.admission.lattice_lineage.scenario.exchange]
#[test]
fn the_exchange_route_checks_the_lineage_and_keeps_its_bytes() {
    let ledger = host_owner_reserved();
    let bytes = dispatched_request(Route::Exchange, &ledger, &work(), &claim(), NOW, ADMITTED);
    assert!(matches!(bytes, Ok(Cow::Borrowed(sent)) if std::ptr::eq(sent, ADMITTED)));
    let mut revoked = host_owner_reserved();
    let root = profile()
        .account_identity()
        .unwrap_or_else(|_| unreachable!());
    revoked
        .revoke_account(&root)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    for route in [Route::Exchange, Route::Worker] {
        assert_eq!(
            dispatched_request(route, &revoked, &work(), &claim(), NOW, ADMITTED),
            Err(LineageRefusal::Blocked {
                blocker: Blocker::Denied
            })
        );
    }
}

/// Lattice's published schema, read as data: the structural rules of
/// `Binding::validate`, then the pre-fence gate of a first step reserving
/// `next` charged units. Returns the unaccounted dimensions.
fn lattice_accepts(
    schema: &Value,
    lineage: &Value,
    next: u64,
) -> Result<Vec<String>, &'static str> {
    let field = |name: &str| schema.get(name).unwrap_or_else(|| unreachable!("{name}"));
    let strings = |name: &str| -> Vec<&str> {
        field(name)
            .as_array()
            .unwrap_or_else(|| unreachable!())
            .iter()
            .filter_map(Value::as_str)
            .collect()
    };
    let number = |name: &str| field(name).as_u64().unwrap_or_else(|| unreachable!());
    let object = lineage.as_object().ok_or("schema")?;
    if object
        .keys()
        .map(String::as_str)
        .ne(sorted(strings("fields")))
    {
        return Err("fields");
    }
    if lineage.get("schema").and_then(Value::as_str) != field("schema").as_str() {
        return Err("schema");
    }
    let digits = field("identity").get("hex_digits").and_then(Value::as_u64);
    let alphabet = field("identity")
        .get("alphabet")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!());
    let identity = |value: Option<&str>| {
        value.is_some_and(|id| {
            Some(id.len() as u64) == digits && id.chars().all(|digit| alphabet.contains(digit))
        })
    };
    let root = lineage.get("root").and_then(Value::as_str);
    let account = lineage.get("account").and_then(Value::as_str);
    let parent = lineage.get("parent").ok_or("fields")?;
    if !identity(root) || !identity(account) || !(parent.is_null() || identity(parent.as_str())) {
        return Err("identity");
    }
    let depth = lineage
        .get("depth")
        .and_then(Value::as_u64)
        .ok_or("shape")?;
    let shaped = match parent.as_str() {
        None => depth == 0 && account == root,
        Some(parent) => {
            depth > 0 && depth <= number("max_depth") && Some(parent) != account && account != root
        }
    };
    if !shaped {
        return Err("shape");
    }
    let remaining = lineage
        .get("remaining")
        .and_then(Value::as_object)
        .ok_or("dimensions")?;
    if remaining.is_empty()
        || remaining.len() as u64 > number("max_dimensions")
        || remaining.iter().any(|(name, amount)| {
            name.is_empty()
                || name.len() as u64 > number("max_dimension_name_bytes")
                || amount.as_u64().is_none()
        })
    {
        return Err("dimensions");
    }
    let blocker = lineage.get("blocker").ok_or("fields")?;
    let gate = lineage
        .get("backpressure")
        .and_then(Value::as_str)
        .ok_or("fields")?;
    if !(blocker.is_null()
        || blocker
            .as_str()
            .is_some_and(|b| strings("blockers").contains(&b)))
        || !strings("backpressure").contains(&gate)
    {
        return Err("enum");
    }
    if !blocker.is_null() {
        return Err("lineage_blocked");
    }
    if !strings("admitting_backpressure").contains(&gate) {
        return Err("lineage_blocked");
    }
    let charged = strings("charged_dimensions");
    for (name, amount) in remaining {
        let needed = if charged.contains(&name.as_str()) {
            next.max(1)
        } else {
            1
        };
        if amount.as_u64().is_none_or(|amount| amount < needed) {
            return Err("lineage_exhausted");
        }
    }
    Ok(remaining
        .keys()
        .filter(|name| !charged.contains(&name.as_str()))
        .cloned()
        .collect())
}

fn sorted(mut fields: Vec<&str>) -> std::vec::IntoIter<&str> {
    // `serde_json::Value` objects iterate in key order.
    fields.sort_unstable();
    fields.into_iter()
}

// r[verify kiln.admission.lattice_lineage.scenario.carried]
#[test]
fn lattice_published_schema_accepts_every_emitted_lineage() {
    let schema: Value =
        serde_json::from_str(SCHEMA).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        schema.pointer("/source/revision").and_then(Value::as_str),
        Some(LATTICE_REVISION)
    );
    assert_eq!(
        schema.get("schema").and_then(Value::as_str),
        Some(LINEAGE_SCHEMA)
    );
    assert_eq!(
        schema.get("charged_dimensions"),
        Some(&json!(CHARGED_DIMENSIONS)),
        "charged dimension names"
    );
    // Lattice's own accepted snapshots decode into Kiln's type and back unchanged.
    for accepted in schema
        .get("accepted")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let decoded: BudgetLineage = serde_json::from_value(accepted.clone())
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(decoded.validate(), Ok(()));
        assert_eq!(serde_json::to_value(&decoded).ok().as_ref(), Some(accepted));
        assert!(lattice_accepts(&schema, accepted, 1).is_ok());
    }
    // Lattice's malformed snapshots are refused by both readings.
    let base = schema
        .pointer("/accepted/0")
        .cloned()
        .unwrap_or_else(|| unreachable!());
    for case in schema
        .get("malformed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let mut changed = base.clone();
        for (name, value) in case
            .get("change")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            if let Some(object) = changed.as_object_mut() {
                object.insert(name.clone(), value.clone());
            }
        }
        let expected = case.get("error").and_then(Value::as_str);
        assert_eq!(
            lattice_accepts(&schema, &changed, 1).err(),
            expected,
            "{case}"
        );
        let decoded: BudgetLineage =
            serde_json::from_value(changed).unwrap_or_else(|error| unreachable!("{error}"));
        assert!(decoded.validate().is_err(), "{case}");
    }
    // The emitted host-owner root lineage: `worker_slots` is unaccounted.
    let host = worker(&host_owner_reserved()).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        lattice_accepts(&schema, &emitted(&host), 1),
        Ok(vec!["worker_slots".to_owned()])
    );
    // A nested work account: charged dimensions are drawn, `gpu-seconds` is not.
    let nested = nested_reserved(&content('2'));
    let bytes = worker(&nested).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        String::from_utf8_lossy(&bytes),
        String::from_utf8_lossy(DISPATCHED_NESTED),
        "golden nested request"
    );
    let lineage = emitted(&bytes);
    assert_eq!(lineage.get("depth"), Some(&json!(1)));
    assert_eq!(lineage.get("account"), Some(&json!(content('2').as_str())));
    assert_eq!(
        lineage.get("remaining"),
        Some(&json!({"gpu-seconds": 30, "model_requests": 4, "tool_invocations": 8}))
    );
    assert_eq!(
        lattice_accepts(&schema, &lineage, 4),
        Ok(vec!["gpu-seconds".to_owned()])
    );
    assert_eq!(
        lattice_accepts(&schema, &lineage, 9),
        Err("lineage_exhausted")
    );
    let decoded: BudgetLineage =
        serde_json::from_value(lineage).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(decoded.unaccounted(), vec!["gpu-seconds"]);
}

// r[verify kiln.admission.lattice_lineage.scenario.refused]
#[test]
fn an_inconsistent_lineage_is_refused_before_transport() {
    let profile = profile();
    let root = profile
        .account_identity()
        .unwrap_or_else(|_| unreachable!());
    // An owner that has not answered.
    let mut tentative = queued(&profile, &root, profile.per_work.clone());
    request(&mut tentative, &profile, &root, profile.per_work.clone());
    let unsettled = Err(LineageRefusal::ReservationUnsettled {
        operation: operation().as_str().to_owned(),
    });
    assert_eq!(worker(&tentative), unsettled);
    // A grant already moved to unknown by a dispatch intent.
    let mut intended = host_owner_reserved();
    let policy = intended.policy_ref().unwrap_or_else(|_| unreachable!());
    intended
        .dispatch_intent(
            &operation(),
            &policy,
            &claim(),
            profile.owner.generation,
            NOW,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    assert_eq!(worker(&intended), unsettled);
    // A grant drawing from another account than the queued work's.
    let mut crossed = host_owner_reserved();
    if let Some(reservation) = crossed.reservations.get_mut(&operation()) {
        reservation.account = content('7');
    }
    assert_eq!(
        worker(&crossed),
        Err(LineageRefusal::CrossedReservation {
            operation: operation().as_str().to_owned()
        })
    );
    // The account tracks a dimension the attempt holds nothing of.
    let mut omitted = host_owner_reserved();
    if let Some(account) = omitted.accounts.get_mut(&root) {
        account.available.insert(alias("model_requests"), 3);
    }
    assert_eq!(
        worker(&omitted),
        Err(LineageRefusal::Exhausted {
            dimension: "model_requests".to_owned()
        })
    );
    // A grant beyond the work's own budget.
    let mut overdrawn = host_owner_reserved();
    if let Some(item) = overdrawn.items.get_mut(&work()) {
        item.budget = amounts(&[("worker_slots", 0)]);
    }
    assert!(inconsistent(
        &derive(&overdrawn, &work(), &claim(), NOW),
        "exceeds the work budget"
    ));
    // A queue row claimed by another attempt.
    let mut reclaimed = host_owner_reserved();
    if let Some(item) = reclaimed.items.get_mut(&work()) {
        item.claim = Some(ClaimRef {
            attempt: content('8'),
            fence: 2,
        });
    }
    assert!(matches!(
        worker(&reclaimed),
        Err(LineageRefusal::LedgerInconsistent { .. })
    ));
    // An ancestor deadline already reached.
    let expired = host_owner_reserved();
    assert_eq!(
        dispatched_request(
            Route::Worker,
            &expired,
            &work(),
            &claim(),
            profile.deadline_ms,
            ADMITTED
        ),
        Err(LineageRefusal::Blocked {
            blocker: Blocker::Expired
        })
    );
    // An identity Lattice cannot read: Kiln never lowercases it for Lattice.
    let upper = ContentId::from_blake3_hex("A".repeat(64)).unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        worker(&nested_reserved(&upper)),
        Err(LineageRefusal::Unrepresentable { .. })
    ));
    // Only Kiln derives the lineage: an admitted request naming one is refused
    // even without admission.
    let named = br#"{"schema":"lattice.implementation-attempt-run.v3","lineage":null}"#;
    for ledger in [AdmissionLedger::default(), host_owner_reserved()] {
        assert_eq!(
            dispatched_request(Route::Worker, &ledger, &work(), &claim(), NOW, named),
            Err(LineageRefusal::RequestNamesLineage)
        );
    }
    assert!(matches!(
        dispatched_request(
            Route::Worker,
            &host_owner_reserved(),
            &work(),
            &claim(),
            NOW,
            b"[]"
        ),
        Err(LineageRefusal::Unrepresentable { .. })
    ));
    // Every refusal names its typed reason and that nothing started.
    let message = LineageRefusal::Blocked {
        blocker: Blocker::Denied,
    }
    .to_string();
    assert!(
        message.contains("(blocked)") && message.contains("nothing was started"),
        "{message}"
    );
}

// r[verify kiln.admission.lattice_lineage.scenario.carried]
#[test]
fn recovery_reuses_only_the_frozen_admitted_request_plus_a_lineage() {
    assert_eq!(verify_frozen(ADMITTED, ADMITTED), Ok(None));
    // The frozen lineage is exactly what the ledger derived before transport.
    let frozen =
        verify_frozen(ADMITTED, DISPATCHED).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        frozen,
        derive(&host_owner_reserved(), &work(), &claim(), NOW).unwrap_or_default()
    );
    assert!(frozen.is_some_and(|lineage| lineage.backpressure == Backpressure::None));
    let text = String::from_utf8_lossy(DISPATCHED);
    for tampered in [
        text.replace("\"depth\":0", "\"depth\":3"),
        text.replace("\"network\": false", "\"network\": true"),
        text.replace("\"lineage\":", "\"lineage\": "),
        text.replace(
            "\"backpressure\":\"none\"",
            "\"backpressure\":\"none\",\"extra\":1",
        ),
        String::from_utf8_lossy(ADMITTED).replace("false", "true"),
    ] {
        assert_eq!(
            verify_frozen(ADMITTED, tampered.as_bytes()),
            Err(LineageRefusal::FrozenRequestCrossed),
            "{tampered}"
        );
    }
}

fn lineage_of(ledger: &AdmissionLedger) -> Result<Option<BudgetLineage>, LineageRefusal> {
    derive(ledger, &work(), &claim(), NOW)
}

fn inconsistent(result: &Result<Option<BudgetLineage>, LineageRefusal>, detail: &str) -> bool {
    matches!(result, Err(LineageRefusal::LedgerInconsistent { detail: found }) if found.contains(detail))
}

// r[verify kiln.admission.lattice_lineage.scenario.carried]
#[test]
fn a_shared_allocation_dimension_the_attempt_does_not_hold_is_omitted() {
    // As in `shared_model_physical_charge_once_and_per_worker_requests_separate`:
    // the account also tracks a deployment's `gpu`, which this worker
    // references through `shared_allocations` and never reserves.
    let mut profile = profile();
    profile.quota = amounts(&[("gpu", 1), ("model_requests", 10)]);
    let root = profile
        .account_identity()
        .unwrap_or_else(|_| unreachable!());
    let held = amounts(&[("model_requests", 3)]);
    let mut ledger = queued(&profile, &root, held.clone());
    if let Some(item) = ledger.items.get_mut(&work()) {
        item.shared_allocations.push(content('5'));
    }
    request(&mut ledger, &profile, &root, held);
    commit(&mut ledger, &profile);
    let lineage = lineage_of(&ledger)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!("no lineage"));
    assert_eq!(
        lineage.remaining,
        BTreeMap::from([("model_requests".to_owned(), 3)])
    );
    assert!(lineage.unaccounted().is_empty());
    // The exchange route admits it too.
    let bytes = dispatched_request(Route::Exchange, &ledger, &work(), &claim(), NOW, ADMITTED);
    assert!(matches!(bytes, Ok(Cow::Borrowed(sent)) if std::ptr::eq(sent, ADMITTED)));
}

// r[verify kiln.admission.lattice_lineage.scenario.refused]
#[test]
fn every_derivation_bound_refuses_an_inconsistent_ledger() {
    let profile = profile();
    let root = profile
        .account_identity()
        .unwrap_or_else(|_| unreachable!());
    // A required owner without a committed grant for this attempt.
    let mut ungranted = host_owner_reserved();
    let second = ProviderProfileId::new("second-owner").unwrap_or_else(|_| unreachable!());
    if let Some(item) = ungranted.items.get_mut(&work()) {
        item.required_owners.insert(second, ChargeKind::Execution);
    }
    assert_eq!(
        lineage_of(&ungranted),
        Err(LineageRefusal::MissingOwnerGrant {
            provider: "second-owner".to_owned()
        })
    );
    // A held amount beyond the root allocation.
    let mut beyond_root = host_owner_reserved();
    beyond_root
        .quotas
        .insert(root.clone(), amounts(&[("worker_slots", 0)]));
    assert!(inconsistent(
        &lineage_of(&beyond_root),
        "exceeds the root allocation"
    ));
    // A root without its allocation.
    let mut unallocated = host_owner_reserved();
    unallocated.quotas.remove(&root);
    assert!(inconsistent(
        &lineage_of(&unallocated),
        "root allocation is missing"
    ));
    // A held dimension the account does not track.
    let mut untracked = host_owner_reserved();
    if let Some(reservation) = untracked.reservations.get_mut(&operation()) {
        reservation.amounts.insert(alias("gpu-seconds"), 1);
    }
    assert!(inconsistent(
        &lineage_of(&untracked),
        "not tracked by the account"
    ));
    // A nested account whose root disappeared.
    let mut rootless = nested_reserved(&content('2'));
    rootless.accounts.remove(&root);
    assert!(inconsistent(
        &lineage_of(&rootless),
        "lineage ancestor missing"
    ));
    // Held amounts that overflow.
    let mut overflowing = host_owner_reserved();
    let mut huge = overflowing
        .reservations
        .get(&operation())
        .cloned()
        .unwrap_or_else(|| unreachable!());
    huge.operation = content('6');
    huge.amounts = amounts(&[("worker_slots", u64::MAX)]);
    overflowing
        .reservations
        .insert(huge.operation.clone(), huge);
    assert!(inconsistent(&lineage_of(&overflowing), "overflow"));
}

// r[verify kiln.admission.lattice_lineage.scenario.carried]
#[test]
fn the_serving_gate_maps_to_backpressure_and_only_an_admitting_gate_passes() {
    // A worker-request owner whose grant is retained reads `eligible`.
    let mut serving = host_owner_reserved();
    let owner = ProviderProfileId::new("serving-owner").unwrap_or_else(|_| unreachable!());
    if let Some(policy) = serving.policy.as_mut() {
        policy.serving = Some(ServingAdmissionPolicy {
            max_snapshot_age_ms: 1_000,
            capacity_rule: ServingCapacityRule::OwnerAdmissionDecides,
        });
    }
    if let Some(item) = serving.items.get_mut(&work()) {
        item.required_owners
            .insert(owner.clone(), ChargeKind::WorkerRequest);
        item.budget = amounts(&[("worker_slots", 2)]);
    }
    let mut grant = serving
        .reservations
        .get(&operation())
        .cloned()
        .unwrap_or_else(|| unreachable!());
    grant.operation = content('7');
    grant.provider = owner.clone();
    grant.kind = ChargeKind::WorkerRequest;
    serving.reservations.insert(grant.operation.clone(), grant);
    let lineage = lineage_of(&serving)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!("no lineage"));
    assert_eq!(lineage.backpressure, Backpressure::Eligible);
    assert_eq!(
        lineage.remaining,
        BTreeMap::from([("worker_slots".to_owned(), 2)])
    );
    // Every gate that does not admit is refused; non-serving reasons are
    // never mapped.
    assert_eq!(admitting_gate(None), Ok(Backpressure::None));
    for (gate, mapped) in [
        (AdmissionReason::Waiting, Backpressure::Waiting),
        (AdmissionReason::Overloaded, Backpressure::Overloaded),
        (AdmissionReason::Stale, Backpressure::Stale),
        (AdmissionReason::Incomplete, Backpressure::Incomplete),
        (AdmissionReason::Incompatible, Backpressure::Incompatible),
    ] {
        let explanation = ServingExplanation {
            provider: owner.clone(),
            gate,
            observation: None,
        };
        assert_eq!(
            admitting_gate(Some(&explanation)),
            Err(LineageRefusal::Backpressure { gate: mapped })
        );
    }
    let unmapped = ServingExplanation {
        provider: owner,
        gate: AdmissionReason::Unknown,
        observation: None,
    };
    assert!(matches!(
        admitting_gate(Some(&unmapped)),
        Err(LineageRefusal::LedgerInconsistent { .. })
    ));
}
