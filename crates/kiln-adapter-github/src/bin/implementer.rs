//! Operator shell for the durable implementation ledger.
//!
//! `drain` moves authenticated queued GitHub issue deliveries into admitted
//! work once. `run` repeats that drain on a bounded interval until a stop signal
//! or an explicit iteration count. `status` reports retained work and attempt
//! counts. `claim`, `cancel`, `reserve` and `start` own the durable attempt and
//! dispatch-reservation transitions. Exact recovery, admission and candidate
//! capture use their owning native consumers; independent verification belongs
//! to the canonical web-service Engine, never an imported success report.
//! r[impl kiln.implementation.recovery_cli]
//! r[impl kiln.implementation.recovery_cli.scenario.distinct]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use kiln_adapter_github::implementation::{ImplementationIssuePolicy, drain_issue_queue};
use kiln_adapter_github::profile::ImplementationQueueConfig;
use kiln_core::implementation::Enrollment;
use kiln_implementer::Implementer;

const EXIT_ERROR: i32 = 2;
const MAX_INTERVAL_MILLISECONDS: usize = 24 * 60 * 60 * 1000;

struct DrainInputs {
    database: PathBuf,
    queue: PathBuf,
    secret: Vec<u8>,
    policy: ImplementationIssuePolicy,
    enrollment: Enrollment,
    maximum_deliveries: usize,
    limit: usize,
}

// r[impl kiln.implementation.operator_cli]
fn main() {
    if run().is_err() {
        std::process::exit(EXIT_ERROR);
    }
}

fn run() -> Result<(), ()> {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        return fail(
            "usage: kiln-implementer <drain|run|poll|poll-run|status|claim|cancel|renew|reserve|reserve-plan|start|reconcile|recover|reconcile-termination|handoff|capture-candidate|prepare-verification-profile|prepare-needs|attach-needs|needs|inspect-work|admission|admission-status|prepare-publication|publish|recover-integration|close-source|source-status|observe-cairn-lifecycle|metrics> --database PATH ...",
        );
    };
    let flags = match parse_flags(arguments.collect()) {
        Ok(flags) => flags,
        Err(error) => return fail(&error),
    };
    match command.as_str() {
        "drain" => drain(&flags),
        "run" => run_loop(&flags),
        "poll" => poll(&flags),
        "poll-run" => poll_run(&flags),
        "status" => status(&flags),
        "claim" => claim(&flags),
        "cancel" => cancel(&flags),
        "renew" => renew(&flags),
        "reserve" => reserve(&flags),
        "reserve-plan" => reserve_plan(&flags),
        "start" => start(&flags),
        "reconcile" => reconcile(&flags),
        "handoff" => handoff(&flags),
        "capture-candidate" => capture_candidate(&flags),
        "prepare-verification-profile" => prepare_verification_profile(&flags),
        "prepare-needs" => prepare_needs(&flags),
        "prepare-publication" => prepare_publication(&flags),
        "publish" => publish(&flags),
        "land" => fail(
            "unattended landing is unsupported: ordinary GitHub merge cannot atomically guard the target; remain PR-ready",
        ),
        "recover-integration" => recover_integration(&flags),
        "close-source" => close_source(&flags),
        "source-status" => source_status(&flags),
        "metrics" => metrics(&flags),
        "recover" => recover_attempts(&flags),
        "reconcile-termination" => reconcile_termination(&flags),
        "attach-needs" => attach_needs(&flags),
        "needs" => show_needs(&flags),
        "admission" => admission_command(&flags),
        "admission-status" => admission_status(&flags),
        "observe-cairn-lifecycle" => observe_cairn_lifecycle(&flags),
        "inspect-work" => inspect_work(&flags),
        _ => fail("unknown command"),
    }
}

