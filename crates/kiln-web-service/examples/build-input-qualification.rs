//! Off-by-default build-input qualification of the projected-build corpus
//! (#77). Plans every case, materializes the planned views in a temporary
//! directory and writes a `kiln.build-input.qualification.receipt.v1`
//! receipt. With `--execute`, each planned Cargo command also runs in its
//! attempt overlay under a cleared environment, using `KILN_CARGO_EXE` or
//! `cargo` on `PATH`. It contacts no provider and changes no default.
//!
//! ```console
//! cargo run -p kiln-web-service --example build-input-qualification -- \
//!     --corpus fixtures/build-inputs/corpus.json --out receipt.json --execute
//! ```
#[path = "build_inputs/harness.rs"]
mod harness;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

struct Arguments {
    corpus: String,
    out: String,
    execute: bool,
}

fn arguments() -> Result<Arguments, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (rest, execute) = match args.as_slice() {
        [rest @ .., flag] if flag == "--execute" => (rest, true),
        rest => (rest, false),
    };
    match rest {
        [corpus_flag, corpus, out_flag, out]
            if corpus_flag == "--corpus" && out_flag == "--out" =>
        {
            Ok(Arguments {
                corpus: corpus.clone(),
                out: out.clone(),
                execute,
            })
        }
        _ => Err(
            "usage: build-input-qualification --corpus CORPUS.json --out RECEIPT.json [--execute]"
                .into(),
        ),
    }
}

fn cargo() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("KILN_CARGO_EXE") {
        return Ok(PathBuf::from(path));
    }
    let path = std::env::var_os("PATH").ok_or("PATH is unset")?;
    harness::on_path(&path, "cargo").ok_or_else(|| "cargo is not on PATH".to_string())
}

fn run() -> harness::Result {
    let arguments = arguments()?;
    let metadata = std::fs::symlink_metadata(&arguments.corpus)?;
    if !metadata.is_file() || metadata.len() > harness::MAX_CORPUS_BYTES {
        return Err("the corpus must be a bounded ordinary file".into());
    }
    let cargo = if arguments.execute {
        Some(cargo()?)
    } else {
        None
    };
    let scratch = tempfile::tempdir()?;
    let receipt = harness::run(
        &std::fs::read(&arguments.corpus)?,
        scratch.path(),
        cargo.as_deref(),
    )?;
    let mut bytes = serde_json::to_vec_pretty(&receipt)?;
    bytes.push(b'\n');
    // A retained receipt is never overwritten.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&arguments.out)?
        .write_all(&bytes)?;
    for case in &receipt.cases {
        let plan = match &case.plan {
            harness::PlanRecord::Planned { .. } => "planned",
            harness::PlanRecord::Refused { .. } => "refused",
        };
        let execution = match &case.execution {
            harness::Execution::Passed { .. } => "passed",
            harness::Execution::Failed { .. } => "failed",
            harness::Execution::TimedOut => "timed out",
            harness::Execution::NotRun { .. } => "not run",
        };
        println!("{}: {plan}, {execution}", case.id);
        if matches!(case.execution, harness::Execution::Failed { .. }) {
            let log = scratch
                .path()
                .join(harness::LOG_DIRECTORY)
                .join(format!("{}.log", case.id));
            let text = std::fs::read_to_string(log).unwrap_or_default();
            for line in text.lines().filter(|line| line.starts_with("error")) {
                println!("  {line}");
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("build-input-qualification: {error}");
            ExitCode::FAILURE
        }
    }
}
