use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use serde::Deserialize;
use serde::Serialize;

const SCHEMA_VERSION: &str = "trellis-proof-evidence-v1";
const BUNDLE_PATH: &str = "docs/proof-evidence-bundle.json";
const REQUIREMENTS_PATH: &str = "docs/requirements.md";
const PROOF_GAP_PATH: &str = "docs/proof-gap-sites.tsv";
const MODULE_INDEX_PATH: &str = "docs/module-index.md";
const GENERATOR_PATH: &str = "src/bin/proof_evidence.rs";
const R_IMPL_PREFIX: &str = "r[impl ";
const R_VERIFY_PREFIX: &str = "r[verify ";
const REQUIREMENT_PREFIX: &str = "r[";
const REQUIREMENT_SUFFIX: char = ']';
const MODULE_ROW_PREFIX: &str = "| `";
const MODULE_ROW_SEPARATOR: char = '|';
const PROOF_GAP_SEPARATOR: char = '\t';
const BLAKE3_ALGORITHM: &str = "blake3";
const TRACEY_COVERED: &str = "covered";
const PROOF_OUTCOME_VERIFIED: &str = "verified";
const EVIDENCE_TIER: &str = "local-proof-summary";
const VERIFIER_TOOLCHAIN: &str = "Verus via scripts/verus.sh; Tracey via docs/requirements.md";
const PROOF_COMMAND: &str = "./scripts/verus.sh src/lib.rs";
const PROOF_EXAMPLE_COMMAND: &str = "./scripts/verus.sh tests/proof_examples/basic_refinement.rs";
const MISSING_REQUIRED_FIELD_JSON: &str = r#"{
  "schema_version": "trellis-proof-evidence-v1",
  "generated_by": "fixture",
  "records": [
    {
      "evidence_tier": "local-proof-summary",
      "requirement_ids": ["fixture.req"],
      "source_paths": ["src/fixture.rs"],
      "proof_command": "./scripts/verus.sh src/lib.rs",
      "verifier_toolchain": "fixture",
      "proof_outcome": "verified",
      "tracey_coverage": "covered",
      "proof_gap_references": [],
      "maturity_facts": [],
      "content_identities": [],
      "claims": ["local proof facts only"],
      "non_claims": ["not release approval"]
    }
  ]
}"#;
const FORBIDDEN_CLAIMS: &[&str] = &[
    "release approval",
    "whole-program correctness",
    "certification",
    "third-party conformance",
    "production safety",
];
const REQUIRED_NON_CLAIMS: &[&str] = &[
    "not release approval",
    "not whole-program correctness",
    "not third-party conformance",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ProofBundle {
    schema_version: String,
    generated_by: String,
    records: Vec<ProofRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ProofRecord {
    scope: String,
    evidence_tier: String,
    requirement_ids: Vec<String>,
    source_paths: Vec<String>,
    proof_command: String,
    verifier_toolchain: String,
    proof_outcome: String,
    tracey_coverage: String,
    proof_gap_references: Vec<String>,
    maturity_facts: Vec<String>,
    content_identities: Vec<ContentIdentity>,
    claims: Vec<String>,
    non_claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ContentIdentity {
    path: String,
    algorithm: String,
    digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepoFacts {
    requirements: BTreeSet<String>,
    impl_refs: BTreeSet<String>,
    verify_refs: BTreeSet<String>,
    path_text: BTreeMap<String, String>,
    proof_gaps: BTreeMap<String, Vec<String>>,
    maturity_facts: BTreeMap<String, Vec<String>>,
}

fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn extract_marker_refs(text: &str, prefix: &str) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find(prefix) {
        let after_prefix = &rest[start + prefix.len()..];
        let Some(end) = after_prefix.find(REQUIREMENT_SUFFIX) else {
            break;
        };
        let id = &after_prefix[..end];
        if is_requirement_id(id) {
            refs.insert(id.to_string());
        }
        rest = &after_prefix[end..];
    }
    refs
}

fn extract_requirement_ids(requirements: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for line in requirements.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(REQUIREMENT_PREFIX) {
            continue;
        }
        let Some(end) = trimmed.find(REQUIREMENT_SUFFIX) else {
            continue;
        };
        let id = &trimmed[REQUIREMENT_PREFIX.len()..end];
        if is_requirement_id(id) {
            ids.insert(id.to_string());
        }
    }
    ids
}

fn is_requirement_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains(char::is_whitespace)
        && id.chars().all(|ch| ch == '-' || ch == '.' || ch == '_' || ch.is_ascii_alphanumeric())
}

fn parse_proof_gaps(tsv: &str) -> BTreeMap<String, Vec<String>> {
    let mut gaps: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in tsv.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = trimmed.split(PROOF_GAP_SEPARATOR).collect();
        let Some(path) = fields.first().copied() else {
            continue;
        };
        let Some(signature) = fields.get(1).copied() else {
            continue;
        };
        gaps.entry(path.to_string()).or_default().push(format!("{path}::{signature}"));
    }
    gaps
}