fn inspect_work(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let result = (|| -> Result<serde_json::Value, String> {
        let identity = required_identity(flags, "--work")?;
        let ledger = open_implementer(flags)?.work()?;
        let work = ledger
            .current()
            .find(|work| work.identity().is_ok_and(|id| id == identity))
            .ok_or("work is not current")?;
        let acceptance = kiln_core::implementation::acceptance_digest(work)
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        Ok(serde_json::json!({"work":work,"identity":identity,"acceptance_digest":acceptance}))
    })();
    match result {
        Ok(value) => {
            println!("{value}");
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

fn observe_cairn_lifecycle(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    use kiln_implementer::publication_effects::{
        CairnLifecycleBinding, PublicationEffectKind, PublicationEffectResult,
    };
    let result = (|| -> Result<kiln_implementer::publication_effects::PublicationEffectReservation,String> {
        let identity = required_identity(flags,"--attempt")?;
        let fence = required_positive_u64(flags,"--fence")?;
        let binding:CairnLifecycleBinding = read_json(&required_path(flags,"--binding-file")?)?;
        let repository = required_path(flags,"--repository")?;
        let report = required_path(flags,"--authoritative-report")?;
        let receipt_path = required_value(flags,"--receipt-path")?;
        let mut store = open_implementer(flags)?;
        let attempt = store.attempt(&identity)?.ok_or("attempt is not retained")?;
        let ledger = store.work()?;
        let work = ledger.current().find(|work|work.identity().is_ok_and(|id|id==attempt.work_identity)).ok_or("work is not current")?;
        store.consume_publication_effect(work,&identity,fence,PublicationEffectKind::CairnLifecycle(binding.clone()),
            |_,publication| {
                let observed=kiln_adapter_cairn::lifecycle_observation::observe_reviewed_lifecycle(
                    &repository,publication,&binding,&receipt_path,&report,
                )?;
                Ok(Some(PublicationEffectResult::CairnLifecycle(observed)))
            })
    })();
    match result {
        Ok(value) => match serde_json::to_string(&value) {
            Ok(json) => {
                println!("{json}");
                Ok(())
            }
            Err(_) => fail("lifecycle observation encoding failed"),
        },
        Err(error) => fail(&error),
    }
}

fn prepare_needs(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let result = (|| -> Result<kiln_core::types::ContentId, String> {
        let identity = required_identity(flags, "--work")?;
        let mut needs: kiln_core::inventory::WorkNeeds =
            read_json(&required_path(flags, "--needs-file")?)?;
        let mut store = open_implementer(flags)?;
        let ledger = store.work()?;
        let work = ledger
            .current()
            .find(|work| work.identity().is_ok_and(|id| id == identity))
            .ok_or("work is not current")?;
        let admission = store.admission_snapshot()?;
        let policy = admission
            .policy
            .ok_or("needs require current admitted policy")?
            .identity()
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        needs.work_version = identity.as_str().into();
        needs.policy_ref = policy.as_str().into();
        let attachment =
            kiln_core::inventory::WorkNeedsAttachment::new(identity, work.revision, policy, needs)
                .map_err(str::to_string)?;
        let identity = attachment.identity().map_err(str::to_string)?;
        let bytes = serde_json::to_vec_pretty(&attachment).map_err(|error| error.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(required_path(flags, "--output")?)
            .map_err(|error| error.to_string())?;
        std::io::Write::write_all(&mut file, &bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        Ok(identity)
    })();
    match result {
        Ok(identity) => {
            println!(
                "{{\"requirements\":\"{}\",\"admitted\":false}}",
                identity.as_str()
            );
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

fn prepare_verification_profile(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let result = (|| -> Result<kiln_core::types::ContentId, String> {
        let mut profile: kiln_implementer::verification::VerificationProfile =
            read_json(&required_path(flags, "--profile")?)?;
        profile.environment_identity = profile.derived_environment_identity()?;
        profile.toolchain = profile.derived_toolchain_identity()?;
        let identity = profile.identity()?;
        let bytes = serde_json::to_vec_pretty(&profile).map_err(|error| error.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(required_path(flags, "--output")?)
            .map_err(|error| error.to_string())?;
        std::io::Write::write_all(&mut file, &bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        Ok(identity)
    })();
    match result {
        Ok(identity) => {
            println!(
                "{{\"verification_profile\":\"{}\",\"checks_executed\":false}}",
                identity.as_str()
            );
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

fn capture_candidate(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let result = (|| -> Result<kiln_core::types::ContentId, String> {
        use kiln_implementer::verification::{
            VerificationProfile, VerifierAuthority, materialize_candidate,
        };
        let identity = required_identity(flags, "--attempt")?;
        let fence = required_positive_u64(flags, "--fence")?;
        let dispatch: kiln_core::implementation::ImplementationDispatchPlan =
            read_json(&required_path(flags, "--dispatch-plan")?)?;
        let profile: VerificationProfile = read_json(&required_path(flags, "--profile")?)?;
        let authority: VerifierAuthority = read_json(&required_path(flags, "--authority")?)?;
        let candidate_root = required_path(flags, "--candidate-root")?;
        let receipt_file = required_path(flags, "--host-receipt")?;
        let metadata = std::fs::metadata(&receipt_file).map_err(|error| error.to_string())?;
        if metadata.len() > 2 * 1024 * 1024 {
            return Err("host receipt exceeds admission bound".into());
        }
        let host_receipt = std::fs::read(&receipt_file).map_err(|error| error.to_string())?;
        let mut implementer = open_implementer(flags)?;
        let attempt = implementer.authorize_current(&identity, fence, wall_clock_ms())?;
        if implementer.dispatch_plan(&identity)?.as_ref() != Some(&dispatch) {
            return Err("capture dispatch differs from retained bounded plan".into());
        }
        let work = implementer.active_work(&identity, fence, wall_clock_ms())?;
        let handoff = materialize_candidate(
            &work,
            &attempt,
            &dispatch,
            &profile,
            &authority,
            &candidate_root,
            &host_receipt,
        )?;
        let bytes = serde_json::to_vec_pretty(&handoff).map_err(|error| error.to_string())?;
        let output = required_path(flags, "--handoff-output")?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)
            .map_err(|error| error.to_string())?;
        std::io::Write::write_all(&mut file, &bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        implementer.admit_candidate_handoff(&work, &identity, fence, wall_clock_ms(), &handoff)
    })();
    match result {
        Ok(identity) => {
            println!("{{\"handoff\":\"{}\"}}", identity.as_str());
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

fn admission_command(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let result = (|| -> Result<kiln_implementer::admission::AdmissionResult, String> {
        let command = read_json(&required_path(flags, "--command-file")?)?;
        open_implementer(flags)?.admission_apply(command, wall_clock_ms())
    })();
    match result {
        Ok(result) => match serde_json::to_string(&result) {
            Ok(json) => {
                println!("{json}");
                Ok(())
            }
            Err(_) => fail("admission encoding failed"),
        },
        Err(error) => fail(&error),
    }
}

fn admission_status(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    match open_implementer(flags).and_then(|mut store| store.admission_snapshot()) {
        Ok(result) => match serde_json::to_string(&result) {
            Ok(json) => {
                println!("{json}");
                Ok(())
            }
            Err(_) => fail("admission encoding failed"),
        },
        Err(error) => fail(&error),
    }
}

fn recover_attempts(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let result = open_implementer(flags).and_then(|mut store| store.recover(wall_clock_ms()));
    match result {
        Ok(recovery) => {
            let expired: Vec<_> = recovery.expired.iter().map(|id| id.as_str()).collect();
            let active: Vec<_> = recovery.active.iter().map(|id| id.as_str()).collect();
            println!(
                "{}",
                serde_json::json!({"expired":expired,"reconcile":active,"project_complete":false})
            );
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

fn reconcile_termination(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let result = (|| -> Result<(), String> {
        let attempt = required_identity(flags, "--attempt")?;
        let fence = required_positive_u64(flags, "--fence")?;
        let observed: kiln_core::types::ExternalRef =
            read_json(&required_path(flags, "--termination-file")?)?;
        open_implementer(flags)?.reconcile_unknown(&attempt, fence, &observed)
    })();
    match result {
        Ok(()) => {
            println!("{{\"termination_observed\":true}}");
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

fn attach_needs(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let result = (|| -> Result<kiln_core::types::ContentId, String> {
        let attachment: kiln_core::inventory::WorkNeedsAttachment =
            read_json(&required_path(flags, "--needs-file")?)?;
        open_implementer(flags)?.attach_work_needs(&attachment)
    })();
    match result {
        Ok(identity) => {
            println!("{{\"requirements\":\"{}\"}}", identity.as_str());
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

fn show_needs(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let result = (|| -> Result<Option<kiln_core::inventory::WorkNeedsAttachment>, String> {
        let work = required_identity(flags, "--work")?;
        let policy = required_identity(flags, "--policy-identity")?;
        open_implementer(flags)?.work_needs(&work, &policy)
    })();
    match result {
        Ok(attachment) => match serde_json::to_string(&attachment) {
            Ok(json) => {
                println!("{json}");
                Ok(())
            }
            Err(_) => fail("requirements encoding failed"),
        },
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.operator_claim]
fn claim(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let lease_ms = match optional_positive(flags, "--lease-ms") {
        Ok(value) => value.map_or(3_600_000, |value| u64::try_from(value).unwrap_or(u64::MAX)),
        Err(error) => return fail(&error),
    };
    match implementer.claim_next(wall_clock_ms(), lease_ms) {
        Ok(Some(attempt)) => match serde_json::to_string(&attempt) {
            Ok(document) => {
                println!("{document}");
                Ok(())
            }
            Err(_) => fail("attempt could not be encoded"),
        },
        Ok(None) => {
            println!("null");
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.operator_claim]
fn cancel(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let attempt = match required_value(flags, "--attempt") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let identity = match kiln_core::types::ContentId::from_blake3_hex(attempt) {
        Ok(identity) => identity,
        Err(_) => return fail("--attempt must be one BLAKE3 identity"),
    };
    let fence = match required_positive_u64(flags, "--fence") {
        Ok(fence) => fence,
        Err(error) => return fail(&error),
    };
    let cause = flags
        .get("--cause")
        .map_or("operator cancelled".to_string(), Clone::clone);
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    match implementer.cancel(&identity, fence, wall_clock_ms(), &cause) {
        Ok(()) => {
            println!("{{\"cancelled\":\"{}\"}}", identity.as_str());
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.fencing]
fn renew(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let identity = match required_identity(flags, "--attempt") {
        Ok(identity) => identity,
        Err(error) => return fail(&error),
    };
    let fence = match required_positive_u64(flags, "--fence") {
        Ok(fence) => fence,
        Err(error) => return fail(&error),
    };
    let lease_ms = match optional_positive(flags, "--lease-ms") {
        Ok(Some(value)) => u64::try_from(value).unwrap_or(u64::MAX),
        Ok(None) => 3_600_000,
        Err(error) => return fail(&error),
    };
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    match implementer.renew(&identity, fence, wall_clock_ms(), lease_ms) {
        Ok(until) => {
            println!("{{\"lease_until_ms\":{until}}}");
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.dispatch_reservation]
fn reserve(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let identity = match required_identity(flags, "--attempt") {
        Ok(identity) => identity,
        Err(error) => return fail(&error),
    };
    let fence = match required_positive_u64(flags, "--fence") {
        Ok(fence) => fence,
        Err(error) => return fail(&error),
    };
    let plan = match required_identity(flags, "--plan") {
        Ok(plan) => plan,
        Err(error) => return fail(&error),
    };
    let provider = match required_value(flags, "--provider") {
        Ok(value) => match kiln_core::types::ProviderProfileId::new(value) {
            Ok(provider) => provider,
            Err(_) => return fail("--provider must be a bounded profile identity"),
        },
        Err(error) => return fail(&error),
    };
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    match implementer.reserve_dispatch(&identity, fence, wall_clock_ms(), &plan, &provider) {
        Ok(reservation) => match serde_json::to_string(&reservation) {
            Ok(document) => {
                println!("{document}");
                Ok(())
            }
            Err(_) => fail("dispatch reservation could not be encoded"),
        },
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.dispatch_plan] r[impl kiln.implementation.dispatch_reservation]
fn reserve_plan(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let plan_file = match required_path(flags, "--plan-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let plan: kiln_core::implementation::ImplementationDispatchPlan = match read_json(&plan_file) {
        Ok(plan) => plan,
        Err(error) => return fail(&error),
    };
    let mut implementer = match open_implementer(flags) {
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
                .is_ok_and(|identity| identity == plan.work_identity)
        })
        .cloned()
    else {
        return fail("dispatch plan work identity is not current");
    };
    match implementer.reserve_dispatch_plan(
        &work,
        &plan.attempt_identity,
        plan.fence,
        wall_clock_ms(),
        &plan,
    ) {
        Ok(reservation) => match serde_json::to_string(&reservation) {
            Ok(document) => {
                println!("{document}");
                Ok(())
            }
            Err(_) => fail("dispatch reservation could not be encoded"),
        },
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.dispatch] r[impl kiln.implementation.dispatch_reservation]
fn start(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let identity = match required_identity(flags, "--attempt") {
        Ok(identity) => identity,
        Err(error) => return fail(&error),
    };
    let fence = match required_positive_u64(flags, "--fence") {
        Ok(fence) => fence,
        Err(error) => return fail(&error),
    };
    let namespace = flags.get("--namespace").map_or("lattice", String::as_str);
    let kind = match required_value(flags, "--workflow-kind") {
        Ok(kind) => kind,
        Err(error) => return fail(&error),
    };
    let id = match required_value(flags, "--workflow-id") {
        Ok(id) => id,
        Err(error) => return fail(&error),
    };
    let workflow = match kiln_core::types::ExternalRef::new(namespace, &kind, &id) {
        Ok(workflow) => workflow,
        Err(_) => return fail("--workflow-kind or --workflow-id is malformed"),
    };
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    match implementer.start_execution(&identity, fence, wall_clock_ms(), &workflow) {
        Ok(()) => {
            println!("{{\"started\":\"{}\"}}", identity.as_str());
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.publication]
fn prepare_publication(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let file = match required_path(flags, "--publication-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let plan: kiln_core::implementation::PublicationPlan = match read_json(&file) {
        Ok(plan) => plan,
        Err(error) => return fail(&error),
    };
    let mut implementer = match open_implementer(flags) {
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
                .is_ok_and(|identity| identity == plan.work_identity)
        })
        .cloned()
    else {
        return fail("publication work identity is not current");
    };
    let handoff = match implementer.candidate_handoff(&plan.attempt) {
        Ok(Some(handoff)) => handoff,
        Ok(None) => return fail("candidate handoff is not retained"),
        Err(error) => return fail(&error),
    };
    let verification = match implementer.verification_receipt(&plan.attempt) {
        Ok(Some(verification)) => verification,
        Ok(None) => return fail("verification receipt is not retained"),
        Err(error) => return fail(&error),
    };
    match implementer.prepare_publication(
        &work,
        &plan.attempt,
        plan.fence,
        &handoff,
        &verification,
        &plan,
    ) {
        Ok(identity) => {
            println!("{{\"publication\":\"{}\"}}", identity.as_str());
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.publication] r[impl kiln.github.publication]
fn publish(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let file = match required_path(flags, "--publication-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let secret_file = match required_path(flags, "--secret-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let plan: kiln_core::implementation::PublicationPlan = match read_json(&file) {
        Ok(plan) => plan,
        Err(error) => return fail(&error),
    };
    let timeout_ms = match optional_positive(flags, "--timeout-ms") {
        Ok(Some(value)) => u64::try_from(value).unwrap_or(u64::MAX),
        Ok(None) => 30_000,
        Err(error) => return fail(&error),
    };
    let token = match std::fs::read(&secret_file) {
        Ok(bytes) => match kiln_adapter_github::secret::ResolvedSecret::from_bytes(bytes) {
            Ok(token) => token,
            Err(_) => return fail("secret file is not a valid GitHub token"),
        },
        Err(_) => return fail("secret file is unreadable"),
    };
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let record = match implementer.publication_record(&plan.attempt) {
        Ok(Some(record)) => record,
        Ok(None) => return fail("publication is not prepared"),
        Err(error) => return fail(&error),
    };
    if record.plan != plan {
        return fail("publication file differs from the prepared plan");
    }
    if matches!(
        record.phase,
        kiln_core::implementation::PublicationPhase::PullRequestPublished
            | kiln_core::implementation::PublicationPhase::Integrated
            | kiln_core::implementation::PublicationPhase::SourceReconciled
    ) {
        println!(
            "{{\"pull_request\":\"{}\"}}",
            record
                .pull_request
                .as_ref()
                .map(|reference| reference.native_id.as_str())
                .unwrap_or("")
        );
        return Ok(());
    }
    let ledger = match implementer.work() {
        Ok(ledger) => ledger,
        Err(error) => return fail(&error),
    };
    let Some(work) = ledger
        .current()
        .find(|work| {
            work.identity()
                .is_ok_and(|identity| identity == plan.work_identity)
        })
        .cloned()
    else {
        return fail("publication work identity is not current");
    };
    use kiln_implementer::publication_effects::{
        EffectDispatch, PublicationEffectKind, PublicationEffectResult,
    };
    let client =
        match kiln_adapter_github::publication::GithubPublicationClient::new(token, timeout_ms) {
            Ok(client) => client,
            Err(_) => return fail("publication transport configuration is invalid"),
        };
    let branch = implementer.consume_publication_effect(
        &work,
        &plan.attempt,
        plan.fence,
        PublicationEffectKind::Branch,
        |dispatch, _| {
            let head = match dispatch {
                EffectDispatch::FirstDispatch => client
                    .ensure_branch(&plan)
                    .and_then(|_| client.reconcile_branch(&plan)),
                EffectDispatch::ReconcileOnly => client.reconcile_branch(&plan),
            }
            .map_err(|error| format!("{error:?}"))?;
            Ok(head.map(PublicationEffectResult::Branch))
        },
    );
    match branch {
        Ok(reservation) if reservation.observed.is_some() => {}
        Ok(_) => return fail("branch outcome remains unknown; reconcile before continuing"),
        Err(error) => return fail(&error),
    }
    match implementer.consume_publication_effect(
        &work,
        &plan.attempt,
        plan.fence,
        PublicationEffectKind::PullRequest,
        |dispatch, _| {
            let outcome = match dispatch {
                EffectDispatch::FirstDispatch => client
                    .ensure_pull_request(&plan)
                    .and_then(|_| client.reconcile_pull_request(&plan)),
                EffectDispatch::ReconcileOnly => client.reconcile_pull_request(&plan),
            }
            .map_err(|error| format!("{error:?}"))?;
            Ok(outcome.map(|outcome| PublicationEffectResult::PullRequest {
                pull_request: outcome.pull_request,
                head: outcome.branch_head,
            }))
        },
    ) {
        Ok(reservation) if reservation.observed.is_some() => {
            println!(
                "{}",
                serde_json::to_string(&reservation).unwrap_or_else(|_| "null".into())
            );
        }
        Ok(_) => return fail("pull request outcome remains unknown; reconcile before retry"),
        Err(error) => return fail(&error),
    }
    Ok(())
}

// r[impl kiln.implementation.integration] r[impl kiln.github.landing.merge]
fn recover_integration(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let publication_file = match required_path(flags, "--publication-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let landing_file = match required_path(flags, "--landing-plan") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let secret_file = match required_path(flags, "--secret-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let plan: kiln_core::implementation::PublicationPlan = match read_json(&publication_file) {
        Ok(plan) => plan,
        Err(error) => return fail(&error),
    };
    let landing: kiln_adapter_github::landing::LandingPlan = match read_json(&landing_file) {
        Ok(landing) => landing,
        Err(error) => return fail(&error),
    };
    let timeout_ms = match optional_positive(flags, "--timeout-ms") {
        Ok(Some(value)) => u64::try_from(value).unwrap_or(u64::MAX),
        Ok(None) => 30_000,
        Err(error) => return fail(&error),
    };
    let token = match std::fs::read(&secret_file) {
        Ok(bytes) => match kiln_adapter_github::secret::ResolvedSecret::from_bytes(bytes) {
            Ok(token) => token,
            Err(_) => return fail("secret file is not a valid GitHub token"),
        },
        Err(_) => return fail("secret file is unreadable"),
    };
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let record = match implementer.publication_record(&plan.attempt) {
        Ok(Some(record)) => record,
        Ok(None) => return fail("publication is not prepared"),
        Err(error) => return fail(&error),
    };
    if record.plan != plan {
        return fail("publication file differs from the prepared plan");
    }
    if record.phase == kiln_core::implementation::PublicationPhase::Integrated {
        let receipt = match implementer.integration_receipt(&plan.attempt) {
            Ok(Some(receipt)) => receipt,
            _ => return fail("integration receipt is missing"),
        };
        println!(
            "{{\"merge_commit\":\"{}\"}}",
            receipt.merge_commit.native_id.as_str()
        );
        return Ok(());
    }
    if record.phase != kiln_core::implementation::PublicationPhase::PullRequestPublished {
        return fail("publication does not have a pull request");
    }
    let Some(pull_request) = &record.pull_request else {
        return fail("publication pull request identity is missing");
    };
    let Some((_, owner)) = plan.repository.split_once('/') else {
        return fail("publication repository is malformed");
    };
    if landing.repository != plan.repository
        || landing.owner != owner
        || landing.head_sha != plan.candidate.native_id.as_str()
        || landing.base_ref != plan.base_branch
        || landing.base_head != plan.base.native_id.as_str()
        || !pull_request
            .native_id
            .as_str()
            .ends_with(&format!("#{}", landing.pull_number))
    {
        return fail("landing plan does not match the prepared publication");
    }
    let work = match implementer.work() {
        Ok(work) => work,
        Err(error) => return fail(&error),
    };
    let Some(work) = work
        .current()
        .find(|work| {
            work.identity()
                .is_ok_and(|identity| identity == plan.work_identity)
        })
        .cloned()
    else {
        return fail("publication work identity is not current");
    };
    let client = match kiln_adapter_github::landing::GithubLandingClient::new(token, timeout_ms) {
        Ok(client) => client,
        Err(_) => return fail("landing transport configuration is invalid"),
    };
    use kiln_implementer::publication_effects::{PublicationEffectKind, PublicationEffectResult};
    match implementer.consume_publication_effect(
        &work,
        &plan.attempt,
        plan.fence,
        PublicationEffectKind::Integration,
        |_, _| {
            let Some(landed) = client
                .recover_landed(&landing)
                .map_err(|error| format!("{error:?}"))?
            else {
                return Ok(None);
            };
            let commit = |value: &str| {
                kiln_core::types::ExternalRef::new("git", "commit", value)
                    .map_err(|error| format!("{error:?}"))
            };
            let receipt = kiln_core::implementation::IntegrationReceipt {
                schema: kiln_core::implementation::INTEGRATION_SCHEMA.to_string(),
                work_identity: plan.work_identity.clone(),
                work_revision: plan.work_revision,
                attempt: plan.attempt.clone(),
                fence: plan.fence,
                handoff: plan.handoff.clone(),
                verification: plan.verification.clone(),
                candidate: plan.candidate.clone(),
                pull_request: landed.pull_request.clone(),
                merge_commit: commit(&landed.merge_commit)?,
                base_ref: landed.base_ref.clone(),
                base_head: commit(&landed.base_head)?,
                mapping: landed.mapping,
            };
            Ok(Some(PublicationEffectResult::Integration(receipt)))
        },
    ) {
        Ok(reservation) if reservation.observed.is_some() => {
            println!(
                "{}",
                serde_json::to_string(&reservation).unwrap_or_else(|_| "null".into())
            );
            Ok(())
        }
        Ok(_) => fail("external integration is not independently established"),
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.source_status] r[impl kiln.github.source_status]
fn close_source(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let plan_file = match required_path(flags, "--source-plan") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let secret_file = match required_path(flags, "--secret-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let plan: kiln_core::implementation::SourceStatusPlan = match read_json(&plan_file) {
        Ok(plan) => plan,
        Err(error) => return fail(&error),
    };
    let timeout_ms = match optional_positive(flags, "--timeout-ms") {
        Ok(Some(value)) => u64::try_from(value).unwrap_or(u64::MAX),
        Ok(None) => 30_000,
        Err(error) => return fail(&error),
    };
    let token = match std::fs::read(&secret_file) {
        Ok(bytes) => match kiln_adapter_github::secret::ResolvedSecret::from_bytes(bytes) {
            Ok(token) => token,
            Err(_) => return fail("secret file is not a valid GitHub token"),
        },
        Err(_) => return fail("secret file is unreadable"),
    };
    let mut implementer = match open_implementer(flags) {
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
                .is_ok_and(|identity| identity == plan.work_identity)
        })
        .cloned()
    else {
        return fail("source-status work identity is not current");
    };
    let attempt = match implementer.attempt(&plan.attempt) {
        Ok(Some(attempt)) => attempt,
        Ok(None) => return fail("source-status attempt is not retained"),
        Err(error) => return fail(&error),
    };
    if attempt.publication.as_ref().is_some_and(|publication| {
        publication.phase == kiln_core::implementation::PublicationPhase::SourceReconciled
    }) {
        let retained = match implementer.source_status_receipt(&plan.attempt) {
            Ok(Some(receipt)) => receipt,
            _ => return fail("source-status receipt is missing"),
        };
        if retained.work_identity != plan.work_identity
            || retained.work_revision != plan.work_revision
            || retained.attempt != plan.attempt
            || retained.fence != plan.fence
            || retained.integration != plan.integration
            || retained.source != plan.source
            || retained.observed_revision != plan.expected_observed_revision
        {
            return fail("source-status plan differs from the reconciled receipt");
        }
        println!("{{\"source_status\":\"retained\"}}");
        return Ok(());
    }
    if kiln_core::implementation::admit_source_status_plan(&work, &attempt, &plan).is_err() {
        return fail("source-status plan is not admissible for the current publication state");
    }
    let client = match kiln_adapter_github::source_status::GithubSourceStatusClient::new(
        token, timeout_ms,
    ) {
        Ok(client) => client,
        Err(_) => return fail("source-status transport configuration is invalid"),
    };
    use kiln_implementer::publication_effects::{PublicationEffectKind, PublicationEffectResult};
    match implementer.consume_publication_effect(
        &work,
        &plan.attempt,
        plan.fence,
        PublicationEffectKind::SourceStatus(plan.clone()),
        |_, _| {
            let observed = client
                .observe_exact(&plan)
                .map_err(|error| format!("{error:?}"))?;
            if observed.state != "closed" {
                return Ok(None);
            }
            let receipt = kiln_core::implementation::SourceStatusReceipt {
                schema: kiln_core::implementation::SOURCE_STATUS_SCHEMA.to_string(),
                work_identity: plan.work_identity.clone(),
                work_revision: plan.work_revision,
                attempt: plan.attempt.clone(),
                fence: plan.fence,
                integration: plan.integration.clone(),
                source: plan.source.clone(),
                state: observed.state,
                observed_revision: observed.revision,
            };
            Ok(Some(PublicationEffectResult::SourceStatus {
                plan: plan.clone(),
                receipt,
            }))
        },
    ) {
        Ok(reservation) if reservation.observed.is_some() => {
            println!(
                "{}",
                serde_json::to_string(&reservation).unwrap_or_else(|_| "null".into())
            );
            Ok(())
        }
        Ok(_) => {
            fail("source remains open; automatic source mutation has no qualified version guard")
        }
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.source_status] r[impl kiln.github.source_status]
fn source_status(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let plan_file = match required_path(flags, "--source-plan") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let secret_file = match required_path(flags, "--secret-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let plan: kiln_core::implementation::SourceStatusPlan = match read_json(&plan_file) {
        Ok(plan) => plan,
        Err(error) => return fail(&error),
    };
    let timeout_ms = match optional_positive(flags, "--timeout-ms") {
        Ok(Some(value)) => u64::try_from(value).unwrap_or(u64::MAX),
        Ok(None) => 30_000,
        Err(error) => return fail(&error),
    };
    let token = match std::fs::read(&secret_file) {
        Ok(bytes) => match kiln_adapter_github::secret::ResolvedSecret::from_bytes(bytes) {
            Ok(token) => token,
            Err(_) => return fail("secret file is not a valid GitHub token"),
        },
        Err(_) => return fail("secret file is unreadable"),
    };
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let receipt = match implementer.source_status_receipt(&plan.attempt) {
        Ok(Some(receipt)) => receipt,
        Ok(None) => return fail("source status has not been reconciled"),
        Err(error) => return fail(&error),
    };
    if receipt.state != "closed"
        || receipt.work_identity != plan.work_identity
        || receipt.work_revision != plan.work_revision
        || receipt.attempt != plan.attempt
        || receipt.fence != plan.fence
        || receipt.integration != plan.integration
        || receipt.source != plan.source
    {
        return fail("source-status plan differs from the reconciled receipt");
    }
    let client = match kiln_adapter_github::source_status::GithubSourceStatusClient::new(
        token, timeout_ms,
    ) {
        Ok(client) => client,
        Err(_) => return fail("source-status transport configuration is invalid"),
    };
    let observed = match client.observe(&plan) {
        Ok(state) => state,
        Err(error) => return fail(&format!("{error:?}")),
    };
    let matches = observed == receipt.state;
    println!(
        "{{\"retained\":{},\"observed\":{},\"match\":{matches}}}",
        serde_json::to_string(&receipt.state).unwrap_or_else(|_| "\"\"".to_string()),
        serde_json::to_string(&observed).unwrap_or_else(|_| "\"\"".to_string())
    );
    if !matches
        && flags
            .get("--require-match")
            .is_some_and(|value| matches!(value.as_str(), "true" | "1"))
    {
        return fail("source state differs from the reconciled receipt");
    }
    Ok(())
}

// r[impl kiln.implementation.metrics]
fn metrics(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let identity = match required_identity(flags, "--attempt") {
        Ok(identity) => identity,
        Err(error) => return fail(&error),
    };
    let mut input = MetricsInput::default();
    if let Some(path) = flags.get("--usage-file") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return fail("--usage-file must be absolute");
        }
        input = match read_json(&path) {
            Ok(input) => input,
            Err(error) => return fail(&error),
        };
    }
    if input.traces.len() > 8 {
        return fail("--usage-file has too many trace references");
    }
    let usage = kiln_core::implementation::AttemptUsage {
        tokens: input
            .tokens
            .unwrap_or(kiln_core::implementation::TokenObservation::Absent {
                reason: kiln_core::implementation::AbsenceReason::NotReported,
            }),
        auxiliary_calls: input.auxiliary_calls.unwrap_or(
            kiln_core::implementation::AuxiliaryCallObservation::Absent {
                reason: kiln_core::implementation::AbsenceReason::NotReported,
            },
        ),
    };
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let stored = match implementer.attempt(&identity) {
        Ok(Some(attempt)) => attempt,
        Ok(None) => return fail("attempt is not retained"),
        Err(error) => return fail(&error),
    };
    let fence = match flags.get("--fence") {
        Some(value) => match value.parse::<u64>() {
            Ok(fence) if fence > 0 => fence,
            _ => return fail("--fence must be positive"),
        },
        None => stored.fence,
    };
    let record = flags
        .get("--record")
        .is_some_and(|value| matches!(value.as_str(), "true" | "1"));
    let metrics = if record {
        implementer.record_metrics(&identity, fence, wall_clock_ms(), usage, input.traces)
    } else {
        implementer.observe_metrics(&identity, fence, wall_clock_ms(), usage, input.traces)
    };
    match metrics {
        Ok(metrics) => match serde_json::to_string(&metrics) {
            Ok(document) => {
                println!("{document}");
                Ok(())
            }
            Err(_) => fail("metrics receipt could not be encoded"),
        },
        Err(error) => fail(&error),
    }
}

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MetricsInput {
    #[serde(default)]
    tokens: Option<kiln_core::implementation::TokenObservation>,
    #[serde(default)]
    auxiliary_calls: Option<kiln_core::implementation::AuxiliaryCallObservation>,
    #[serde(default)]
    traces: Vec<kiln_core::types::ExternalRef>,
}

// r[impl kiln.implementation.dispatch]
fn reconcile(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let identity = match required_identity(flags, "--attempt") {
        Ok(identity) => identity,
        Err(error) => return fail(&error),
    };
    let fence = match required_positive_u64(flags, "--fence") {
        Ok(fence) => fence,
        Err(error) => return fail(&error),
    };
    let namespace = flags.get("--namespace").map_or("lattice", String::as_str);
    let kind = match required_value(flags, "--workflow-kind") {
        Ok(kind) => kind,
        Err(error) => return fail(&error),
    };
    let id = match required_value(flags, "--workflow-id") {
        Ok(id) => id,
        Err(error) => return fail(&error),
    };
    let workflow = match kiln_core::types::ExternalRef::new(namespace, &kind, &id) {
        Ok(workflow) => workflow,
        Err(_) => return fail("--workflow-kind or --workflow-id is malformed"),
    };
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    match implementer.reconcile_dispatch(&identity, fence, wall_clock_ms(), &workflow) {
        Ok(()) => {
            println!("{{\"reconciled\":\"{}\"}}", identity.as_str());
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.candidate_handoff]
fn handoff(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let handoff_file = match required_path(flags, "--handoff-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let handoff: kiln_core::implementation::CandidateHandoff = match read_json(&handoff_file) {
        Ok(handoff) => handoff,
        Err(error) => return fail(&error),
    };
    let mut implementer = match open_implementer(flags) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let work =
        match implementer.active_work(&handoff.attempt_identity, handoff.fence, wall_clock_ms()) {
            Ok(work) => work,
            Err(error) => return fail(&error),
        };
    match implementer.admit_candidate_handoff(
        &work,
        &handoff.attempt_identity,
        handoff.fence,
        wall_clock_ms(),
        &handoff,
    ) {
        Ok(identity) => {
            println!("{{\"handoff\":\"{}\"}}", identity.as_str());
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

fn required_identity(
    flags: &BTreeMap<String, String>,
    key: &str,
) -> Result<kiln_core::types::ContentId, String> {
    let value = required_value(flags, key)?;
    kiln_core::types::ContentId::from_blake3_hex(value)
        .map_err(|_| format!("{key} must be one BLAKE3 identity"))
}

fn open_implementer(flags: &BTreeMap<String, String>) -> Result<Implementer, String> {
    let database = required_path(flags, "--database")?;
    let enrollment_file = required_path(flags, "--enrollment")?;
    let enrollment: Enrollment = read_json(&enrollment_file)?;
    Implementer::open(&database, enrollment).map_err(|error| error.to_string())
}

fn drain(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let inputs = match drain_inputs(flags) {
        Ok(inputs) => inputs,
        Err(error) => return fail(&error),
    };
    match drain_once(&inputs) {
        Ok(drained) => {
            println!("{{\"drained\":{drained}}}");
            Ok(())
        }
        Err(error) => fail(&error),
    }
}

// r[impl kiln.implementation.drain_loop]
fn run_loop(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let inputs = match drain_inputs(flags) {
        Ok(inputs) => inputs,
        Err(error) => return fail(&error),
    };
    let interval_ms = match bounded_interval(flags, "--interval-ms") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let iterations = match optional_positive(flags, "--iterations") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        if signal_hook::flag::register(signal, Arc::clone(&stop)).is_err() {
            return fail("shutdown signal registration failed");
        }
    }
    let mut implementer = match Implementer::open(&inputs.database, inputs.enrollment.clone()) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let mut done = 0usize;
    let mut drained = 0usize;
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        match drain_into(&inputs, &mut implementer) {
            Ok(count) => drained = drained.saturating_add(count),
            Err(error) => return fail(&error),
        }
        done += 1;
        if iterations.is_some_and(|limit| done >= limit) {
            break;
        }
        wait_for_interval(interval_ms, &stop);
    }
    println!("{{\"iterations\":{done},\"drained\":{drained}}}");
    Ok(())
}

fn drain_once(inputs: &DrainInputs) -> Result<usize, String> {
    let mut implementer = Implementer::open(&inputs.database, inputs.enrollment.clone())
        .map_err(|error| error.to_string())?;
    drain_into(inputs, &mut implementer)
}

fn drain_into(inputs: &DrainInputs, implementer: &mut Implementer) -> Result<usize, String> {
    let config = ImplementationQueueConfig {
        path: inputs.queue.clone(),
        maximum_deliveries: inputs.maximum_deliveries,
    };
    drain_issue_queue(
        &config,
        &inputs.secret,
        &inputs.policy,
        &inputs.enrollment.actors,
        implementer,
        inputs.limit,
    )
    .map_err(|error| format!("{error:?}"))
}

fn drain_inputs(flags: &BTreeMap<String, String>) -> Result<DrainInputs, String> {
    let database = required_path(flags, "--database")?;
    let queue = required_path(flags, "--queue")?;
    let secret_file = required_path(flags, "--secret-file")?;
    let policy_file = required_path(flags, "--policy")?;
    let enrollment_file = required_path(flags, "--enrollment")?;
    let maximum_deliveries = required_positive(flags, "--maximum-deliveries")?;
    let limit = required_positive(flags, "--limit")?;
    let secret = match std::fs::read(&secret_file) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        _ => return Err("secret file is empty or unreadable".to_string()),
    };
    let policy: ImplementationIssuePolicy = read_json(&policy_file)?;
    let enrollment: Enrollment = read_json(&enrollment_file)?;
    Ok(DrainInputs {
        database,
        queue,
        secret,
        policy,
        enrollment,
        maximum_deliveries,
        limit,
    })
}

// r[impl kiln.github.poll.transport] r[impl kiln.implementation.polling]
fn poll_run(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let interval_ms = match bounded_interval(flags, "--interval-ms") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let iterations = match optional_positive(flags, "--iterations") {
        Ok(value) => value,
        Err(error) => return fail(&error),
    };
    let poll_flags = poll_loop_flags(flags);
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        if signal_hook::flag::register(signal, std::sync::Arc::clone(&stop)).is_err() {
            return fail("shutdown signal registration failed");
        }
    }
    let mut done = 0usize;
    loop {
        if stop.load(std::sync::atomic::Ordering::Acquire) {
            break;
        }
        poll(&poll_flags)?;
        done += 1;
        if iterations.is_some_and(|limit| done >= limit) {
            break;
        }
        wait_for_interval(interval_ms, &stop);
    }
    println!("{{\"iterations\":{done}}}");
    Ok(())
}

fn poll_loop_flags(flags: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut poll_flags = flags.clone();
    poll_flags
        .entry("--since-ms".to_string())
        .or_insert_with(|| wall_clock_ms().to_string());
    poll_flags
}

fn wait_for_interval(interval_ms: usize, stop: &std::sync::atomic::AtomicBool) {
    let deadline =
        Instant::now() + Duration::from_millis(u64::try_from(interval_ms).unwrap_or(u64::MAX));
    while !stop.load(std::sync::atomic::Ordering::Acquire) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        std::thread::sleep(remaining.min(Duration::from_millis(250)));
    }
}

// r[impl kiln.github.poll.transport] r[impl kiln.implementation.polling]
fn poll(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let database = match required_path(flags, "--database") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let enrollment_file = match required_path(flags, "--enrollment") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let policy_file = match required_path(flags, "--policy") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let secret_file = match required_path(flags, "--secret-file") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let max_pages = match required_positive(flags, "--max-pages") {
        Ok(pages) => match u32::try_from(pages) {
            Ok(pages) => pages,
            Err(_) => return fail("--max-pages exceeds its bound"),
        },
        Err(error) => return fail(&error),
    };
    let since_ms = match flags.get("--since-ms") {
        Some(value) => match value.parse::<u64>() {
            Ok(value) => value,
            Err(_) => return fail("--since-ms must be a non-negative integer"),
        },
        None => 0,
    };
    let timeout_ms = match optional_positive(flags, "--timeout-ms") {
        Ok(Some(value)) => u64::try_from(value).unwrap_or(u64::MAX),
        Ok(None) => 30_000,
        Err(error) => return fail(&error),
    };
    let enrollment: Enrollment = match read_json(&enrollment_file) {
        Ok(enrollment) => enrollment,
        Err(error) => return fail(&error),
    };
    let policy = match read_json(&policy_file) {
        Ok(policy) => policy,
        Err(error) => return fail(&error),
    };
    let token = match std::fs::read(&secret_file) {
        Ok(bytes) => match kiln_adapter_github::secret::ResolvedSecret::from_bytes(bytes) {
            Ok(token) => token,
            Err(_) => return fail("secret file is not a valid GitHub token"),
        },
        Err(_) => return fail("secret file is unreadable"),
    };
    let mut implementer = match Implementer::open(&database, enrollment.clone()) {
        Ok(implementer) => implementer,
        Err(error) => return fail(&error),
    };
    let mut transport =
        match kiln_adapter_github::poll_http::GithubIssuePollClient::new(token, timeout_ms) {
            Ok(transport) => transport,
            Err(_) => return fail("poll transport configuration is invalid"),
        };
    match kiln_adapter_github::poll::poll_into_implementer(
        &policy,
        &enrollment.actors,
        &mut transport,
        since_ms,
        max_pages,
        &mut implementer,
    ) {
        Ok(summary) => match serde_json::to_string(&summary) {
            Ok(document) => {
                println!("{document}");
                Ok(())
            }
            Err(_) => fail("poll summary could not be encoded"),
        },
        Err(error) => fail(&format!("{error:?}")),
    }
}

fn status(flags: &BTreeMap<String, String>) -> Result<(), ()> {
    let database = match required_path(flags, "--database") {
        Ok(path) => path,
        Err(error) => return fail(&error),
    };
    let enrollment_file = match required_path(flags, "--enrollment") {
        Ok(path) => path,
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
    let attempts = match implementer.attempts() {
        Ok(attempts) => attempts,
        Err(error) => return fail(&error),
    };
    let active = attempts
        .attempts()
        .filter(|attempt| attempt.phase.is_active())
        .count();
    let verified = attempts
        .attempts()
        .filter(|attempt| attempt.phase == kiln_core::implementation::AttemptPhase::Verified)
        .count();
    let published = attempts
        .attempts()
        .filter(|attempt| {
            attempt.publication.as_ref().is_some_and(|publication| {
                matches!(
                    publication.phase,
                    kiln_core::implementation::PublicationPhase::PullRequestPublished
                        | kiln_core::implementation::PublicationPhase::Integrated
                        | kiln_core::implementation::PublicationPhase::SourceReconciled
                )
            })
        })
        .count();
    let integrated = attempts
        .attempts()
        .filter(|attempt| {
            attempt.publication.as_ref().is_some_and(|publication| {
                matches!(
                    publication.phase,
                    kiln_core::implementation::PublicationPhase::Integrated
                        | kiln_core::implementation::PublicationPhase::SourceReconciled
                )
            })
        })
        .count();
    let source_reconciled = attempts
        .attempts()
        .filter(|attempt| {
            attempt.publication.as_ref().is_some_and(|publication| {
                publication.phase == kiln_core::implementation::PublicationPhase::SourceReconciled
            })
        })
        .count();
    let failed = attempts
        .attempts()
        .filter(|attempt| attempt.phase == kiln_core::implementation::AttemptPhase::Failed)
        .count();
    let blocked = attempts
        .attempts()
        .filter(|attempt| attempt.phase == kiln_core::implementation::AttemptPhase::Blocked)
        .count();
    let cancelled = attempts
        .attempts()
        .filter(|attempt| attempt.phase == kiln_core::implementation::AttemptPhase::Cancelled)
        .count();
    let superseded = attempts
        .attempts()
        .filter(|attempt| attempt.phase == kiln_core::implementation::AttemptPhase::Superseded)
        .count();
    println!(
        "{{\"work\":{},\"attempts\":{},\"active\":{active},\"verified\":{verified},\"published\":{published},\"integrated\":{integrated},\"source_reconciled\":{source_reconciled},\"failed\":{failed},\"blocked\":{blocked},\"cancelled\":{cancelled},\"superseded\":{superseded}}}",
        work.current().count(),
        attempts.attempts().count()
    );
    Ok(())
}

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

fn required_path(flags: &BTreeMap<String, String>, key: &str) -> Result<PathBuf, String> {
    let value = flags.get(key).ok_or_else(|| format!("{key} is required"))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    Ok(path)
}

fn required_value(flags: &BTreeMap<String, String>, key: &str) -> Result<String, String> {
    flags
        .get(key)
        .cloned()
        .ok_or_else(|| format!("{key} is required"))
}

fn required_positive_u64(flags: &BTreeMap<String, String>, key: &str) -> Result<u64, String> {
    let value = flags.get(key).ok_or_else(|| format!("{key} is required"))?;
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{key} must be positive"))
}

fn wall_clock_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn bounded_interval(flags: &BTreeMap<String, String>, key: &str) -> Result<usize, String> {
    let value = required_positive(flags, key)?;
    if value > MAX_INTERVAL_MILLISECONDS {
        return Err(format!("{key} exceeds its bound"));
    }
    Ok(value)
}

fn required_positive(flags: &BTreeMap<String, String>, key: &str) -> Result<usize, String> {
    let value = flags.get(key).ok_or_else(|| format!("{key} is required"))?;
    value
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{key} must be positive"))
}

fn optional_positive(flags: &BTreeMap<String, String>, key: &str) -> Result<Option<usize>, String> {
    match flags.get(key) {
        None => Ok(None),
        Some(value) => value
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .map(Some)
            .ok_or_else(|| format!("{key} must be positive")),
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    use std::io::Read;
    const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;
    let file =
        std::fs::File::open(path).map_err(|_| format!("{} is unreadable", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "bounded input read failed".to_string())?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err("JSON input exceeds 16MiB admission bound".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| format!("{} is not valid JSON", path.display()))
}

fn fail(message: &str) -> Result<(), ()> {
    eprintln!("kiln-implementer: {message}");
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polling_loop_defaults_to_its_start_time() {
        let mut flags = BTreeMap::new();
        flags.insert("--since-ms".to_string(), "7".to_string());
        assert_eq!(
            poll_loop_flags(&flags)
                .get("--since-ms")
                .map(String::as_str),
            Some("7")
        );
        let since = poll_loop_flags(&BTreeMap::new())
            .get("--since-ms")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        assert!(since > 0);
    }
}
