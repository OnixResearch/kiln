# Verification evidence

## Result

Revisions submitted by a declared agent principal now record that exact
principal in `submitted_by`. An agent principal declaring human authorship is
rejected before mutation. Human and unattested submissions remain
byte-compatible. The review list, header and identity view show the attested principal
separately from self-declared, unverified authorship.

## Evidence

- Focused core tests cover agent attestation, human-declared rejection without
  mutation and unchanged human submissions.
- Full core and web-service suites pass.
- Clippy, rustfmt, JavaScript syntax and Cairn validation pass.

## Non-claims

Authorship remains self-declared and unverified; principal attestation does not
verify execution. Harness/model recording is extended by
`add-attested-agent-metadata`; signed per-request agent credentials remain
future work.
The browser label grants no authority or merge effect.