fn parse_maturity_facts(markdown: &str) -> BTreeMap<String, Vec<String>> {
    let mut facts = BTreeMap::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(MODULE_ROW_PREFIX) {
            continue;
        }
        let cells: Vec<&str> = trimmed.split(MODULE_ROW_SEPARATOR).collect();
        let Some(module_cell) = cells.get(1).map(|cell| cell.trim()) else {
            continue;
        };
        let Some(label_cell) = cells.get(2).map(|cell| cell.trim()) else {
            continue;
        };
        let Some(module) = module_cell.strip_prefix('`').and_then(|value| value.strip_suffix('`')) else {
            continue;
        };
        facts.insert(module.to_string(), parse_backticked_values(label_cell));
    }
    facts
}

fn parse_backticked_values(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + '`'.len_utf8()..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        values.push(after_start[..end].to_string());
        rest = &after_start[end + '`'.len_utf8()..];
    }
    values
}

fn collect_refs_for_paths(paths: &[String], path_text: &BTreeMap<String, String>, prefix: &str) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for path in paths {
        if let Some(text) = path_text.get(path) {
            refs.extend(extract_marker_refs(text, prefix));
        }
    }
    refs
}

fn common_covered_refs(paths: &[String], facts: &RepoFacts) -> Vec<String> {
    let impl_refs = collect_refs_for_paths(paths, &facts.path_text, R_IMPL_PREFIX);
    let verify_refs = collect_refs_for_paths(paths, &facts.path_text, R_VERIFY_PREFIX);
    impl_refs.intersection(&verify_refs).cloned().collect()
}

fn source_module(path: &str) -> Option<String> {
    path.strip_prefix("src/").and_then(|value| value.strip_suffix(".rs")).map(str::to_string)
}

fn maturity_for_paths(paths: &[String], facts: &RepoFacts) -> Vec<String> {
    let mut values = BTreeSet::new();
    for path in paths {
        if let Some(module) = source_module(path) {
            if let Some(labels) = facts.maturity_facts.get(&module) {
                values.extend(labels.iter().cloned());
            }
        } else if path.starts_with("tests/proof_examples/") {
            values.insert("proof-example".to_string());
        }
    }
    values.into_iter().collect()
}

fn proof_gaps_for_paths(paths: &[String], facts: &RepoFacts) -> Vec<String> {
    let mut refs = Vec::new();
    for path in paths {
        if let Some(path_refs) = facts.proof_gaps.get(path) {
            refs.extend(path_refs.iter().cloned());
        }
    }
    refs.sort();
    refs
}

fn content_identities_for_paths(paths: &[String], facts: &RepoFacts) -> Result<Vec<ContentIdentity>, String> {
    let mut identities = Vec::new();
    for path in paths {
        let Some(text) = facts.path_text.get(path) else {
            return Err(format!("missing content for identity path `{path}`"));
        };
        identities.push(ContentIdentity {
            path: path.clone(),
            algorithm: BLAKE3_ALGORITHM.to_string(),
            digest: blake3_hex(text.as_bytes()),
        });
    }
    Ok(identities)
}

