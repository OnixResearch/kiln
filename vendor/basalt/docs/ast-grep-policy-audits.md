# ast-grep Policy-Boundary Audits

Basalt uses ast-grep as a bounded structural signal around its UCAN/Nickel/Steel
policy boundaries. ast-grep provides fast syntax-level audits of policy
enforcement call sites, shell/core separation, Steel adapter authority, and
CLI-only effect leaks. It is candidate boundary evidence, not authorization
proof.

## Rule pack

Advisory ast-grep rule profiles live in `tools/ast-grep/rules/`, configured by
`tools/ast-grep/sgconfig.yml`. Positive and negative Rust fixtures live in
`tools/ast-grep/fixtures/`. A rule may only move from advisory to release
blocking carry posture when it has both a positive and a negative fixture.

## Pure core

`ast_grep_audit` validates the rule pack shape, enforces the blocking-requires-
fixtures boundary, and binds a deterministic BLAKE3 rule-pack identity plus
explicit non-claims into the audit receipt. It performs no process execution,
no filesystem scan, and consumes no ast-grep runtime state. Release verifier
shell code owns running ast-grep and capturing candidate findings.

## Release posture

The ast-grep policy-boundary audit is advisory by default. Scan findings are
reported in release verification receipts as structural candidate evidence with
remediation guidance, and they never block release by default.

## Claim boundary

A clean or finding-free ast-grep scan does not authorize a request nor prove
policy safety. It does not replace Nickel policy contracts, generated policy
freshness, UCAN proof-chain verification, or Steel adapter DTO validation.
Findings identify source shapes only around declared policy boundaries.
