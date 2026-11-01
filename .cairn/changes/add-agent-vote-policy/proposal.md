## Why

Review already marks authorship as declared, but policy cannot distinguish votes
from agent principals. That lets an agent approval satisfy human intent gates by
default, or forces operators to avoid agent identities entirely.

Issue: [#26](https://github.com/OnixResearch/kiln/issues/26).

## What Changes

- Add optional `agent_principals` to the code-review profile: exact write
  principals whose votes are agent votes. The list is bounded, unique and must
  contain only admitted writers.
- Add `policy.agent_votes`: `advisory` by default, or explicit `binding`.
- Advisory agent approvals are retained and displayed but never satisfy the
  ordinary approval count or an ownership rule owner item.
- Binding mode counts agent approvals exactly like human approvals.
- Agent `NeedsRevision` and `Veto` votes keep their existing blocking effect in
  either mode.
- Add core and profile-admission tests, the Nickel contract and operator docs.

This first cut configures agents explicitly in the review profile. Deriving
agents from access bindings, recording attested authorship and separate browser
identity display remain future work.
