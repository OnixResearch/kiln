//! Dispatch one operator-admitted Lattice implementation request under a
//! reserved implementation attempt.

#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use kiln_adapter_lattice::{
    UnixWorkflowExchange, admit_implementation_request, dispatch_implementation_request,
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
    let request: WorkflowRequest = match read_json(&request_file) {
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
    let mut implementer = match Implementer::open(&database, enrollment) {
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
    if attempt.phase != AttemptPhase::Claimed {
        return fail("dispatch attempt is not claimed");
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
    if let Err(error) = admit_implementation_request(&dispatch, reservation, &attempt, &request) {
        return fail(&format!("{error:?}"));
    }
    let mut exchange =
        match UnixWorkflowExchange::new(socket, transport, profile.clone(), maximum_records) {
            Ok(exchange) => exchange,
            Err(error) => return fail(&format!("{error:?}")),
        };
    let accepted = match dispatch_implementation_request(&mut exchange, &profile, &request) {
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
    let bytes = std::fs::read(path).map_err(|_| format!("{} is unreadable", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|_| format!("{} is not valid JSON", path.display()))
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
