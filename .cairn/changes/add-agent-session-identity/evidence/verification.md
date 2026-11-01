# Verification evidence

## Result

Authenticated viewer and tailnet principals now carry their declared agent
identity. `/api/session` exposes it only when present, and the workspace shows
the declared harness/model in principal facts. No permissions or review
authority change.

## Evidence

- The session regression test verifies the agent field and unchanged read-only
  permissions.
- Full core and web-service suites pass.
- Clippy, rustfmt, JavaScript syntax, Nickel typecheck/export and Cairn
  validation pass.

## Non-claims

The declaration is configuration, not verified execution proof. Signed
per-request agent credentials remain future work, and the workspace label grants
no authority or permission.
