//! Real-`jj` fixture tests for the read-only observation shell.
#![allow(clippy::expect_used, clippy::panic)]
use std::path::PathBuf;

use kiln_adapter_jj::cli::{self, Observation};
use kiln_adapter_jj::fixtures::Fixture;
use kiln_core::types::CoreResult;

fn scratch(name: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("kiln-jj-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base.join("repo")
}

fn code(result: CoreResult<Observation>) -> String {
    match result {
        Ok(_) => "ok".into(),
        Err(error) => error.code,
    }
}

// r[verify kiln.review.jj_intake.scenario.rewritten]
// r[verify kiln.review.jj_intake.read_only.scenario.unchanged]
#[test]
fn observes_exact_stack_and_evolog_predecessors_without_mutation() {
    let fixture = Fixture::new(&scratch("stack"));
    let first = fixture.commit("a.txt", "a1\n", "first");
    let second = fixture.commit("b.txt", "b1\n", "second");
    let before_rewrite = fixture.commit_id(&first);
    fixture.rewrite(&first, "a.txt", "a2\n");
    let ids = vec![first.clone(), second.clone()];
    let before = fixture.fingerprint();
    let observation = cli::observe_changes(&fixture.workspace(), &ids).expect("observed");
    assert_eq!(
        fixture.fingerprint(),
        before,
        "observation must not mutate jj state"
    );
    let [lower, upper] = observation.changes.as_slice() else {
        panic!("two changes expected: {observation:?}");
    };
    assert_eq!(lower.change_id, first);
    assert_eq!(upper.change_id, second);
    assert_eq!(lower.commit, fixture.commit_id(&first));
    assert!(
        lower.evolog.predecessors.contains(&before_rewrite),
        "rewritten change records its previous commit as a jj predecessor"
    );
    assert!(lower.evolog.operation.is_some());
    assert_eq!(
        observation.git_store,
        fixture.root.join(".git").canonicalize().expect("git store")
    );
    assert_eq!(observation.operation.len(), cli::OPERATION_ID_HEX);
    // A prefix is never resolved, even when unambiguous.
    assert_eq!(
        code(cli::observe_changes(
            &fixture.workspace(),
            &[first.chars().take(12).collect::<String>()]
        )),
        "change_id_invalid"
    );
    assert_eq!(fixture.fingerprint(), before);
}

// r[verify kiln.review.jj_intake.admission.scenario.divergent]
// r[verify kiln.review.jj_intake.read_only.scenario.unchanged]
#[test]
fn refuses_divergent_change_and_divergent_operation_heads_read_only() {
    let fixture = Fixture::new(&scratch("divergent"));
    let change = fixture.commit("a.txt", "a\n", "leaf");
    let operation = fixture.jj(&["op", "log", "--no-graph", "-n1", "-T", "id"]);
    fixture.jj(&["describe", "-r", &change, "-m", "leaf one"]);
    fixture.jj(&[
        "--at-op", &operation, "describe", "-r", &change, "-m", "leaf two",
    ]);
    // Two operation heads now exist; Kiln must refuse without merging them.
    let before = fixture.fingerprint();
    assert_eq!(
        code(cli::observe_changes(
            &fixture.workspace(),
            &[change.clone()]
        )),
        "operation_divergent"
    );
    assert_eq!(fixture.fingerprint(), before);
    // A fixture command merges the heads; the change is now divergent.
    fixture.jj(&["op", "log", "-n1"]);
    let before = fixture.fingerprint();
    assert_eq!(
        code(cli::observe_changes(&fixture.workspace(), &[change])),
        "divergent_change"
    );
    assert_eq!(fixture.fingerprint(), before);
}

// r[verify kiln.review.jj_intake.admission.scenario.conflicted]
#[test]
fn refuses_conflicted_bookmark_and_conflicted_commit() {
    let fixture = Fixture::new(&scratch("conflicted"));
    let left = fixture.commit("a.txt", "left\n", "left");
    fixture.jj(&["new", "main"]);
    let right = fixture.commit("a.txt", "right\n", "right");
    fixture.jj(&["new", "main"]);
    let other = fixture.commit("c.txt", "c\n", "other");
    fixture.jj(&["bookmark", "create", "topic", "-r", &other]);
    let operation = fixture.jj(&["op", "log", "--no-graph", "-n1", "-T", "id"]);
    fixture.jj(&["bookmark", "set", "topic", "-r", &left, "--allow-backwards"]);
    fixture.jj(&[
        "--at-op",
        &operation,
        "bookmark",
        "set",
        "topic",
        "-r",
        &right,
        "--allow-backwards",
    ]);
    fixture.jj(&["op", "log", "-n1"]);
    let before = fixture.fingerprint();
    assert_eq!(
        code(cli::observe_changes(&fixture.workspace(), &[left.clone()])),
        "conflicted_bookmark"
    );
    assert_eq!(
        code(cli::observe_changes(&fixture.workspace(), &[other])),
        "ok"
    );
    assert_eq!(fixture.fingerprint(), before);
    fixture.jj(&[
        "bookmark",
        "set",
        "topic",
        "-r",
        &right,
        "--allow-backwards",
    ]);
    fixture.jj(&["rebase", "-r", &left, "-d", &right]);
    assert_eq!(
        code(cli::observe_changes(&fixture.workspace(), &[left])),
        "conflicted_commit"
    );
    assert_eq!(
        code(cli::observe_changes(
            &fixture.workspace(),
            &["kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk".to_owned()]
        )),
        "change_missing"
    );
}
