//! Operator-only execution into the existing canonical CI engine, never report import.
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use kiln_core::implementation::{CandidateVerification, Enrollment};
use kiln_core::types::ContentId;
use kiln_implementer::Implementer;
use kiln_implementer::verification::{VerificationProfile, VerifierAuthority};
use kiln_web_service::{engine::Engine, profile::ServiceProfile};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const USAGE: &str = "usage: kiln-web-service verify-implementation --profile CI_SERVICE_PROFILE --database DATABASE --enrollment ENROLLMENT --attempt ID --fence N --verification-profile PROFILE --authority AUTHORITY [--local-operator OPERATOR | --authority-config AUTHORITY_CONFIG]";

#[derive(Default)]
struct Fields {
    service: Option<String>,
    database: Option<String>,
    enrollment: Option<String>,
    attempt: Option<String>,
    fence: Option<String>,
    verification: Option<String>,
    authority: Option<String>,
    local_operator: Option<String>,
    authority_config: Option<String>,
}

struct Inputs {
    service: PathBuf,
    database: PathBuf,
    enrollment: PathBuf,
    attempt: ContentId,
    fence: u64,
    verification: PathBuf,
    authority: PathBuf,
    local_operator: Option<String>,
    authority_config: Option<PathBuf>,
}

fn required(value: &mut Option<String>, flag: &str) -> Result<String, String> {
    value
        .take()
        .ok_or_else(|| format!("missing required {flag}"))
}

fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Inputs, String> {
    let mut fields = Fields::default();
    while let Some(flag) = arguments.next() {
        let slot = match flag.as_str() {
            "--profile" => &mut fields.service,
            "--database" => &mut fields.database,
            "--enrollment" => &mut fields.enrollment,
            "--attempt" => &mut fields.attempt,
            "--fence" => &mut fields.fence,
            "--verification-profile" => &mut fields.verification,
            "--authority" => &mut fields.authority,
            "--local-operator" => &mut fields.local_operator,
            "--authority-config" => &mut fields.authority_config,
            _ => return Err(format!("unsupported verification option: {flag}")),
        };
        if slot.is_some() {
            return Err(format!("duplicate verification option: {flag}"));
        }
        *slot = Some(
            arguments
                .next()
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| format!("missing value for {flag}"))?,
        );
    }
    let attempt = ContentId::from_blake3_hex(required(&mut fields.attempt, "--attempt")?)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let fence = required(&mut fields.fence, "--fence")?
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or("fence must be a positive u64")?;
    Ok(Inputs {
        service: required(&mut fields.service, "--profile")?.into(),
        database: required(&mut fields.database, "--database")?.into(),
        enrollment: required(&mut fields.enrollment, "--enrollment")?.into(),
        attempt,
        fence,
        verification: required(&mut fields.verification, "--verification-profile")?.into(),
        authority: required(&mut fields.authority, "--authority")?.into(),
        local_operator: fields.local_operator,
        authority_config: fields.authority_config.map(PathBuf::from),
    })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let file =
        File::open(path).map_err(|error| format!("verification input unavailable: {error}"))?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("verification input must be a regular file".to_owned());
    }
    let mut reader = BufReader::new(file.take(MAXIMUM_INPUT_BYTES + 1));
    let value = serde_json::from_reader(&mut reader)
        .map_err(|error| format!("verification input invalid: {error}"))?;
    if reader.get_ref().limit() == 0 {
        return Err("verification input exceeds its byte bound".to_owned());
    }
    Ok(value)
}

// r[impl kiln.implementation.verification]
fn execute(inputs: Inputs) -> Result<CandidateVerification, String> {
    let service = ServiceProfile::load(&inputs.service)?;
    let enrollment: Enrollment = read_json(&inputs.enrollment)?;
    let verification: VerificationProfile = read_json(&inputs.verification)?;
    let authority: VerifierAuthority = read_json(&inputs.authority)?;
    let mut ledger = Implementer::open_with_authority(
        &inputs.database,
        enrollment,
        inputs.local_operator.as_deref(),
        inputs.authority_config.as_deref(),
    )?;
    let mut engine = Engine::open(service)?;
    engine.verify_implementation_candidate(
        &mut ledger,
        &inputs.attempt,
        inputs.fence,
        &verification,
        &authority,
    )
}

pub(super) fn run(mut arguments: impl Iterator<Item = String>) -> i32 {
    let first = arguments.next();
    if matches!(first.as_deref(), Some("--help" | "-h")) && arguments.next().is_none() {
        println!("{USAGE}");
        return 0;
    }
    let inputs = match parse(first.into_iter().chain(arguments)) {
        Ok(inputs) => inputs,
        Err(message) => {
            eprintln!("kiln-web-service verify-implementation: {message}\n{USAGE}");
            return 2;
        }
    };
    let result = execute(inputs).and_then(|verification| {
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        serde_json::to_writer(&mut output, &verification).map_err(|error| error.to_string())?;
        writeln!(output).map_err(|error| error.to_string())
    });
    match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("kiln-web-service verify-implementation: {message}");
            1
        }
    }
}
