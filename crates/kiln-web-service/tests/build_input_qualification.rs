//! The build-input corpus (#77) through the planner, the projected
//! materializer and the affected-project decision, compared with its
//! committed expectation. Refresh `fixtures/build-inputs/expected.json` only
//! after reviewing a deliberate corpus, planner or materializer change. The
//! `build-input-fixtures` feature also runs every planned Cargo command, using
//! `KILN_CARGO_EXE` or `cargo` on `PATH`.
#[path = "../examples/build_inputs/harness.rs"]
mod harness;

use serde_json::Value;

const CORPUS: &[u8] = include_bytes!("../../../fixtures/build-inputs/corpus.json");
const EXPECTED: &[u8] = include_bytes!("../../../fixtures/build-inputs/expected.json");

/// A receipt without what this host measured, and without executions when
/// commands were not run. The committed expectation already lacks the
/// measured fields.
fn comparable(mut receipt: Value, executions: bool) -> harness::Result<Value> {
    let fields = receipt
        .as_object_mut()
        .ok_or("the receipt is not an object")?;
    fields.remove("timings");
    fields
        .get_mut("toolchain")
        .and_then(Value::as_object_mut)
        .ok_or("the receipt has no toolchain")?
        .remove("observed");
    if !executions {
        for case in fields
            .get_mut("cases")
            .and_then(Value::as_array_mut)
            .ok_or("the receipt has no cases")?
        {
            case.as_object_mut()
                .and_then(|case| case.remove("execution"))
                .ok_or("a case has no execution")?;
        }
    }
    Ok(receipt)
}

// r[verify kiln.source.projection.build_inputs.scenario.planned]
// r[verify kiln.source.projection.build_inputs.scenario.refused]
#[test]
fn plans_and_the_shared_edit_match_the_frozen_expectation() -> harness::Result {
    let scratch = tempfile::tempdir()?;
    let receipt = harness::run(CORPUS, scratch.path(), None)?;
    assert!(
        receipt
            .cases
            .iter()
            .all(|case| matches!(case.execution, harness::Execution::NotRun { .. }))
    );
    let observed = comparable(serde_json::to_value(&receipt)?, false)?;
    let expected = comparable(serde_json::from_slice(EXPECTED)?, false)?;
    assert_eq!(observed, expected);
    Ok(())
}

#[test]
fn a_malformed_corpus_is_refused_before_anything_is_written() -> harness::Result {
    let corpus: Value = serde_json::from_slice(CORPUS)?;
    let mut duplicated = corpus.clone();
    if let Some(files) = duplicated.get_mut("files").and_then(Value::as_array_mut) {
        let first = files.first().cloned().ok_or("the corpus has no files")?;
        files.push(first);
    }
    let mut unknown_edit = corpus.clone();
    if let Some(edit) = unknown_edit
        .get_mut("shared_edit")
        .and_then(Value::as_object_mut)
    {
        edit.insert("case".into(), "gamma-tests".into());
    }
    let mut escaping = corpus.clone();
    if let Some(case) = escaping
        .get_mut("cases")
        .and_then(Value::as_array_mut)
        .and_then(|cases| cases.first_mut())
        .and_then(Value::as_object_mut)
    {
        case.insert("id".into(), "../outside".into());
    }
    let mut renamed = corpus;
    if let Some(fields) = renamed.as_object_mut() {
        fields.insert("schema".into(), "kiln.build-input-corpus.v0".into());
    }
    for refused in [duplicated, unknown_edit, escaping, renamed] {
        let scratch = tempfile::tempdir()?;
        assert!(harness::run(&serde_json::to_vec(&refused)?, scratch.path(), None).is_err());
        assert_eq!(std::fs::read_dir(scratch.path())?.count(), 0);
    }
    Ok(())
}

// r[verify kiln.source.projection.build_inputs.scenario.qualified]
#[cfg(feature = "build-input-fixtures")]
#[test]
fn planned_commands_qualify_their_views() -> harness::Result {
    let cargo = match std::env::var_os("KILN_CARGO_EXE") {
        Some(path) => std::path::PathBuf::from(path),
        None => harness::on_path(&std::env::var_os("PATH").ok_or("PATH is unset")?, "cargo")
            .ok_or("build-input-fixtures needs KILN_CARGO_EXE or cargo on PATH")?,
    };
    let scratch = tempfile::tempdir()?;
    let receipt = harness::run(CORPUS, scratch.path(), Some(&cargo))?;
    let observed = comparable(serde_json::to_value(&receipt)?, true)?;
    let expected = comparable(serde_json::from_slice(EXPECTED)?, true)?;
    assert_eq!(observed, expected);
    // Beta failed on its undeclared read of alpha's asset, which the working
    // checkout still holds.
    let log = std::fs::read_to_string(
        scratch
            .path()
            .join(harness::LOG_DIRECTORY)
            .join("beta-tests.log"),
    )?;
    assert!(log.contains("alpha/assets/banner.txt"), "{log}");
    assert!(
        scratch
            .path()
            .join(harness::CHECKOUT_DIRECTORY)
            .join("projects/alpha/assets/banner.txt")
            .is_file()
    );
    Ok(())
}
