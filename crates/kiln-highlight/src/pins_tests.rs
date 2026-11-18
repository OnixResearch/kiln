//! The pinned grammar description matches the lock file and the manifest.
use std::collections::BTreeSet;

use crate::{GRAMMAR_PINS, GrammarPin, Language, grammar_package};

const LOCK: &str = include_str!("../../../Cargo.lock");
const MANIFEST: &str = include_str!("../Cargo.toml");

/// `(name, version, checksum)` of every `[[package]]` entry in a lock file.
fn packages(lock: &str) -> Vec<(&str, &str, Option<&str>)> {
    lock.split("[[package]]")
        .skip(1)
        .map(|entry| {
            let field = |key: &str| {
                entry.lines().find_map(|line| {
                    line.strip_prefix(key)?
                        .strip_prefix(" = \"")?
                        .strip_suffix('"')
                })
            };
            (
                field("name").unwrap_or_default(),
                field("version").unwrap_or_default(),
                field("checksum"),
            )
        })
        .collect()
}

/// A pin matches only one locked entry with its exact version and checksum.
fn locked(lock: &str, pin: &GrammarPin) -> Result<(), String> {
    let entries: Vec<_> = packages(lock)
        .into_iter()
        .filter(|(name, _, _)| *name == pin.package)
        .collect();
    match entries.as_slice() {
        [(_, version, checksum)] if *version == pin.version && *checksum == Some(pin.checksum) => {
            Ok(())
        }
        [(_, version, checksum)] => Err(format!(
            "{} is locked at {version} with checksum {checksum:?}",
            pin.package
        )),
        [] => Err(format!("{} is not locked", pin.package)),
        _ => Err(format!("{} is locked {} times", pin.package, entries.len())),
    }
}

// r[verify kiln.preview.structural]
#[test]
fn every_pin_matches_exactly_one_locked_package() {
    for pin in &GRAMMAR_PINS {
        assert_eq!(locked(LOCK, pin), Ok(()));
    }
}

// r[verify kiln.preview.structural]
#[test]
fn stale_missing_or_ambiguous_pins_are_refused() {
    let Some(rust) = GRAMMAR_PINS
        .iter()
        .find(|pin| pin.package == "tree-sitter-rust")
    else {
        unreachable!("tree-sitter-rust is pinned");
    };
    let entry = format!(
        "[[package]]\nname = \"{}\"\nversion = \"{}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
        rust.package, rust.version, rust.checksum
    );
    assert_eq!(locked(&format!("version = 4\n\n{entry}"), rust), Ok(()));
    let stale_version = GrammarPin {
        version: "0.24.1",
        ..*rust
    };
    let stale_checksum = GrammarPin {
        checksum: "0000000000000000000000000000000000000000000000000000000000000000",
        ..*rust
    };
    let unlocked = GrammarPin {
        package: "tree-sitter-unpinned",
        ..*rust
    };
    for (pin, lock) in [
        (stale_version, entry.clone()),
        (stale_checksum, entry.clone()),
        (unlocked, entry.clone()),
        (*rust, format!("{entry}\n{entry}")),
        (*rust, entry.replace("checksum", "# checksum")),
    ] {
        assert!(locked(&lock, &pin).is_err(), "{pin:?} matched {lock}");
    }
}

// r[verify kiln.preview.structural]
#[test]
fn pins_are_exactly_the_tree_sitter_dependencies() {
    let mut in_dependencies = false;
    let mut declared = BTreeSet::new();
    for line in MANIFEST.lines() {
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]";
        } else if in_dependencies {
            let name = line.split('.').next().unwrap_or_default().trim();
            if name.starts_with("tree-sitter") {
                declared.insert(name);
            }
        }
    }
    let pinned: Vec<_> = GRAMMAR_PINS.iter().map(|pin| pin.package).collect();
    let mut sorted = pinned.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(pinned, sorted, "pins are sorted and unique");
    assert_eq!(declared, pinned.into_iter().collect::<BTreeSet<_>>());
}

// r[verify kiln.preview.structural]
#[test]
fn every_language_names_a_pinned_grammar() {
    let runtime = ["tree-sitter", "tree-sitter-highlight"];
    for language in Language::ALL {
        let package = grammar_package(language);
        assert!(!runtime.contains(&package), "{language:?}");
        assert!(
            GRAMMAR_PINS.iter().any(|pin| pin.package == package),
            "{language:?} parses with unpinned {package}"
        );
    }
}
