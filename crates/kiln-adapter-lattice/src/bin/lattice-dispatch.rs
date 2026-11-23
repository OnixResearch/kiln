//! Dispatch one operator-admitted Lattice implementation request under a
//! reserved implementation attempt.

#[cfg(target_os = "linux")]
use kiln_adapter_lattice::worker;

#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use kiln_adapter_lattice::{
    UnixWorkflowExchange, admit_implementation_request, dispatch_implementation_request, lineage,
    recover_implementation_request,
};
#[cfg(target_os = "linux")]
use kiln_core::implementation::{AttemptPhase, Enrollment, ImplementationDispatchPlan};
#[cfg(target_os = "linux")]
use kiln_core::types::ExternalRef;
#[cfg(target_os = "linux")]
use kiln_implementer::Implementer;
#[cfg(target_os = "linux")]
use lattice_workflow_contract::{CapabilityProfile, WorkflowRequest};
#[cfg(target_os = "linux")]
use lattice_workflow_transport::TransportProfile;

#[cfg(target_os = "linux")]
const EXIT_ERROR: i32 = 2;

#[cfg(target_os = "linux")]
fn main() {
    if run().is_err() {
        std::process::exit(EXIT_ERROR);
    }
}

#[cfg(target_os = "linux")]
fn run() -> Result<(), ()> {
    let flags = match parse_flags(std::env::args().skip(1).collect()) {
        Ok(flags) => flags,
        Err(error) => return fail(&error),
    };
    if flags.contains_key("--worker-request") {
        return match worker::run(&flags) {
            Ok(observation) => {
                println!("{observation}");
                Ok(())
            }
            Err(error) => fail(&error),
        };
    }
    let database = match required_path(&flags, "--database") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let enrollment_file = match required_path(&flags, "--enrollment") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let dispatch_file = match required_path(&flags, "--dispatch-plan") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let request_file = match required_path(&flags, "--request") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let profile_file = match required_path(&flags, "--lattice-profile") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let transport_file = match required_path(&flags, "--transport-profile") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let socket = match required_path(&flags, "--socket") {
        Ok(socket) => socket,
        Err(error) => return fail(&error),
    };
    let maximum_records = match required_positive(&flags, "--maximum-records") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let dispatch: ImplementationDispatchPlan = match read_json(&dispatch_file) {
        Ok(dispatch) => dispatch,
        Err(error) => return fail(&error),
    };
    let (request, request_bytes): (WorkflowRequest, Vec<u8>) =
        match read_json_with_bytes(&request_file) {
            Ok(request) => request,
            Err(error) => return fail(&error),
        };
    let profile: CapabilityProfile = match read_json(&profile_file) {
        Ok(profile) => profile,
        Err(error) => return fail(&error),
    };
    let transport: TransportProfile = match read_json(&transport_file) {
        Ok(transport) => transport,
        Err(error) => return fail(&error),
    };
    let enrollment: Enrollment = match read_json(&enrollment_file) {
        Ok(enrollment) => enrollment,
        Err(error) => return fail(&error),
    };
    let implementer = Implementer::open_with_authority(
        &database,
        enrollment,
        flags.get("--local-operator").map(String::as_str),
        flags.get("--authority-config").map(std::path::Path::new),
    );
    let mut implementer = match implementer {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let work = match implementer.work() {
        Ok(work) => work,
        Err(error) => return fail(&error),
    };
    let Some(work) = work
        .current()
        .find(|work| {
            work.identity()
                .is_ok_and(|identity| identity == dispatch.work_identity)
        })
        .cloned()
    else {
        return fail("dispatch work identity is not current");
    };
    let attempt = match implementer.attempt(&dispatch.attempt_identity) {
        Ok(Some(attempt)) => attempt,
        Ok(None) => return fail("dispatch attempt is not retained"),
        Err(error) => return fail(&error),
    };
    if !matches!(
        attempt.phase,
        AttemptPhase::Claimed | AttemptPhase::Executing
    ) {
        return fail("dispatch attempt is neither claimed nor executing");
    }
    let attempts = match implementer.attempts() {
        Ok(attempts) => attempts,
        Err(error) => return fail(&error),
    };
    if !attempts.is_current(&attempt.identity, attempt.fence, wall_clock_ms()) {
        return fail("dispatch attempt is stale or its lease expired");
    }
    let Some(reservation) = attempt.dispatch.as_ref() else {
        return fail("dispatch attempt has no reservation");
    };
    let digest = match kiln_core::implementation::admit_dispatch_plan(&work, &attempt, &dispatch) {
        Ok(digest) => digest,
        Err(error) => return fail(&format!("{error:?}")),
    };
    if digest != reservation.plan {
        return fail("dispatch plan differs from the attempt reservation");
    }
    // Recovery only resumes a persisted transport intent; it never creates one.
    // Decided before any write, so a refused `--recover` records nothing and a
    // fresh attempt always passes the owner-grant gate below.
    let previous = match implementer.dispatch_requires_recovery(&attempt.identity) {
        Ok(previous) => previous,
        Err(error) => return fail(&error),
    };
    let recover = match exchange_recovery(flags.get("--recover"), previous) {
        Ok(recover) => recover,
        Err(error) => return fail(error),
    };
    let request_binding = match implementer.bind_campaign_request(
        &work,
        &attempt.identity,
        attempt.fence,
        wall_clock_ms(),
        &request_bytes,
    ) {
        Ok(binding) => binding,
        Err(error) => return fail(&error),
    };
    if request_binding.plan != digest {
        return fail("retained request seal differs from the exact dispatch plan");
    }
    if let Err(error) = admit_implementation_request(&dispatch, reservation, &attempt, &request) {
        return fail(&format!("{error:?}"));
    }
    let mut exchange =
        match UnixWorkflowExchange::new(socket, transport, profile.clone(), maximum_records) {
            Ok(exchange) => exchange,
            Err(error) => return fail(&format!("{error:?}")),
        };
    if !recover {
        let admission = match implementer.admission_snapshot() {
            Ok(value) => value,
            Err(error) => return fail(&error),
        };
        if admission.policy.is_some() {
            let Some(item) = admission.items.get(&dispatch.work_identity) else {
                return fail("queued dispatch work is missing");
            };
            if wall_clock_ms()
                .checked_add(u64::from(dispatch.deadline_minutes) * 60 * 1000)
                .is_none_or(|end| end > item.deadline_ms)
            {
                return fail("dispatch runtime exceeds its parent queue deadline");
            }
        }
        if let Err(error) = exchange_lineage_check(
            &admission,
            &dispatch.work_identity,
            &attempt.identity,
            attempt.fence,
            wall_clock_ms(),
            &request_bytes,
        ) {
            return fail(&error);
        }
        let fresh: Vec<kiln_core::fair_admission::EligibilityInput> =
            match flags.get("--admission-observations") {
                Some(path) => match read_json(Path::new(path)) {
                    Ok(fresh) => fresh,
                    Err(error) => return fail(&error),
                },
                None => Vec::new(),
            };
        if let Err(error) = implementer.admission_before_transport(
            &attempt.identity,
            attempt.fence,
            &fresh,
            wall_clock_ms(),
        ) {
            return fail(&error);
        }
    }
    // The workflow-exchange route has no CLI program or state root to record.
    let recover = match implementer.begin_dispatch_transport(
        &attempt.identity,
        attempt.fence,
        wall_clock_ms(),
        &reservation.plan,
        None,
    ) {
        Ok(start) => recover || start.is_recovery(),
        Err(error) => return fail(&error),
    };
    let accepted = if recover {
        recover_implementation_request(&mut exchange, &profile, &request)
    } else {
        dispatch_implementation_request(&mut exchange, &profile, &request)
    };
    let accepted = match accepted {
        Ok(accepted) => accepted,
        Err(error) => return fail(&format!("{error:?}")),
    };
    let workflow =
        match ExternalRef::new("lattice", "workflow", accepted.execution_identity.as_str()) {
            Ok(workflow) => workflow,
            Err(_) => return fail("accepted Lattice execution identity is malformed"),
        };
    if let Err(error) =
        implementer.start_execution(&attempt.identity, attempt.fence, wall_clock_ms(), &workflow)
    {
        return fail(&error);
    }
    if let Err(error) =
        implementer.admission_execution_observed(&attempt.identity, attempt.fence, wall_clock_ms())
    {
        return fail(&error);
    }
    println!(
        "{{\"accepted\":\"{}\"}}",
        accepted.execution_identity.as_str()
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn parse_flags(values: Vec<String>) -> Result<BTreeMap<String, String>, String> {
    let mut flags = BTreeMap::new();
    let mut values = values.into_iter();
    while let Some(flag) = values.next() {
        if !flag.starts_with("--") {
            return Err(format!("unexpected argument {flag}"));
        }
        let value = values
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        if flags.insert(flag.clone(), value).is_some() {
            return Err(format!("{flag} is repeated"));
        }
    }
    Ok(flags)
}

#[cfg(target_os = "linux")]
fn required_path(flags: &BTreeMap<String, String>, key: &str) -> Result<PathBuf, String> {
    let value = flags.get(key).ok_or_else(|| format!("{key} is required"))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
fn required_positive(flags: &BTreeMap<String, String>, key: &str) -> Result<usize, String> {
    let value = flags.get(key).ok_or_else(|| format!("{key} is required"))?;
    value
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{key} must be positive"))
}

#[cfg(target_os = "linux")]
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    use std::io::Read;
    let file =
        std::fs::File::open(path).map_err(|_| format!("{} is unreadable", path.display()))?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "bounded input read failed".to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("JSON input exceeds 16MiB admission bound".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| format!("{} is not valid JSON", path.display()))
}

#[cfg(target_os = "linux")]
fn read_json_with_bytes<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<(T, Vec<u8>), String> {
    use std::io::Read;
    let file =
        std::fs::File::open(path).map_err(|_| format!("{} is unreadable", path.display()))?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "bounded input read failed".to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("JSON input exceeds 16MiB admission bound".into());
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|_| format!("{} is not valid JSON", path.display()))?;
    Ok((value, bytes))
}

#[cfg(target_os = "linux")]
fn wall_clock_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn fail(message: &str) -> Result<(), ()> {
    eprintln!("kiln-lattice-dispatch: {message}");
    Err(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("kiln-lattice-dispatch is supported only on Linux");
    std::process::exit(2);
}

// r[impl kiln.admission.host_owner]
/// `--recover` resumes only a persisted transport intent, as the worker route
/// does: without one it is refused, so a fresh attempt always passes the
/// owner-grant gate before its transport start is recorded.
#[cfg(target_os = "linux")]
fn exchange_recovery(flag: Option<&String>, previous: bool) -> Result<bool, &'static str> {
    let requested = flag.is_some_and(|value| matches!(value.as_str(), "true" | "1"));
    if requested && !previous {
        return Err("recovery requires prior persisted transport intent");
    }
    Ok(requested || previous)
}

// r[impl kiln.admission.lattice_lineage.scenario.exchange]
/// A `lattice.workflow-request` has no lineage field, so this route never
/// carries one: it derives the lineage with the worker route's function and
/// refuses an inconsistent one before the transport start, and the request
/// bytes stay exactly as admitted.
#[cfg(target_os = "linux")]
fn exchange_lineage_check(
    admission: &kiln_core::fair_admission::AdmissionLedger,
    work: &kiln_core::types::ContentId,
    attempt: &kiln_core::types::ContentId,
    fence: u64,
    now_ms: u64,
    request_bytes: &[u8],
) -> Result<(), String> {
    let claim = kiln_core::fair_admission::ClaimRef {
        attempt: attempt.clone(),
        fence,
    };
    let sent = lineage::dispatched_request(
        lineage::Route::Exchange,
        admission,
        work,
        &claim,
        now_ms,
        request_bytes,
    )
    .map_err(|refusal| refusal.to_string())?;
    if *sent != *request_bytes {
        return Err("the exchange route changed its request bytes; nothing was started".into());
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{exchange_lineage_check, exchange_recovery};
    use kiln_core::fair_admission::{
        AdmissionLedger, AdmissionPolicy, ChargeKind, ClaimRef, OwnerReservation, ReservationState,
    };
    use kiln_core::types::{ContentId, ProviderProfileId};

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    // r[verify kiln.admission.lattice_lineage.scenario.exchange]
    #[test]
    fn the_exchange_route_checks_the_lineage_and_keeps_its_bytes() {
        let request = br#"{"schema":"lattice.workflow-request.v1"}"#;
        // Without admission there is nothing to check.
        assert_eq!(
            exchange_lineage_check(
                &AdmissionLedger::default(),
                &content('a'),
                &content('b'),
                1,
                1,
                request
            ),
            Ok(())
        );
        // An admitted attempt whose owner has not answered is refused.
        let mut ledger = AdmissionLedger {
            policy: Some(AdmissionPolicy {
                version: 1,
                accounting_basis: content('c'),
                max_items: 1,
                max_bytes: 1,
                max_wait_ms: 1,
                max_observation_age_ms: 1,
                max_retry_after_ms: 1,
                max_children: 1,
                max_active: 1,
                strategy: kiln_core::operations::FairnessStrategy::DominantShare,
                shares: std::collections::BTreeMap::new(),
                group_limits: Vec::new(),
                serving: None,
            }),
            ..AdmissionLedger::default()
        };
        ledger.reservations.insert(
            content('d'),
            OwnerReservation {
                version: 1,
                operation: content('d'),
                provider: ProviderProfileId::new("owner").unwrap_or_else(|_| unreachable!()),
                generation: 1,
                work_version: content('a'),
                claim: ClaimRef {
                    attempt: content('b'),
                    fence: 1,
                },
                account: content('c'),
                kind: ChargeKind::Execution,
                subject: content('e'),
                accounting_basis: content('c'),
                serving_bounds: None,
                amounts: std::collections::BTreeMap::new(),
                expires_at_ms: 2,
                state: ReservationState::Tentative,
                receipt: None,
            },
        );
        let refused = exchange_lineage_check(&ledger, &content('a'), &content('b'), 1, 1, request);
        assert!(
            refused
                .as_ref()
                .is_err_and(|error| error.contains("(reservation_unsettled)")),
            "{refused:?}"
        );
    }

    // r[verify kiln.admission.host_owner]
    #[test]
    fn recover_without_a_persisted_transport_start_is_refused() {
        let flag = "true".to_string();
        assert_eq!(
            exchange_recovery(Some(&flag), false),
            Err("recovery requires prior persisted transport intent")
        );
        // After a real in-band crash the persisted start drives recovery,
        // with or without the explicit flag.
        assert_eq!(exchange_recovery(Some(&flag), true), Ok(true));
        assert_eq!(exchange_recovery(None, true), Ok(true));
        // A fresh dispatch goes through the owner-grant gate.
        assert_eq!(exchange_recovery(None, false), Ok(false));
        assert_eq!(
            exchange_recovery(Some(&"false".to_string()), false),
            Ok(false)
        );
    }
}
