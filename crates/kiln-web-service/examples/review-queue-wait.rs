//! Off-by-default review-queue wait fixture (#61). Runs one frozen synthetic
//! load through the real review queue in every declared mode and writes a
//! `kiln.review-queue-wait.receipt.v1` receipt. It never runs inside a
//! service and changes no admission, queue or budget default.
//!
//! ```console
//! cargo run -p kiln-web-service --example review-queue-wait -- \
//!     --load fixtures/review-queue-wait/load.json --out receipt.json
//! ```
#[path = "review_queue_wait/harness.rs"]
mod harness;

use std::io::Write;
use std::process::ExitCode;

use kiln_web_service::code_review::queue_wait::{Load, Measurement, Receipt};

/// Largest load file the fixture reads.
const MAX_LOAD_BYTES: u64 = 64 * 1024;

fn arguments() -> Result<(String, String), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [load_flag, load, out_flag, out] if load_flag == "--load" && out_flag == "--out" => {
            Ok((load.clone(), out.clone()))
        }
        _ => Err("usage: review-queue-wait --load LOAD.json --out RECEIPT.json".into()),
    }
}

fn run() -> harness::Result {
    let (load_path, out_path) = arguments()?;
    let metadata = std::fs::symlink_metadata(&load_path)?;
    if !metadata.is_file() || metadata.len() > MAX_LOAD_BYTES {
        return Err("the load must be a bounded ordinary file".into());
    }
    let (load, identity) = Load::admit(&std::fs::read(&load_path)?)?;
    let receipt = Receipt::new(&load, harness::run(&load)?)?;
    let mut bytes = serde_json::to_vec_pretty(&receipt)?;
    bytes.push(b'\n');
    // A retained receipt is never overwritten.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&out_path)?
        .write_all(&bytes)?;
    if let Measurement::Measured { value } = &receipt.series.review_queue_wait.measurement {
        for arm in &value.arms {
            let counts = arm.summary.outcomes;
            println!(
                "{:?}: submitted {}, waiting {}, rejected {}, expired {}, failed {}, unknown {}, cancelled {}",
                arm.mode.mode,
                counts.submitted,
                counts.waiting,
                counts.rejected,
                counts.expired,
                counts.failed,
                counts.unknown,
                counts.cancelled
            );
        }
    }
    println!("load {identity}; receipt {out_path}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("review-queue-wait: {error}");
            ExitCode::FAILURE
        }
    }
}
