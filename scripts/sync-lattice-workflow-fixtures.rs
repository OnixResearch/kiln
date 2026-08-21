#!/usr/bin/env -S CARGO_TARGET_DIR=/tmp/kiln-sync-lattice-target nix shell "github:nix-community/fenix?rev=092bd452904e749efa39907aa4a20a42678ac31e#minimal.toolchain" nixpkgs#gcc nixpkgs#clang nixpkgs#mold -c cargo -q -Zscript

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const EXPECTED_ARGUMENT_COUNT: usize = 2;
const EXPECTED_CHECK_ARGUMENT_COUNT: usize = 3;
const SOURCE_ARGUMENT_INDEX: usize = 1;
const MODE_ARGUMENT_INDEX: usize = 2;
const CHECK_MODE: &str = "--check";
const EXPECTED_REVISION: &str = "70496e67c7fd4a8b05914161a8e09de2759bebc8";
const SOURCE_URL: &str = "ssh://git@github.com/OnixResearch/lattice.git";
const SOURCE_FIXTURE: &str = "crates/lattice-workflow-contract/fixtures/producer-fixtures-v1.json";
const SOURCE_TRANSPORT_FIXTURE: &str =
    "crates/lattice-workflow-contract/transport/fixtures/producer-fixtures-v1.json";
const SOURCE_RECOVERY_FIXTURE: &str =
    "crates/lattice-workflow-contract/transport/fixtures/recovery-fixtures-v1.json";
const SOURCE_LICENSE: &str = "crates/lattice-workflow-contract/LICENSE";
const SOURCE_MANIFEST: &str = "crates/lattice-workflow-contract/Cargo.toml";
const SOURCE_TRANSPORT_MANIFEST: &str = "crates/lattice-workflow-contract/transport/Cargo.toml";
const DESTINATION_DIRECTORY: &str =
    "fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8";
const DESTINATION_FIXTURE: &str = "producer-fixtures-v1.json";
const DESTINATION_TRANSPORT_FIXTURE: &str = "transport-producer-fixtures-v1.json";
const DESTINATION_RECOVERY_FIXTURE: &str = "recovery-fixtures-v1.json";
const DESTINATION_METADATA: &str = "source.ncl";
const MAXIMUM_FIXTURE_BYTES: u64 = 1_048_576;
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
        return Err("usage: sync-lattice-workflow-fixtures.rs SOURCE_ROOT [--check]".to_string());
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
    let fixture = read_bounded(&source_root.join(SOURCE_FIXTURE))?;
    let transport_fixture = read_bounded(&source_root.join(SOURCE_TRANSPORT_FIXTURE))?;
    let recovery_fixture = read_bounded(&source_root.join(SOURCE_RECOVERY_FIXTURE))?;
    let metadata = metadata();
    let destination_root = PathBuf::from(DESTINATION_DIRECTORY);
    let destination_fixture = destination_root.join(DESTINATION_FIXTURE);
    let destination_transport_fixture = destination_root.join(DESTINATION_TRANSPORT_FIXTURE);
    let destination_recovery_fixture = destination_root.join(DESTINATION_RECOVERY_FIXTURE);
    let destination_metadata = destination_root.join(DESTINATION_METADATA);
    if check_only {
        compare(&destination_fixture, &fixture)?;
        compare(&destination_transport_fixture, &transport_fixture)?;
        compare(&destination_recovery_fixture, &recovery_fixture)?;
        compare(&destination_metadata, metadata.as_bytes())?;
        return Ok("Lattice workflow fixtures match the pinned producer revision".to_string());
    }
    fs::create_dir_all(&destination_root)
        .map_err(|error| format!("create destination: {error}"))?;
    fs::write(&destination_fixture, fixture).map_err(|error| format!("write fixture: {error}"))?;
    fs::write(&destination_transport_fixture, transport_fixture)
        .map_err(|error| format!("write transport fixture: {error}"))?;
    fs::write(&destination_recovery_fixture, recovery_fixture)
        .map_err(|error| format!("write recovery fixture: {error}"))?;
    fs::write(&destination_metadata, metadata)
        .map_err(|error| format!("write metadata: {error}"))?;
    Ok("synchronized Lattice workflow fixtures".to_string())
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
    let license = fs::read_to_string(source_root.join(SOURCE_LICENSE))
        .map_err(|error| format!("read source license: {error}"))?;
    let manifest = fs::read_to_string(source_root.join(SOURCE_MANIFEST))
        .map_err(|error| format!("read source manifest: {error}"))?;
    let transport_manifest = fs::read_to_string(source_root.join(SOURCE_TRANSPORT_MANIFEST))
        .map_err(|error| format!("read transport manifest: {error}"))?;
    if !license.starts_with("MIT License")
        || !manifest.contains("name = \"lattice-workflow-contract\"")
        || !manifest.contains("license = \"MIT\"")
        || !transport_manifest.contains("name = \"lattice-workflow-transport\"")
        || !transport_manifest.contains("license = \"MIT\"")
    {
        return Err("source contract identity or license is not the reviewed cohort".to_string());
    }
    Ok(())
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("read fixture metadata: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_FIXTURE_BYTES {
        return Err("producer fixture is missing, non-regular, or oversized".to_string());
    }
    fs::read(path).map_err(|error| format!("read fixture: {error}"))
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
  non_claims | Array String,
}} in
({{
  schema = "kiln.lattice-workflow-fixture-source.v1",
  repository = "{SOURCE_URL}",
  revision = "{EXPECTED_REVISION}",
  crate = "lattice-workflow-contract",
  license = "MIT",
  fixture = "producer-fixtures-v1.json",
  non_claims = [
    "fixture identity does not prove workflow execution",
    "producer fixtures do not grant Lattice availability or authority",
    "transport recovery fixtures do not prove durable lookup or non-execution",
  ],
}} | Source)
"#
    )
}
