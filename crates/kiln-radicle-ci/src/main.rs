use std::io::{Read, Write};
use std::path::Path;

use kiln_adapter_http_observe::ObservationSnapshot;
use kiln_adapter_radicle::defelo::{
    DefeloResponse, DefeloResult, DefeloRunId, MAX_DEFELO_REQUEST_BYTES, map_defelo_request,
    parse_defelo_request,
};
use kiln_adapter_radicle::status::StatusDispatch;
use kiln_core::domain::Outcome;
use kiln_observe_core::EventWindow;
use kiln_onixos_pilot::{MAX_CONFIG_BYTES, load_json};
use kiln_radicle_ci::project::{SourceContext, identity};
use kiln_radicle_ci::{CiProfile, CiService, Result};

const USAGE: &str = "kiln-radicle-ci commands:
  --profile PROFILE_JSON                  # deployed Radicle broker stdin/stdout protocol
  start PROFILE_JSON TRIGGER_JSON        # reserve exact source and submit once
  reconcile PROFILE_JSON SOURCE_JSON     # original-operation/status reconciliation only
  observe PROFILE_JSON SOURCE_JSON       # finite observation, never redispatch
  replay PROFILE_JSON SOURCE_JSON
  bindings PROFILE_JSON SOURCE_JSON
  profile-identity PROFILE_JSON
  serve PROFILE_JSON SOURCE_JSON HTTP_JSON
  ucan-init HTTP_TEMPLATE_JSON PROFILE_JSON SOURCE_JSON PRIVATE_DIRECTORY
  ucan-probe PROFILE_JSON SOURCE_JSON PRIVATE_DIRECTORY
Source context is {provider,repository,revision}; the profile fixes the branch and signer.
All JSON input is bounded. Status signing uses only the separately configured status profile.
--native-fixtures is an explicit leading switch available only in native-fixtures builds.
No command synchronizes repositories, activates outputs or grants release authority.";

fn emit(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string(value).map_err(text)?);
    std::io::stdout().flush().map_err(text)
}
fn service(path: &str, fixture: bool) -> Result<CiService> {
    let profile: CiProfile = load_json(Path::new(path), MAX_CONFIG_BYTES)?;
    #[cfg(feature = "native-fixtures")]
    if fixture {
        return CiService::new_fixture(profile);
    }
    if fixture {
        return Err("native fixture authority is not compiled into this binary".into());
    }
    CiService::new(profile)
}
fn source(path: &str) -> Result<SourceContext> {
    load_json(Path::new(path), MAX_CONFIG_BYTES)
}

fn broker_result(outcome: Outcome, written: bool) -> Result<DefeloResult> {
    if matches!(outcome, Outcome::Pending | Outcome::Unknown) {
        return Err("canonical run remains nonterminal; reconcile the original operation".into());
    }
    if !written {
        return Err("status remains uncertain; reconcile the reserved original intent".into());
    }
    Ok(if outcome == Outcome::Success {
        DefeloResult::Success
    } else {
        DefeloResult::Failure
    })
}

// r[impl kiln.radicle_ci.journal] r[impl kiln.radicle_ci.status]
fn broker(service: &CiService) -> Result<()> {
    let mut input = String::new();
    std::io::stdin()
        .take((MAX_DEFELO_REQUEST_BYTES + 1) as u64)
        .read_to_string(&mut input)
        .map_err(text)?;
    let trigger = map_defelo_request(&parse_defelo_request(&input).map_err(text)?).map_err(text)?;
    let accepted = service.accept(trigger)?;
    let bindings = service
        .worker_profile()
        .bindings(&service.profile().worker_profile_identity, &accepted.source)?;
    emit(&DefeloResponse::Triggered {
        run_id: DefeloRunId {
            id: bindings.run_id.as_str(),
        },
        info_url: None,
    })?;
    let journal = service.observe(&accepted.source)?;
    let state = service.replay(&accepted.source)?;
    let outcome = state
        .runs
        .get(&bindings.run_id)
        .ok_or("canonical run missing")?
        .outcome;
    let written = matches!(journal.status_observation, Some(StatusDispatch::Written(_)));
    let result = broker_result(outcome, written)?;
    emit(&DefeloResponse::Finished { result })
}

