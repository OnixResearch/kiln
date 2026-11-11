## Why

Agent principals are attested, and access bindings now declare a harness and
optional model, but revisions retain only the principal id. Review cannot show
the declared agent identity that produced a patch set, and the review Nickel
contract previously dropped agent profile fields from its exported output.

Issue: [#26](https://github.com/OnixResearch/kiln/issues/26).

## What Changes

- Add `AgentIdentity` and structured `AttestedSubmission` to the core revision.
- Add trusted `agent_identities` facts; attest revisions with the declared
  harness/model when available.
- Add optional review-profile `agent_metadata`, bounded and restricted to
  declared agent principals.
- Merge access agent declarations into review metadata with duplicate
  suppression and conflicting-metadata rejection.
- Export `agent_principals` and `agent_metadata` from the review Nickel profile.
- Show the declared harness/model beside the attested principal in the review
  identity view.
- Add core, profile admission, merge and contract tests.

Declared metadata remains configuration, not proof that a particular model or
harness executed.
