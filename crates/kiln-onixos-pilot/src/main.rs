use kiln_onixos_pilot::{
    MAX_CONFIG_BYTES, OperatorRequest, PilotProfile, Result, SignedRequest, identity, keygen,
    load_json, now_unix, prepare_request, publish_new, reconcile, replay, sign, start,
};
use std::path::Path;

const USAGE: &str = "kiln-onixos-pilot commands:
  keygen SECRET_JSON PUBLIC_JSON
  runtime-profile ASPEN_DOCUMENT_JSON
  native-workflow RADICLE_PROFILE_JSON TRIGGER_JSON [EFFECT_ID]
  profile-identity PROFILE_JSON
  prepare-request PROFILE_JSON TRIGGER_JSON ISSUED_UNIX EXPIRES_UNIX REQUEST_JSON
  sign SECRET_JSON REQUEST_JSON SIGNED_JSON
  start PROFILE_JSON SIGNED_JSON JOURNAL_JSON
  reconcile PROFILE_JSON JOURNAL_JSON [WORKER_RESULT_JSON]
  replay PROFILE_JSON JOURNAL_JSON
  serve PROFILE_JSON JOURNAL_JSON HTTP_PROFILE_JSON
  ucan-init HTTP_TEMPLATE_JSON PROFILE_JSON JOURNAL_JSON PRIVATE_DIRECTORY
  ucan-probe PROFILE_JSON JOURNAL_JSON PRIVATE_DIRECTORY
  now
All files are bounded JSON. New files refuse replacement; the journal alone is atomically updated.
Operator/public keys are base64url raw Ed25519 bytes wrapped as JSON strings.
ucan-init creates PRIVATE_DIRECTORY/{credentials.json,http.json}; serve uses that http.json.
No command publishes remote status, releases worker roots, or stops an external service.";

fn profile(path: &str) -> Result<PilotProfile> {
    load_json(Path::new(path), MAX_CONFIG_BYTES)
}
fn emit<T: serde::Serialize>(value: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|error| error.to_string())?
    );
    Ok(())
}
fn native_workflow(config: &str, trigger: &str, effect_id: Option<&str>) -> Result<()> {
    let config: kiln_adapter_radicle::RadicleShellConfig =
        load_json(Path::new(config), MAX_CONFIG_BYTES)?;
    let admitted = kiln_adapter_radicle::admit_config(config)
        .map_err(|error| format!("Radicle config: {error:?}"))?;
    let mut record = kiln_adapter_radicle::plan_trigger(
        &admitted.shell,
        load_json(Path::new(trigger), MAX_CONFIG_BYTES)?,
    )
    .map_err(|error| format!("Radicle workflow plan: {error:?}"))?;
    if let Some(effect_id) = effect_id {
        record.plan.effect_id = kiln_core::EffectId::new(effect_id)
            .map_err(|error| format!("native callback effect: {error:?}"))?;
    }
    emit(
        &kiln_adapter_lattice::lower_request(&admitted.lattice, &record.plan)
            .map_err(|error| format!("native Lattice request: {error:?}"))?,
    )
}
fn run() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["keygen", secret, public] => keygen(Path::new(secret), Path::new(public)),
        ["runtime-profile", input] => {
            let document: kiln_adapter_aspen::AspenRuntimeProfileDocument =
                load_json(Path::new(input), MAX_CONFIG_BYTES)?;
            emit(
                &document
                    .into_runtime_profile()
                    .map_err(|error| format!("runtime profile document: {error:?}"))?,
            )
        }
        ["native-workflow", config, trigger] => native_workflow(config, trigger, None),
        ["native-workflow", config, trigger, effect_id] => {
            native_workflow(config, trigger, Some(effect_id))
        }
        ["profile-identity", input] => {
            let profile = profile(input)?;
            profile.admit()?;
            emit(&identity(&profile)?)
        }
        ["prepare-request", input, trigger, issued, expires, output] => {
            let request = prepare_request(
                &profile(input)?,
                load_json(Path::new(trigger), MAX_CONFIG_BYTES)?,
                issued.parse::<u64>().map_err(|error| error.to_string())?,
                expires.parse::<u64>().map_err(|error| error.to_string())?,
            )?;
            publish_new(Path::new(output), &request)
        }
        ["sign", secret, request, output] => {
            let request: OperatorRequest = load_json(Path::new(request), MAX_CONFIG_BYTES)?;
            publish_new(Path::new(output), &sign(Path::new(secret), request)?)
        }
        ["start", input, signed, journal] => {
            let profile = profile(input)?;
            let signed: SignedRequest = load_json(Path::new(signed), profile.max_request_bytes)?;
            emit(&start(&profile, signed, Path::new(journal))?)
        }
        ["reconcile", input, journal] => {
            emit(&reconcile(&profile(input)?, Path::new(journal), None)?)
        }
        ["reconcile", input, journal, worker] => emit(&reconcile(
            &profile(input)?,
            Path::new(journal),
            Some(Path::new(worker)),
        )?),
        ["replay", input, journal] => emit(&replay(&profile(input)?, Path::new(journal))?),
        ["serve", input, journal, http] => kiln_onixos_pilot::observation::serve(
            &profile(input)?,
            Path::new(journal),
            load_json(Path::new(http), MAX_CONFIG_BYTES)?,
        ),
        ["ucan-init", template, input, journal, directory] => kiln_onixos_pilot::probe::init(
            Path::new(template),
            &profile(input)?,
            Path::new(journal),
            Path::new(directory),
        ),
        ["ucan-probe", input, journal, directory] => emit(&kiln_onixos_pilot::probe::run(
            &profile(input)?,
            Path::new(journal),
            Path::new(directory),
        )?),
        ["now"] => emit(&now_unix()?),
        ["help"] | ["--help"] | [] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("kiln.onixos_pilot.failure: {error}");
        std::process::exit(1);
    }
}
