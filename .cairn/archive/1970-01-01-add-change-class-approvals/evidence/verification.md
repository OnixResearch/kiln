# Verification evidence

## Result

Ownership policies may declare ordered change classes. A docs-class change can
explicitly use `evidence_only` for a matching rule: its evidence still gates
readiness, but that rule's owner vote is not required. A logic change, or a
rename crossing the class boundary, keeps owner approval. Ordinary approvals,
vetoes, threads, dependencies, CI and merge preflight are unchanged.

## Evidence

- Focused core tests cover docs evidence-only approval, logic fallback,
  current/old path classification and bounded/explicit policy admission.
- Full core and web-service suites pass.
- Clippy, rustfmt, Nickel typecheck/export and Cairn validation pass.

## Non-claims

Class selection is policy-glob only. Empty or incomplete changed-file facts fail
ownership before classification. Evidence-only never grants merge authority or
bypasses ordinary review policy. Passing tests do not prove provider execution
correctness or release readiness.
