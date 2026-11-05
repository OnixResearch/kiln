# Verification evidence

## Result

Exact CI facts now carry typed checks derived from the run's workflow nodes and
retained reports. The review panel shows each check's name, kind, outcome and
artifact digest. Policies can require a known check kind and optional name; only
a passing check on the exact reviewed commit satisfies it. Old serialized CI
facts and policies remain byte-compatible when the new fields are empty.

## Evidence

- All 187 core library tests pass, including exact-commit/kind and policy
  validation cases.
- All 167 web-service library tests pass (1 ignored).
- All 26 code-review integration tests pass.
- Web-service `--all-targets` and workspace `--all-targets` checks pass.
- The review JavaScript parses with `node --check`.
- Formatting checks pass.
- `cairn validate` reports zero issues.

## Non-claims

Agent comment outcome measurement from issue #35 remains unimplemented. Typed
checks are evidence and do not grant gate, merge or release authority. The node
kind is used as the check kind; policy requirements admit only `test`,
`property`, `differential`, `proof`, and `build`.
