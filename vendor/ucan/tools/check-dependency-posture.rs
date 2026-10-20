//! Checked dependency-advisory posture gate.
//!
//! This check runs against the committed `Cargo.lock` and the committed
//! cargo-deny policy. It never resolves a fresh dependency set. It validates:
//!
//! 1. the `deny.toml` policy rejects unapproved advisories and pins the two applicable advisories
//!    with an explicit non-exploitable rationale;
//! 2. the license section denies unlicensed packages;
//! 3. the workspace lockfile still contains the two pinned crates;
//! 4. the positive fixture passes the policy and the negative fixture fails it, proving the policy
//!    rejects a missing pin.
//!
//! The Nix `dependency-posture` check compiles and runs this file with rustc;
//! it has no crate dependencies.
//!
//! Usage: tools/check-dependency-posture.rs [--root PATH]

use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

const DENY_TOML_RELATIVE: &str = "deny.toml";
const FIXTURE_DIR_RELATIVE: &str = "cargo-deny/fixtures";
const POSITIVE_FIXTURE: &str = "positive-locked.deny.toml";
const NEGATIVE_FIXTURE: &str = "negative-missing-pin.deny.toml";
const LOCKFILE_RELATIVE: &str = "Cargo.lock";

const ADVISORIES_SECTION: &str = "[advisories]";
const LICENSES_SECTION: &str = "[licenses]";
const LICENSES_ALLOW_MARKER: &str = "allow = [";
const ADVISORY_PASTE: &str = "RUSTSEC-2024-0436";
const ADVISORY_ANYHOW: &str = "RUSTSEC-2026-0190";
const PASTE_CRATE: &str = "paste";
const ANYHOW_CRATE: &str = "anyhow";
const REQUIRED_LICENSE_IDS: [&str; 2] = ["MPL-2.0", "AGPL-3.0-or-later"];

fn main() {
    let root = parse_root();
    let checks = run_all(&root);
    let mut failures = 0_usize;
    for check in &checks {
        if check.0 {
            println!("PASS {}", check.1);
        } else {
            eprintln!("FAIL {}", check.1);
            failures += 1;
        }
    }
    if failures > 0 {
        std::process::exit(1);
    }
}

fn parse_root() -> PathBuf {
    let mut root = PathBuf::from(".");
    let args: Vec<String> = env::args().collect();
    if let Some(index) = args.iter().position(|arg| arg == "--root") {
        if let Some(value) = args.get(index + 1) {
            root = PathBuf::from(value);
        }
    }
    root
}

fn read_text(root: &Path, relative: &str, label: &str) -> String {
    let path = root.join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{label}: cannot read {}: {error}", path.display()))
}

fn pin_entry_has_reason_after(text: &str, id: &str) -> bool {
    let marker = format!("id = \"{id}\"");
    let Some(start) = text.find(&marker) else {
        return false;
    };
    let after = &text[start + marker.len()..];
    let end = after.find("id = \"").unwrap_or(after.len());
    after[..end].contains("reason = \"")
}

fn policy_accepts(text: &str) -> Result<(), String> {
    if !text.contains(ADVISORIES_SECTION) {
        return Err("advisories section missing".to_owned());
    }
    if !text.contains(LICENSES_SECTION) {
        return Err("licenses section missing".to_owned());
    }
    if !text.contains(LICENSES_ALLOW_MARKER) {
        return Err("licenses allow list missing".to_owned());
    }
    for license_id in REQUIRED_LICENSE_IDS {
        if !text.contains(license_id) {
            return Err(format!("license allow list does not cover workspace license {license_id}"));
        }
    }
    for required in [ADVISORY_PASTE, ADVISORY_ANYHOW] {
        let marker = format!("id = \"{required}\"");
        if !text.contains(&marker) {
            return Err(format!("advisory {required} is not pinned"));
        }
        if !pin_entry_has_reason_after(text, required) {
            return Err(format!("advisory {required} is pinned without an explicit rationale"));
        }
    }
    Ok(())
}

fn lockfile_has_crate(text: &str, crates: &[&str]) -> Result<(), String> {
    for crate_name in crates {
        let marker = format!("name = \"{crate_name}\"");
        if !text.contains(&marker) {
            return Err(format!("lockfile does not contain crate {crate_name}"));
        }
    }
    Ok(())
}

fn run_all(root: &Path) -> Vec<(bool, String)> {
    let policy_text = read_text(root, DENY_TOML_RELATIVE, "policy");
    let lock_text = read_text(root, LOCKFILE_RELATIVE, "lockfile");
    let fixture_dir = root.join(FIXTURE_DIR_RELATIVE);
    let positive = fs::read_to_string(fixture_dir.join(POSITIVE_FIXTURE))
        .unwrap_or_else(|error| panic!("fixture: cannot read positive fixture: {error}"));
    let negative = fs::read_to_string(fixture_dir.join(NEGATIVE_FIXTURE))
        .unwrap_or_else(|error| panic!("fixture: cannot read negative fixture: {error}"));

    let mut results = Vec::new();
    match policy_accepts(&policy_text) {
        Ok(()) => results.push((true, "policy accepts the locked advisory pins".to_owned())),
        Err(reason) => results.push((false, format!("policy is invalid: {reason}"))),
    }
    match lockfile_has_crate(&lock_text, &[PASTE_CRATE, ANYHOW_CRATE]) {
        Ok(()) => results.push((true, "lockfile binds the pinned crates".to_owned())),
        Err(reason) => results.push((false, format!("lockfile check failed: {reason}"))),
    }
    match policy_accepts(&positive) {
        Ok(()) => results.push((true, "positive fixture policy accepted".to_owned())),
        Err(reason) => results.push((false, format!("positive fixture policy rejected: {reason}"))),
    }
    match policy_accepts(&negative) {
        Ok(()) => results.push((false, "negative fixture must fail policy".to_owned())),
        Err(reason) => {
            if reason.contains(ADVISORY_PASTE) {
                results.push((true, "negative fixture fails policy for the missing pin".to_owned()));
            } else {
                results.push((false, format!("negative fixture failed for the wrong reason: {reason}")));
            }
        }
    }
    results
}
