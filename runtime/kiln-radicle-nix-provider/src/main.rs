#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the binary composition root keeps explicit provider inputs and output ownership visible"
)]
#![allow(
    tigerstyle::ambient_env,
    reason = "the binary shell captures argv once and passes explicit values into provider orchestration"
)]

use std::io::Write;
use std::path::PathBuf;

use kiln_radicle_nix_provider::{
    HostedProviderRequest, execute_hosted_nix_provider, load_hosted_provider_profile,
};

const INPUT_ERROR_EXIT_CODE: i32 = 1;
const PROVIDER_ERROR_EXIT_CODE: i32 = 2;
const CHECK_FAILED_EXIT_CODE: i32 = 3;
const PROFILE_ARGUMENT: &str = "--profile";
const REPOSITORY_ARGUMENT: &str = "--repository";
const REVISION_ARGUMENT: &str = "--revision";
const MAXIMUM_ARGUMENT_PAIRS: usize = 3;

#[derive(Debug)]
struct Arguments {
    profile: PathBuf,
    repository: String,
    revision: String,
}

fn parse_arguments() -> Result<Arguments, &'static str> {
    let mut values = std::env::args().skip(1);
    let mut profile = None;
    let mut repository = None;
    let mut revision = None;
    for _argument_index in 0..MAXIMUM_ARGUMENT_PAIRS {
        let Some(argument) = values.next() else {
            break;
        };
        match argument.as_str() {
            PROFILE_ARGUMENT => set_once(&mut profile, values.next(), "profile")?,
            REPOSITORY_ARGUMENT => set_once(&mut repository, values.next(), "repository")?,
            REVISION_ARGUMENT => set_once(&mut revision, values.next(), "revision")?,
            _ => return Err("unknown provider argument"),
        }
    }
    if values.next().is_some() {
        return Err("provider argument count exceeds the closed interface");
    }
    Ok(Arguments {
        profile: PathBuf::from(profile.ok_or("--profile is required")?),
        repository: repository.ok_or("--repository is required")?,
        revision: revision.ok_or("--revision is required")?,
    })
}

fn set_once(
    field: &mut Option<String>,
    value: Option<String>,
    label: &'static str,
) -> Result<(), &'static str> {
    if field.is_some() {
        return Err(match label {
            "profile" => "profile argument is duplicated",
            "repository" => "repository argument is duplicated",
            "revision" => "revision argument is duplicated",
            _ => "provider argument is duplicated",
        });
    }
    *field = Some(value.ok_or("provider argument value is missing")?);
    Ok(())
}

fn run() -> Result<bool, String> {
    let arguments = parse_arguments().map_err(str::to_string)?;
    let profile =
        load_hosted_provider_profile(&arguments.profile).map_err(|error| error.to_string())?;
    let receipt = execute_hosted_nix_provider(
        &profile,
        &HostedProviderRequest {
            repository: arguments.repository,
            revision: arguments.revision,
        },
    )
    .map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&receipt).map_err(|_| "provider receipt encoding failed")?;
    std::io::stdout()
        .write_all(&bytes)
        .and_then(|()| std::io::stdout().write_all(b"\n"))
        .map_err(|error| format!("provider receipt write failed: {error}"))?;
    Ok(receipt.outcome.is_success())
}

fn main() {
    match run() {
        Ok(true) => {}
        Ok(false) => std::process::exit(CHECK_FAILED_EXIT_CODE),
        Err(error) => {
            eprintln!("{error}");
            let code = if error.contains("argument") {
                INPUT_ERROR_EXIT_CODE
            } else {
                PROVIDER_ERROR_EXIT_CODE
            };
            std::process::exit(code);
        }
    }
}
