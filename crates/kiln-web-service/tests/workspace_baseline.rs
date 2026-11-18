//! The frozen shared projected-workspace corpus (#82, #78) through the local
//! projected materializer, compared with its committed expectation apart from
//! timings. Refresh `fixtures/workspace-corpus/expected.json` only after
//! reviewing a deliberate corpus or materializer change.
#[path = "../examples/workspace_baseline/harness.rs"]
mod harness;

const CORPUS: &[u8] = include_bytes!("../../../fixtures/workspace-corpus/corpus.json");
const EXPECTED: &[u8] = include_bytes!("../../../fixtures/workspace-corpus/expected.json");

// r[verify kiln.workspace_provider.evaluation.scenario.baseline]
#[test]
fn the_shared_corpus_matches_its_local_baseline() -> harness::Result {
    let scratch = tempfile::tempdir()?;
    let receipt = harness::run(CORPUS, scratch.path())?;
    let mut observed = serde_json::to_value(&receipt)?;
    observed
        .as_object_mut()
        .and_then(|fields| fields.remove("timings"))
        .ok_or("the receipt has no timings")?;
    let expected: serde_json::Value = serde_json::from_slice(EXPECTED)?;
    assert_eq!(observed, expected);
    Ok(())
}

// r[verify kiln.workspace_provider.evaluation.scenario.baseline]
#[test]
fn a_malformed_corpus_is_refused_before_anything_is_written() -> harness::Result {
    let corpus: serde_json::Value = serde_json::from_slice(CORPUS)?;
    let mut duplicated = corpus.clone();
    if let Some(files) = duplicated
        .get_mut("files")
        .and_then(serde_json::Value::as_array_mut)
    {
        let first = files.first().cloned().ok_or("the corpus has no files")?;
        files.push(first);
    }
    let mut single_attempt = corpus.clone();
    if let Some(attempts) = single_attempt
        .get_mut("attempts")
        .and_then(serde_json::Value::as_array_mut)
    {
        attempts.truncate(1);
    }
    let mut renamed = corpus;
    if let Some(fields) = renamed.as_object_mut() {
        fields.insert("schema".into(), "kiln.workspace-corpus.v0".into());
    }
    for refused in [duplicated, single_attempt, renamed] {
        let scratch = tempfile::tempdir()?;
        assert!(harness::run(&serde_json::to_vec(&refused)?, scratch.path()).is_err());
        assert_eq!(std::fs::read_dir(scratch.path())?.count(), 0);
    }
    Ok(())
}
