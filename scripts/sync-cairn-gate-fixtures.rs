#!/usr/bin/env -S CARGO_TARGET_DIR=/tmp/kiln-sync-cairn-target nix shell "github:nix-community/fenix?rev=092bd452904e749efa39907aa4a20a42678ac31e#minimal.toolchain" nixpkgs#gcc nixpkgs#clang nixpkgs#mold -c cargo -q -Zscript

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const EXPECTED_ARGUMENT_COUNT: usize = 2;
const EXPECTED_CHECK_ARGUMENT_COUNT: usize = 3;
const SOURCE_ARGUMENT_INDEX: usize = 1;
const MODE_ARGUMENT_INDEX: usize = 2;
const CHECK_MODE: &str = "--check";
const EXPECTED_REVISION: &str = "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f";
const SOURCE_URL: &str = "ssh://git@github.com/OnixResearch/cairn.git";
const SOURCE_FIXTURE: &str = "fixtures/cairn-gate-contract/producer/fixtures-v1.json";
const SOURCE_CONTRACT: &str = "contracts/cairn-gate-contract.ncl";
const SOURCE_MANIFEST: &str = "crates/cairn-gate-contract/Cargo.toml";
const SOURCE_LICENSE_MIT: &str = "crates/cairn-gate-contract/LICENSE-MIT";
const SOURCE_LICENSE_APACHE: &str = "crates/cairn-gate-contract/LICENSE-APACHE";
const DESTINATION_DIRECTORY: &str =
    "fixtures/cairn/gate-contract/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f";
const DESTINATION_FIXTURE: &str = "producer-fixtures-v1.json";
const DESTINATION_CONTRACT: &str = "contract.ncl";
const DESTINATION_METADATA: &str = "source.ncl";
const MAXIMUM_FIXTURE_BYTES: u64 = 1_048_576;
const MAXIMUM_CONTRACT_BYTES: u64 = 1_048_576;
const GIT_PROGRAM: &str = "git";
const FAILURE_EXIT_CODE: u8 = 1;

fn main() -> ExitCode {
    match run(&std::env::args().collect::<Vec<_>>()) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("fixture sync failed: {message}");
            ExitCode::from(FAILURE_EXIT_CODE)
        }
    }
}

fn run(arguments: &[String]) -> Result<String, String> {
    if arguments.len() != EXPECTED_ARGUMENT_COUNT
        && arguments.len() != EXPECTED_CHECK_ARGUMENT_COUNT
    {
        return Err("usage: sync-cairn-gate-fixtures.rs SOURCE_ROOT [--check]".to_string());
    }
    let check_only = arguments
        .get(MODE_ARGUMENT_INDEX)
        .is_some_and(|mode| mode == CHECK_MODE);
    if arguments.len() == EXPECTED_CHECK_ARGUMENT_COUNT && !check_only {
        return Err("the optional mode must be --check".to_string());
    }
    let source_root = Path::new(&arguments[SOURCE_ARGUMENT_INDEX]);
    verify_revision(source_root)?;
    verify_source_contract(source_root)?;
    let fixture = read_bounded(&source_root.join(SOURCE_FIXTURE), MAXIMUM_FIXTURE_BYTES)?;
    let contract = read_bounded(&source_root.join(SOURCE_CONTRACT), MAXIMUM_CONTRACT_BYTES)?;
    let metadata = metadata();
    let destination_root = PathBuf::from(DESTINATION_DIRECTORY);
    let destination_fixture = destination_root.join(DESTINATION_FIXTURE);
    let destination_contract = destination_root.join(DESTINATION_CONTRACT);
    let destination_metadata = destination_root.join(DESTINATION_METADATA);
    if check_only {
        compare(&destination_fixture, &fixture)?;
        compare(&destination_contract, &contract)?;
        compare(&destination_metadata, metadata.as_bytes())?;
        return Ok("Cairn gate fixtures match the pinned producer revision".to_string());
    }
    fs::create_dir_all(&destination_root)
        .map_err(|error| format!("create destination: {error}"))?;
    fs::write(&destination_fixture, fixture).map_err(|error| format!("write fixture: {error}"))?;
    fs::write(&destination_contract, contract)
        .map_err(|error| format!("write contract: {error}"))?;
    fs::write(&destination_metadata, metadata)
        .map_err(|error| format!("write metadata: {error}"))?;
    Ok("synchronized Cairn gate fixtures".to_string())
}

fn verify_revision(source_root: &Path) -> Result<(), String> {
    let output = Command::new(GIT_PROGRAM)
        .args(["-C"])
        .arg(source_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| format!("run git: {error}"))?;
    if !output.status.success() {
        return Err("source is not a readable Git checkout".to_string());
    }
    let revision =
        String::from_utf8(output.stdout).map_err(|_| "source revision is not UTF-8".to_string())?;
    if revision.trim() != EXPECTED_REVISION {
        return Err(format!(
            "source revision {} does not match {EXPECTED_REVISION}",
            revision.trim()
        ));
    }
    Ok(())
}

fn verify_source_contract(source_root: &Path) -> Result<(), String> {
    let manifest = fs::read_to_string(source_root.join(SOURCE_MANIFEST))
        .map_err(|error| format!("read source manifest: {error}"))?;
    let mit = fs::read_to_string(source_root.join(SOURCE_LICENSE_MIT))
        .map_err(|error| format!("read MIT license: {error}"))?;
    let apache = fs::read_to_string(source_root.join(SOURCE_LICENSE_APACHE))
        .map_err(|error| format!("read Apache license: {error}"))?;
    if !manifest.contains("name = \"cairn-gate-contract\"")
        || !manifest.contains("license.workspace = true")
        || !mit.starts_with("MIT License")
        || !apache.contains("Apache License")
    {
        return Err("source contract identity or license is not the reviewed cohort".to_string());
    }
    Ok(())
}

fn read_bounded(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("read source metadata: {error}"))?;
    if !metadata.is_file() || metadata.len() > maximum_bytes {
        return Err("source artifact is missing, non-regular, or oversized".to_string());
    }
    fs::read(path).map_err(|error| format!("read source artifact: {error}"))
}

fn compare(path: &Path, expected: &[u8]) -> Result<(), String> {
    let observed = fs::read(path).map_err(|error| format!("read checked destination: {error}"))?;
    if observed != expected {
        return Err(format!(
            "{} differs from the pinned producer",
            path.display()
        ));
    }
    Ok(())
}

fn metadata() -> String {
    format!(
        r#"let Source = {{
  schema | String,
  repository | String,
  revision | String,
  crate | String,
  license | String,
  fixture | String,
  contract | String,
  non_claims | Array String,
}} in
({{
  schema = "kiln.cairn-gate-fixture-source.v1",
  repository = "{SOURCE_URL}",
  revision = "{EXPECTED_REVISION}",
  crate = "cairn-gate-contract",
  license = "MIT OR Apache-2.0",
  fixture = "producer-fixtures-v1.json",
  contract = "contract.ncl",
  non_claims = [
    "fixture identity does not prove lifecycle gate correctness",
    "producer fixtures do not grant executable, filesystem, or policy authority",
    "fixture admission does not authorize merge, publication, or release execution",
  ],
}} | Source)
"#
    )
}
