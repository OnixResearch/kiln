# Context

Kiln's existing v1 implementation publication plan and integration receipt are PR-shaped. Reinterpretation would break existing wire semantics and permit fictional PR identity. Native Git delivery therefore uses distinct schema identifiers and types; existing PR adapters and call paths stay intact.

## Decision

Model native branch publication and target integration as separate effect kinds and observations. Each native plan binds repository enrollment, exact remote profile/identity, target and candidate refs/commits/trees, expected old target, work revision, attempt/fence, exact handoff/verification/review/policy identities, provider profile, operation identity and strategy. Pending durable effects are reconciliation-only on restart. A provider observation records what remote refs actually expose; Kiln acceptance additionally requires current authority and exact lineage.

The only selected integration strategy is non-force fast-forward of an enrolled target ref. It is supported only where the selected transport can establish server-side expected-old comparison and fast-forward validation. Local target reads followed by ordinary unguarded push are insufficient. Unsupported transport/authentication/host-key/configuration fails before effects. Branch publication is independently reconciled. Lost acknowledgements remain unknown until exact remote readback. Never create a PR or silently use another route.

If target advanced before integration preparation, create a new candidate and rerun the applicable verification/review/policy gates. A stale fence or changed authority blocks effect and admission. Cleanup is never allowed to erase pending evidence.

## Verification

Build pure positive/negative transition fixtures plus durable restart/lost-ack and mismatch cases. Adapter fixtures use isolated local bare remotes and assert actual refs, race/non-FF refusal, remote identity, candidate identity and protected failures. Provide an opt-in reproducible remote qualification command which requires explicit scope/authority and reports observations, but do not run it or claim live results here. Run Cairn proposal, design and task gates before implementation; integrated checks and builds remain the parent agent's responsibility.

## Non-claims

Fixtures do not qualify real host protection, SSH key or certificate policy, credentials, server behavior, lifecycle execution, production safety or live remote completion. A local-only observation is never remote qualification.
