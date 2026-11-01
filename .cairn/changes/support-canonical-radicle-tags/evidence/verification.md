# Verification evidence

## Result

Radicle source admission accepts matching `refs/tags/` references. The signed
and canonical targets must agree; both are peeled to a commit for
`expected_revision` and snapshot acquisition. Annotated-tag canonical evidence
retains the tag object, kind and peeled revision. Existing branch receipts and
older evidence without `revision` keep their behavior.

## Evidence

- Source-contract tests pass, including matching tag-reference admission and
  empty branch/tag name rejection.
- Native canonical tests pass for branches, lightweight and annotated tags,
  tag alias resolution and a non-commit tag target; the positive tag test asserts a commit snapshot and tag/revision
  evidence, and the negative test fails before snapshot acquisition.
- Full workspace tests and workspace-wide Clippy pass.
- `cargo fmt` and Cairn validation pass.

## Non-claims

This does not prove tag-object reachability beyond the verified signed
references, global freshness, tag trust, peer convergence or release
eligibility. It does not add arbitrary Radicle refs or patch CI.