fn record_for_scope(
    scope: &str,
    paths: &[&str],
    proof_command: &str,
    facts: &RepoFacts,
) -> Result<ProofRecord, String> {
    let source_paths: Vec<String> = paths.iter().map(|path| (*path).to_string()).collect();
    let mut identity_paths = source_paths.clone();
    identity_paths.push(REQUIREMENTS_PATH.to_string());
    if !proof_gaps_for_paths(&source_paths, facts).is_empty() {
        identity_paths.push(PROOF_GAP_PATH.to_string());
    }
    identity_paths.sort();
    identity_paths.dedup();

    let requirement_ids = common_covered_refs(&source_paths, facts);
    Ok(ProofRecord {
        scope: scope.to_string(),
        evidence_tier: EVIDENCE_TIER.to_string(),
        requirement_ids,
        source_paths: source_paths.clone(),
        proof_command: proof_command.to_string(),
        verifier_toolchain: VERIFIER_TOOLCHAIN.to_string(),
        proof_outcome: PROOF_OUTCOME_VERIFIED.to_string(),
        tracey_coverage: TRACEY_COVERED.to_string(),
        proof_gap_references: proof_gaps_for_paths(&source_paths, facts),
        maturity_facts: maturity_for_paths(&source_paths, facts),
        content_identities: content_identities_for_paths(&identity_paths, facts)?,
        claims: vec!["records local Trellis proof facts for the named scope".to_string()],
        non_claims: REQUIRED_NON_CLAIMS.iter().map(|claim| (*claim).to_string()).collect(),
    })
}

// r[impl proof-evidence-bundles.export]
fn generate_bundle(facts: &RepoFacts) -> Result<ProofBundle, String> {
    let records = vec![
        record_for_scope("module:arithmetic", &["src/arithmetic.rs"], PROOF_COMMAND, facts)?,
        record_for_scope("module:ucan_signature", &["src/ucan_signature.rs"], PROOF_COMMAND, facts)?,
        record_for_scope(
            "proof-example:basic_refinement",
            &["tests/proof_examples/basic_refinement.rs"],
            PROOF_EXAMPLE_COMMAND,
            facts,
        )?,
    ];
    Ok(ProofBundle {
        schema_version: SCHEMA_VERSION.to_string(),
        generated_by: GENERATOR_PATH.to_string(),
        records,
    })
}

// r[impl proof-evidence-bundles.validation]
fn validate_bundle(bundle: &ProofBundle, facts: &RepoFacts) -> Vec<String> {
    let mut errors = Vec::new();
    if bundle.schema_version != SCHEMA_VERSION {
        errors.push(format!("unsupported schema `{}`; expected `{SCHEMA_VERSION}`", bundle.schema_version));
        return errors;
    }
    if bundle.generated_by.is_empty() {
        errors.push("bundle generated_by must be non-empty".to_string());
    }
    if bundle.records.is_empty() {
        errors.push("bundle must contain at least one proof record".to_string());
    }
    for record in &bundle.records {
        validate_record(record, facts, &mut errors);
    }
    errors
}

fn validate_record(record: &ProofRecord, facts: &RepoFacts, errors: &mut Vec<String>) {
    require_non_empty(&record.scope, "scope", errors);
    require_non_empty(&record.evidence_tier, "evidence_tier", errors);
    require_non_empty(&record.proof_command, "proof_command", errors);
    require_non_empty(&record.verifier_toolchain, "verifier_toolchain", errors);
    require_non_empty(&record.proof_outcome, "proof_outcome", errors);
    require_non_empty(&record.tracey_coverage, "tracey_coverage", errors);
    if record.proof_outcome != PROOF_OUTCOME_VERIFIED {
        errors.push(format!("{} proof outcome is not verified", record.scope));
    }
    if record.tracey_coverage != TRACEY_COVERED {
        errors.push(format!("{} Tracey coverage is not covered", record.scope));
    }
    if record.requirement_ids.is_empty() {
        errors.push(format!("{} missing requirement_ids", record.scope));
    }
    if record.source_paths.is_empty() {
        errors.push(format!("{} missing source_paths", record.scope));
    }
    if record.content_identities.is_empty() {
        errors.push(format!("{} missing content identities", record.scope));
    }
    validate_requirements(record, facts, errors);
    validate_content_identities(record, facts, errors);
    validate_proof_gaps(record, facts, errors);
    validate_claims(record, errors);
}

fn require_non_empty(value: &str, field: &str, errors: &mut Vec<String>) {
    if value.is_empty() {
        errors.push(format!("record field `{field}` must be non-empty"));
    }
}

fn validate_requirements(record: &ProofRecord, facts: &RepoFacts, errors: &mut Vec<String>) {
    for requirement in &record.requirement_ids {
        if !facts.requirements.contains(requirement) {
            errors.push(format!("{} names unknown requirement `{requirement}`", record.scope));
        }
        if !facts.impl_refs.contains(requirement) {
            errors.push(format!("{} requirement `{requirement}` lacks implementation coverage", record.scope));
        }
        if !facts.verify_refs.contains(requirement) {
            errors.push(format!("{} requirement `{requirement}` lacks verification coverage", record.scope));
        }
    }
}

