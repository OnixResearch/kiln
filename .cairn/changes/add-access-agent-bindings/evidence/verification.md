# Verification evidence

## Result

Access viewer bindings may declare an agent harness and optional model. The
service derives the exact `viewer:<id>` principal and merges it into the review
agent list before review admission, so advisory/binding vote policy and
submission attestation use the access-declared identity. Non-writer agents fail
review startup.

## Evidence

- Access admission tests cover valid declarations, collection and invalid
  empty/untrimmed/control/over-bound text.
- Full core and web-service suites pass.
- Clippy, rustfmt, Nickel typecheck/export and Cairn validation pass.

## Non-claims

The harness/model declaration is configuration, not verified execution
metadata. Harness/model recording is implemented by the follow-up
`add-attested-agent-metadata` change. Signed per-request agent credentials
remain future work. The declaration grants no review, execution or
merge authority by itself.
