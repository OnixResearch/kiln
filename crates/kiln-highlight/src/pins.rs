//! Exact identity inputs of the structural engine.
//!
//! [`STRUCTURAL_RULES`] versions the matching rules in this crate.
//! [`GRAMMAR_PINS`] repeats the package, version and registry checksum that
//! `Cargo.lock` selects for the parser runtime, the highlight engine (whose
//! query configuration decides whether a grammar is available) and every
//! grammar. Tests fail when a pin and the lock file or this crate's grammar
//! dependencies disagree, so a dependency update cannot leave a stale
//! description. Consumers hash [`grammar_set`] into their own domain-tagged
//! identities; this crate links no hash function.

// r[impl kiln.preview.structural]

use serde::Serialize;

use crate::Language;

/// Version of the structural matching rules. Bump it with any change to
/// [`crate::structural_diff`] that can alter an outcome for the same blobs and
/// bounds.
pub const STRUCTURAL_RULES: &str = "kiln.highlight.structural-rules.v1";

/// One linked registry package, exactly as `Cargo.lock` selects it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GrammarPin {
    pub package: &'static str,
    pub version: &'static str,
    /// Cargo's registry checksum, the SHA-256 of the published archive, kept
    /// in its native form.
    pub checksum: &'static str,
}

const fn pin(package: &'static str, version: &'static str, checksum: &'static str) -> GrammarPin {
    GrammarPin {
        package,
        version,
        checksum,
    }
}

/// Every package that decides a parse tree or a grammar's availability,
/// sorted by package name.
pub const GRAMMAR_PINS: [GrammarPin; 17] = [
    pin(
        "tree-sitter",
        "0.26.13",
        "17ebdd3a5a7e28a1890b876fdbd0c3c0fe0a6336cffaa104f11b9f720c9daa29",
    ),
    pin(
        "tree-sitter-bash",
        "0.25.1",
        "9e5ec769279cc91b561d3df0d8a5deb26b0ad40d183127f409494d6d8fc53062",
    ),
    pin(
        "tree-sitter-c",
        "0.24.2",
        "a9b2eb57a55fed6b00812912e730b7a275cf4fe98bfd6a5d76263d4438371728",
    ),
    pin(
        "tree-sitter-css",
        "0.25.0",
        "a5cbc5e18f29a2c6d6435891f42569525cf95435a3e01c2f1947abcde178686f",
    ),
    pin(
        "tree-sitter-go",
        "0.25.0",
        "c8560a4d2f835cc0d4d2c2e03cbd0dde2f6114b43bc491164238d333e28b16ea",
    ),
    pin(
        "tree-sitter-highlight",
        "0.26.13",
        "662316ecd27166f79e18865429638c05194aee76a7bc374136d844f05d778f74",
    ),
    pin(
        "tree-sitter-html",
        "0.23.2",
        "261b708e5d92061ede329babaaa427b819329a9d427a1d710abb0f67bbef63ee",
    ),
    pin(
        "tree-sitter-javascript",
        "0.25.0",
        "68204f2abc0627a90bdf06e605f5c470aa26fdcb2081ea553a04bdad756693f5",
    ),
    pin(
        "tree-sitter-json",
        "0.24.8",
        "4d727acca406c0020cffc6cf35516764f36c8e3dc4408e5ebe2cb35a947ec471",
    ),
    pin(
        "tree-sitter-md",
        "0.5.4",
        "8ed661bfe3c264961425f77cd89ad2998bf5d629e71ce8bab1e5d7e940d6246e",
    ),
    pin(
        "tree-sitter-nickel",
        "0.5.0",
        "f7bb930cf314466ad3ca1e45c876bbbca228f66fe92db8a087796cf8f26d3ba8",
    ),
    pin(
        "tree-sitter-nix",
        "0.3.0",
        "4952a9733f3a98f6683a0ccd1035d84ab7a52f7e84eeed58548d86765ad92de3",
    ),
    pin(
        "tree-sitter-python",
        "0.25.0",
        "6bf85fd39652e740bf60f46f4cda9492c3a9ad75880575bf14960f775cb74a1c",
    ),
    pin(
        "tree-sitter-rust",
        "0.24.2",
        "439e577dbe07423ec2582ac62c7531120dbfccfa6e5f92406f93dd271a120e45",
    ),
    pin(
        "tree-sitter-toml-ng",
        "0.7.0",
        "e9adc2c898ae49730e857d75be403da3f92bb81d8e37a2f918a08dd10de5ebb1",
    ),
    pin(
        "tree-sitter-typescript",
        "0.23.2",
        "6c5f76ed8d947a75cc446d5fccd8b602ebf0cde64ccf2ffa434d873d7a575eff",
    ),
    pin(
        "tree-sitter-yaml",
        "0.7.2",
        "53c223db85f05e34794f065454843b0668ebc15d240ada63e2b5939f43ce7c97",
    ),
];

/// The pinned package whose grammar parses `language`; it must match the
/// crate the grammar registry loads for that language.
pub const fn grammar_package(language: Language) -> &'static str {
    match language {
        Language::Rust => "tree-sitter-rust",
        Language::Javascript => "tree-sitter-javascript",
        Language::Typescript | Language::Tsx => "tree-sitter-typescript",
        Language::Python => "tree-sitter-python",
        Language::Nix => "tree-sitter-nix",
        Language::Nickel => "tree-sitter-nickel",
        Language::Toml => "tree-sitter-toml-ng",
        Language::Json => "tree-sitter-json",
        Language::Yaml => "tree-sitter-yaml",
        Language::Css => "tree-sitter-css",
        Language::Html => "tree-sitter-html",
        Language::Bash => "tree-sitter-bash",
        Language::Markdown => "tree-sitter-md",
        Language::C => "tree-sitter-c",
        Language::Go => "tree-sitter-go",
    }
}

/// The pinned packages and which of them parses each language.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GrammarSet {
    pub pins: &'static [GrammarPin],
    pub languages: [(Language, &'static str); Language::ALL.len()],
}

/// The grammar set the structural engine parses with.
pub fn grammar_set() -> GrammarSet {
    GrammarSet {
        pins: &GRAMMAR_PINS,
        languages: Language::ALL.map(|language| (language, grammar_package(language))),
    }
}