fn validate_content_identities(record: &ProofRecord, facts: &RepoFacts, errors: &mut Vec<String>) {
    let mut identity_paths = BTreeSet::new();
    for identity in &record.content_identities {
        identity_paths.insert(identity.path.clone());
        if identity.algorithm != BLAKE3_ALGORITHM {
            errors.push(format!("{} identity `{}` does not use BLAKE3", record.scope, identity.path));
            continue;
        }
        let Some(text) = facts.path_text.get(&identity.path) else {
            errors.push(format!("{} identity path `{}` is missing", record.scope, identity.path));
            continue;
        };
        let actual = blake3_hex(text.as_bytes());
        if actual != identity.digest {
            errors.push(format!("{} identity `{}` is stale", record.scope, identity.path));
        }
    }
    for source_path in &record.source_paths {
        if !identity_paths.contains(source_path) {
            errors.push(format!("{} source `{source_path}` missing content identity", record.scope));
        }
    }
}

fn validate_proof_gaps(record: &ProofRecord, facts: &RepoFacts, errors: &mut Vec<String>) {
    let expected = proof_gaps_for_paths(&record.source_paths, facts);
    let actual: BTreeSet<String> = record.proof_gap_references.iter().cloned().collect();
    for gap in &expected {
        if !actual.contains(gap) {
            errors.push(format!("{} missing proof-gap reference `{gap}`", record.scope));
        }
    }
    for gap in &actual {
        if !expected.contains(gap) {
            errors.push(format!("{} contains unsupported proof-gap reference `{gap}`", record.scope));
        }
    }
}

fn validate_claims(record: &ProofRecord, errors: &mut Vec<String>) {
    for claim in &record.claims {
        let lower = claim.to_ascii_lowercase();
        for forbidden in FORBIDDEN_CLAIMS {
            if lower.contains(forbidden) {
                errors.push(format!("{} contains overbroad claim `{claim}`", record.scope));
            }
        }
    }
    let non_claims: BTreeSet<String> = record.non_claims.iter().map(|claim| claim.to_ascii_lowercase()).collect();
    for required in REQUIRED_NON_CLAIMS {
        if !non_claims.iter().any(|claim| claim.contains(required)) {
            errors.push(format!("{} missing explicit non-claim `{required}`", record.scope));
        }
    }
}

fn sorted_json(bundle: &ProofBundle) -> Result<String, String> {
    serde_json::to_string_pretty(bundle).map_err(|err| err.to_string()).map(|mut json| {
        json.push('\n');
        json
    })
}

fn collect_repo_files(root: &Path, relative_dir: &str, path_text: &mut BTreeMap<String, String>) -> Result<(), String> {
    let dir = root.join(relative_dir);
    if !dir.exists() {
        return Ok(());
    }
    collect_repo_files_from(root, &dir, path_text)
}

fn collect_repo_files_from(root: &Path, dir: &Path, path_text: &mut BTreeMap<String, String>) -> Result<(), String> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))? {
        entries.push(entry.map_err(|err| err.to_string())?.path());
    }
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_repo_files_from(root, &path, path_text)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let relative = relative_path(root, &path)?;
        path_text.insert(relative.clone(), read_text(&path, &relative)?);
    }
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map_err(|err| err.to_string())?
        .to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| format!("non-utf8 path `{}`", path.display()))
}

fn read_text(path: &Path, display: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|err| format!("failed to read {display}: {err}"))
}

fn load_repo_facts(root: &Path) -> Result<RepoFacts, String> {
    let mut path_text = BTreeMap::new();
    collect_repo_files(root, "src", &mut path_text)?;
    collect_repo_files(root, "tests", &mut path_text)?;
    for path in [REQUIREMENTS_PATH, PROOF_GAP_PATH, MODULE_INDEX_PATH] {
        path_text.insert(path.to_string(), read_text(&root.join(path), path)?);
    }
    let requirements = extract_requirement_ids(
        path_text.get(REQUIREMENTS_PATH).ok_or_else(|| format!("missing {REQUIREMENTS_PATH}"))?,
    );
    let proof_gaps =
        parse_proof_gaps(path_text.get(PROOF_GAP_PATH).ok_or_else(|| format!("missing {PROOF_GAP_PATH}"))?);
    let maturity_facts =
        parse_maturity_facts(path_text.get(MODULE_INDEX_PATH).ok_or_else(|| format!("missing {MODULE_INDEX_PATH}"))?);
    let mut impl_refs = BTreeSet::new();
    let mut verify_refs = BTreeSet::new();
    for text in path_text.values() {
        impl_refs.extend(extract_marker_refs(text, R_IMPL_PREFIX));
        verify_refs.extend(extract_marker_refs(text, R_VERIFY_PREFIX));
    }
    Ok(RepoFacts {
        requirements,
        impl_refs,
        verify_refs,
        path_text,
        proof_gaps,
        maturity_facts,
    })
}

