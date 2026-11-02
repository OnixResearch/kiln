//! Read-only Nix binary-cache projection command.

use std::path::PathBuf;

use kiln_nix_store::cache::{CacheProjection, render};

const MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return usage();
    };
    if command != "render" {
        return usage();
    }
    let mut document: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--document" => document = args.next().map(PathBuf::from),
            "--output" => output = args.next().map(PathBuf::from),
            _ => return usage(),
        }
    }
    let (Some(document), Some(output)) = (document, output) else {
        return usage();
    };
    let bytes = match std::fs::read(&document) {
        Ok(bytes) if u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= MAX_DOCUMENT_BYTES => bytes,
        Ok(_) => {
            eprintln!("kiln-nix-cache: projection document exceeds its byte bound");
            return std::process::ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!(
                "kiln-nix-cache: cannot read {}: {error}",
                document.display()
            );
            return std::process::ExitCode::FAILURE;
        }
    };
    let projection: CacheProjection = match serde_json::from_slice(&bytes) {
        Ok(projection) => projection,
        Err(error) => {
            eprintln!("kiln-nix-cache: projection document is invalid: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    match render(&projection, &output) {
        Ok(receipt) => {
            match serde_json::to_string_pretty(&receipt) {
                Ok(text) => println!("{text}"),
                Err(error) => {
                    eprintln!("kiln-nix-cache: receipt encoding failed: {error}");
                    return std::process::ExitCode::FAILURE;
                }
            }
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("kiln-nix-cache: {}: {}", error.code, error.message);
            std::process::ExitCode::FAILURE
        }
    }
}

fn usage() -> std::process::ExitCode {
    eprintln!("usage: kiln-nix-cache render --document <path> --output <absolute-directory>");
    std::process::ExitCode::FAILURE
}
