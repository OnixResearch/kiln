//! Off-by-default local baseline of the shared projected-workspace corpus
//! (#82, #78). Materializes every projection of the frozen corpus through the
//! projected materializer in a temporary directory and writes a
//! `kiln.workspace-baseline.receipt.v1` receipt. It contacts no provider and
//! changes no default.
//!
//! ```console
//! cargo run -p kiln-web-service --example workspace-baseline -- \
//!     --corpus fixtures/workspace-corpus/corpus.json --out receipt.json
//! ```
#[path = "workspace_baseline/harness.rs"]
mod harness;

use std::io::Write;
use std::process::ExitCode;

fn arguments() -> Result<(String, String), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [corpus_flag, corpus, out_flag, out]
            if corpus_flag == "--corpus" && out_flag == "--out" =>
        {
            Ok((corpus.clone(), out.clone()))
        }
        _ => Err("usage: workspace-baseline --corpus CORPUS.json --out RECEIPT.json".into()),
    }
}

fn run() -> harness::Result {
    let (corpus_path, out_path) = arguments()?;
    let metadata = std::fs::symlink_metadata(&corpus_path)?;
    if !metadata.is_file() || metadata.len() > harness::MAX_CORPUS_BYTES {
        return Err("the corpus must be a bounded ordinary file".into());
    }
    let scratch = tempfile::tempdir()?;
    let receipt = harness::run(&std::fs::read(&corpus_path)?, scratch.path())?;
    let mut bytes = serde_json::to_vec_pretty(&receipt)?;
    bytes.push(b'\n');
    // A retained receipt is never overwritten.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&out_path)?
        .write_all(&bytes)?;
    for check in &receipt.checks {
        let local = match &check.local {
            harness::Outcome::Passed { .. } => "passed",
            harness::Outcome::Failed { .. } => "failed",
            harness::Outcome::NotRun { .. } => "not run",
        };
        println!("{}: {local}", check.check);
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("workspace-baseline: {error}");
            ExitCode::FAILURE
        }
    }
}