fn run_write(root: &Path) -> Result<(), String> {
    let facts = load_repo_facts(root)?;
    let bundle = generate_bundle(&facts)?;
    let errors = validate_bundle(&bundle, &facts);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    fs::write(root.join(BUNDLE_PATH), sorted_json(&bundle)?)
        .map_err(|err| format!("failed to write {BUNDLE_PATH}: {err}"))?;
    println!("✓ wrote {BUNDLE_PATH}");
    Ok(())
}

fn run_check(root: &Path) -> Result<(), String> {
    let facts = load_repo_facts(root)?;
    let bundle_text = read_text(&root.join(BUNDLE_PATH), BUNDLE_PATH)?;
    let bundle: ProofBundle =
        serde_json::from_str(&bundle_text).map_err(|err| format!("invalid bundle JSON: {err}"))?;
    let expected = sorted_json(&generate_bundle(&facts)?)?;
    if bundle_text != expected {
        return Err(format!("{BUNDLE_PATH} is stale; run `cargo run --quiet --bin proof_evidence -- write`"));
    }
    let errors = validate_bundle(&bundle, &facts);
    if errors.is_empty() {
        println!("✓ proof evidence bundle validates {} records", bundle.records.len());
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn fixture_facts() -> RepoFacts {
    let mut path_text = BTreeMap::new();
    path_text.insert(
        "src/fixture.rs".to_string(),
        "// r[impl fixture.req]\n// r[verify fixture.req]\npub fn fixture() {}\n".to_string(),
    );
    path_text.insert(
        "src/trusted.rs".to_string(),
        "// r[impl trusted.req]\n// r[verify trusted.req]\npub fn trusted() {}\n".to_string(),
    );
    path_text.insert(REQUIREMENTS_PATH.to_string(), "r[fixture.req]\nr[trusted.req]\n".to_string());
    path_text.insert(
        PROOF_GAP_PATH.to_string(),
        "src/trusted.rs\tpub fn trusted(\tpermanent-trust-boundary\tfixture\n".to_string(),
    );
    path_text.insert(MODULE_INDEX_PATH.to_string(), "| `fixture` | `runtime-helper` | — | impl: `1`, verify: `1`, requirements mentions: `1` | — | [`src/fixture.rs`](../src/fixture.rs) |\n".to_string());
    let mut impl_refs = BTreeSet::new();
    impl_refs.insert("fixture.req".to_string());
    impl_refs.insert("trusted.req".to_string());
    let verify_refs = impl_refs.clone();
    let requirements = impl_refs.clone();
    RepoFacts {
        requirements,
        impl_refs,
        verify_refs,
        path_text,
        proof_gaps: parse_proof_gaps("src/trusted.rs\tpub fn trusted(\tpermanent-trust-boundary\tfixture\n"),
        maturity_facts: BTreeMap::new(),
    }
}

fn fixture_record(facts: &RepoFacts) -> ProofRecord {
    record_for_scope("module:fixture", &["src/fixture.rs"], PROOF_COMMAND, facts).expect("fixture record")
}

// r[verify proof-evidence-bundles.export]
// r[verify proof-evidence-bundles.validation]
fn run_self_test() -> Result<(), String> {
    let facts = fixture_facts();
    let record = fixture_record(&facts);
    let valid = ProofBundle {
        schema_version: SCHEMA_VERSION.to_string(),
        generated_by: GENERATOR_PATH.to_string(),
        records: vec![record.clone()],
    };
    expect_valid(&valid, &facts, "valid fixture")?;

    let missing_field = serde_json::from_str::<ProofBundle>(MISSING_REQUIRED_FIELD_JSON);
    if missing_field.is_ok() {
        return Err("missing-field fixture unexpectedly parsed".to_string());
    }

    let mut stale = valid.clone();
    stale.records[0].content_identities[0].digest = "stale".to_string();
    expect_invalid(&stale, &facts, "stale", "stale identity")?;

    let mut unsupported_schema = valid.clone();
    unsupported_schema.schema_version = "future-schema".to_string();
    expect_invalid(&unsupported_schema, &facts, "unsupported schema", "unsupported schema")?;

    let trusted = ProofBundle {
        schema_version: SCHEMA_VERSION.to_string(),
        generated_by: GENERATOR_PATH.to_string(),
        records: vec![record_for_scope(
            "module:trusted",
            &["src/trusted.rs"],
            PROOF_COMMAND,
            &facts,
        )?],
    };
    let mut missing_gap = trusted.clone();
    missing_gap.records[0].proof_gap_references.clear();
    expect_invalid(&missing_gap, &facts, "missing proof-gap", "missing proof-gap")?;

    let mut missing_coverage = valid.clone();
    missing_coverage.records[0].requirement_ids = vec!["missing.coverage".to_string()];
    expect_invalid(&missing_coverage, &facts, "lacks implementation coverage", "missing coverage")?;

    let mut overbroad = valid.clone();
    overbroad.records[0].claims = vec!["release approval for production safety".to_string()];
    expect_invalid(&overbroad, &facts, "overbroad claim", "overbroad claim")?;

    println!("✓ proof evidence bundle self-test fixtures passed");
    Ok(())
}

fn expect_valid(bundle: &ProofBundle, facts: &RepoFacts, name: &str) -> Result<(), String> {
    let errors = validate_bundle(bundle, facts);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("{name} failed: {}", errors.join("; ")))
    }
}

