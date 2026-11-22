## Why

Kiln #83 asks Kiln to use one admitted Cairn snapshot throughout selection,
worker context and completion explanations. Today the Cairn campaign reader
collects several native JSON projections (graph, workflow status and
instructions, readiness, implementation-ready) and campaign observation maps
their fields into coarse remaining-obligation categories. Nothing binds an
attempt to one exact versioned producer record, and Cairn #12 obligation
explanations cannot reach checks, completion or the operator surfaces.

Cairn #14 now produces `cairn.selected-work.v1`: one typed portable snapshot of
one change (optionally one task) with eight BLAKE3 cohort identities, typed
requirement and acceptance-criterion references, phase readiness, stable blockers, remaining
work, dependency facts and Cairn #12's exported obligation section (binding,
predicate, typed node facts and evaluation). Its Rust admission
(`admit_portable`) and node-fact verification live in `cairn-core`, which Kiln
already vendors for authoritative completion.

## What Changes

- Re-import `vendor/cairn-verifier` (`cairn-core`) at the producer revision that
  publishes `cairn.selected-work.v1`, with its generated policy inputs and its
  exact transitive Git dependencies (`artifact-auth-ed25519`, the historical
  `kiln-core` it pins) imported through the repo-owned importer and redirected by
  root manifest patches. The gate contract import in `vendor/cairn` is unchanged.
- Add provider-neutral selected-work types to `kiln-core`: cohort, typed
  references, readiness, blockers, remaining work, dependencies, obligation
  plan and evaluation; per-attempt snapshot contracts; refresh and replan
  transitions; current, historical and unavailable observations; a restricted
  display projection; and an obligation completion decision over canonical
  checks and authoritative completion.
- Add a `kiln-adapter-cairn` reader and admission for `cairn selected-work`
  that runs the producer's own admission and node-fact verification and maps the
  record into Kiln types. The existing multi-projection reader stays as the
  explicit `legacy_readiness_compatibility` intake profile.
- Retain the admitted snapshot and its obligation guidance projection per
  campaign member and attempt; worker context, campaign inspection and the UI
  projection read the same retained projection.
- Host-normalize admitted obligations into Lattice's
  `lattice.workflow-intent.v1` input in `kiln-adapter-lattice`, admit a compiled
  plan identity together with the selected obligations, and refuse
  agent-proposed workflow changes until re-admitted with new identities.
- Retain deterministic producer/consumer fixtures and a finite verify pilot
  (passing, missing check, stale subject, changed policy, interrupted
  observation) with stale, malformed, incomplete, mixed-cohort and
  crossed-subject refusals.

## Impact

Affected: `kiln-core` implementation model, `kiln-adapter-cairn`,
`kiln-adapter-lattice`, `kiln-implementer` campaign store, `kiln-web-service`
campaign observation and projection, vendored Cairn verifier imports, root
manifest patches and lock. No remote effect, live campaign, live Lattice
execution or provider qualification is added; real-provider qualification stays
with Kiln #49. Cairn's generic Kiln workflow importer stays non-authoritative for
completion. Closed Cairn #1 is not reopened.
