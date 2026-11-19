//! Check a bounded Cargo normal-library dependency observation, not authority.
use std::io::Read;
use std::io::{self};
use std::process::ExitCode;

const MAX_BYTES: u64 = 65_536;
const FORBIDDEN: [&str; 2] = ["replay_ledger_core", "revocation_view_core"];

// r[impl ucan.durable_profile.verification]
fn validate(text: &str) -> Result<(), &'static str> {
    let names: Vec<_> = text.lines().filter_map(|line| line.split_whitespace().next()).collect();
    if !names.contains(&"ucan") || !names.contains(&"ucan_core") {
        return Err("minimal observation lacks UCAN or its core");
    }
    if FORBIDDEN.iter().any(|name| names.contains(name)) {
        return Err("minimal observation contains a durable-state adapter dependency");
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut text = String::new();
    if io::stdin().take(MAX_BYTES + 1).read_to_string(&mut text).is_err() || text.len() as u64 > MAX_BYTES {
        eprintln!("invalid or oversized dependency observation");
        return ExitCode::FAILURE;
    }
    match validate(&text) {
        Ok(()) => {
            println!("minimal dependency profile: PASS (not authority or durability)");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate;

    #[test]
    fn admits_present_verification_libraries() {
        assert!(validate("ucan\nucan_core\nserde\nucan_core (*)\n").is_ok());
    }

    #[test]
    fn rejects_missing_and_forbidden_libraries() {
        for text in ["", "ucan", "ucan_core", "not_ucan\nnot_ucan_core"] {
            assert!(validate(text).is_err());
        }
        for forbidden in super::FORBIDDEN {
            assert!(validate(&format!("ucan\nucan_core\n{forbidden}\n")).is_err());
        }
    }
}