fn observation(arguments: &[&str], fixture: bool) -> Result<()> {
    match arguments {
        ["serve", profile, context, http] => {
            let service = service(profile, fixture)?;
            let (journal, state) = service.load_replayed(&source(context)?)?;
            let snapshot = ObservationSnapshot::new(state).with_events(EventWindow {
                records: journal.records,
            });
            kiln_onixos_pilot::observation::serve_snapshot(
                snapshot,
                load_json(Path::new(http), MAX_CONFIG_BYTES)?,
            )
        }
        ["ucan-init", template, profile, context, directory] => {
            let service = service(profile, fixture)?;
            let source = source(context)?;
            let state = service.replay(&source)?;
            let bindings = service
                .worker_profile()
                .bindings(&service.profile().worker_profile_identity, &source)?;
            kiln_onixos_pilot::probe::init_for_run(
                Path::new(template),
                &state,
                &bindings.run_id,
                Path::new(directory),
            )
        }
        ["ucan-probe", profile, context, directory] => {
            let service = service(profile, fixture)?;
            let source = source(context)?;
            let state = service.replay(&source)?;
            let bindings = service
                .worker_profile()
                .bindings(&service.profile().worker_profile_identity, &source)?;
            emit(&kiln_onixos_pilot::probe::run_for_attempt(
                &state,
                &bindings.run_id,
                &bindings.attempt_id,
                Path::new(directory),
            )?)
        }
        _ => Err(USAGE.into()),
    }
}

fn run() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (fixture, arguments) = match arguments.as_slice() {
        [flag, rest @ ..] if flag == "--native-fixtures" => (true, rest),
        args => (false, args),
    };
    let arguments = arguments.iter().map(String::as_str).collect::<Vec<_>>();
    match arguments.as_slice() {
        ["--profile", profile] => broker(&service(profile, fixture)?),
        ["start", profile, trigger] => emit(
            &service(profile, fixture)?.accept(load_json(Path::new(trigger), MAX_CONFIG_BYTES)?)?,
        ),
        ["reconcile", profile, context] => {
            emit(&service(profile, fixture)?.reconcile(&source(context)?)?)
        }
        ["observe", profile, context] => {
            emit(&service(profile, fixture)?.observe(&source(context)?)?)
        }
        ["replay", profile, context] => {
            emit(&service(profile, fixture)?.replay(&source(context)?)?)
        }
        ["bindings", profile, context] => {
            let service = service(profile, fixture)?;
            emit(&service.worker_profile().bindings(
                &service.profile().worker_profile_identity,
                &source(context)?,
            )?)
        }
        ["profile-identity", input] => {
            let profile: CiProfile = load_json(Path::new(input), MAX_CONFIG_BYTES)?;
            profile.admit()?;
            emit(&identity(&profile)?)
        }
        [] | ["help"] | ["--help"] => {
            println!("{USAGE}");
            Ok(())
        }
        args => observation(args, fixture),
    }
}
fn text(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
fn main() {
    if let Err(error) = run() {
        eprintln!("kiln.radicle_ci.failure: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify kiln.radicle_ci.recovery] r[verify kiln.radicle_ci.status]
    #[test]
    fn uncertain_canonical_or_status_truth_cannot_finish_a_broker_run() {
        for outcome in [Outcome::Pending, Outcome::Unknown] {
            for written in [false, true] {
                assert!(broker_result(outcome, written).is_err());
            }
        }
        assert!(broker_result(Outcome::Success, false).is_err());
        assert!(matches!(
            broker_result(Outcome::Success, true),
            Ok(DefeloResult::Success)
        ));
        for outcome in [
            Outcome::DomainFailure,
            Outcome::ProviderError,
            Outcome::TimedOut,
            Outcome::Cancelled,
            Outcome::Denied,
        ] {
            assert!(broker_result(outcome, false).is_err());
            assert!(matches!(
                broker_result(outcome, true),
                Ok(DefeloResult::Failure)
            ));
        }
    }
}