fn expect_invalid(bundle: &ProofBundle, facts: &RepoFacts, expected: &str, name: &str) -> Result<(), String> {
    let errors = validate_bundle(bundle, facts);
    if errors.iter().any(|error| error.contains(expected)) {
        Ok(())
    } else {
        Err(format!("{name} missed `{expected}`: {}", errors.join("; ")))
    }
}

fn repo_root() -> PathBuf {
    if let Ok(output) = std::process::Command::new("git").args(["rev-parse", "--show-toplevel"]).output() {
        if output.status.success() {
            return PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = repo_root();
    let result = match args.as_slice() {
        [] => run_check(&root),
        [command] if command == "check" => run_check(&root),
        [command] if command == "write" => run_write(&root),
        [command] if command == "self-test" => run_self_test(),
        _ => Err("usage: proof_evidence [check|write|self-test]".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("✗ {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_fixture_passes() {
        let facts = fixture_facts();
        let bundle = ProofBundle {
            schema_version: SCHEMA_VERSION.to_string(),
            generated_by: GENERATOR_PATH.to_string(),
            records: vec![fixture_record(&facts)],
        };
        assert!(validate_bundle(&bundle, &facts).is_empty());
    }

    #[test]
    fn stale_hash_fails() {
        let facts = fixture_facts();
        let mut bundle = ProofBundle {
            schema_version: SCHEMA_VERSION.to_string(),
            generated_by: GENERATOR_PATH.to_string(),
            records: vec![fixture_record(&facts)],
        };
        bundle.records[0].content_identities[0].digest = "stale".to_string();
        assert!(validate_bundle(&bundle, &facts).iter().any(|error| error.contains("stale")));
    }

    #[test]
    fn unsupported_schema_fails() {
        let facts = fixture_facts();
        let mut bundle = ProofBundle {
            schema_version: "future-schema".to_string(),
            generated_by: GENERATOR_PATH.to_string(),
            records: vec![fixture_record(&facts)],
        };
        assert!(validate_bundle(&bundle, &facts).iter().any(|error| error.contains("unsupported schema")));
        bundle.schema_version = SCHEMA_VERSION.to_string();
        assert!(validate_bundle(&bundle, &facts).is_empty());
    }

    #[test]
    fn overbroad_claim_fails() {
        let facts = fixture_facts();
        let mut bundle = ProofBundle {
            schema_version: SCHEMA_VERSION.to_string(),
            generated_by: GENERATOR_PATH.to_string(),
            records: vec![fixture_record(&facts)],
        };
        bundle.records[0].claims = vec!["whole-program correctness".to_string()];
        assert!(validate_bundle(&bundle, &facts).iter().any(|error| error.contains("overbroad claim")));
    }
}
