# Verification evidence

## Result

Review policy can explicitly classify write principals as agents. Agent
approvals are advisory by default: they remain visible but cannot satisfy the
ordinary approval count or an ownership item. Explicit `binding` policy counts
them like human approvals. Agent needs-revision and veto votes still block in
either mode.

## Evidence

- Core tests cover advisory inertness, explicit binding, needs-revision blocking
  and unchanged default behavior.
- Review-profile admission tests cover valid, empty, duplicate, non-writer and
  over-bound agent principal lists.
- Review detail tests verify declared agent reviewers and votes are marked and
  the effective advisory/binding mode is reported.
- Full core and web-service suites pass.
- Clippy, rustfmt, Nickel typecheck/export and Cairn validation pass.

## Non-claims

This change configures agent identities explicitly in the review profile. It
does not yet derive them from access-profile viewer bindings, record attested
authorship (`submitted_by`) or display a separate agent identity in the browser.
No agent mode grants merge authority. Passing tests do not prove provider
execution correctness or release readiness.
