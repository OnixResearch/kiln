# Implementation audit

## Goal

Implement a provider-neutral CI core and standalone shell without provider-runtime authority in `kiln-core`.

Completion evidence is a passing workspace test, strict Clippy, `no_std` build, Nickel checks, dependency audit, Cairn validation, and Nix flake check.

False completion includes provider imports in core, mutable shell state machines, silent fallback, collapsed unknown outcomes, status authority, or remote publication.

## Serial approach registry

These analysis passes are correlated. Deterministic repository checks remain authoritative.

| Family | Mechanism | State | Evidence |
|---|---|---|---|
| Pure event core | One `decide` and `evolve` path | selected | Domain tests cover deterministic start, terminal rejection, retry identity, and unknown outcomes. |
| Generic workflow core | Replace CI concepts with workflow objects | rejected | It would transfer Lattice or Choregraph semantics into Kiln. |
| Narrow provider ports | Versioned role and capability admission | selected | Provider tests reject role mismatch, unsupported capability, and provider substitution. |
| Universal provider object | One source, forge, workflow, and build interface | rejected | It erases native identities and authority boundaries. |
| Transactional reference shell | Commit events and outbox records together | selected | Interruption tests record neither side before commit. |
| Provider-owned state | Treat callbacks or status COBs as canonical state | rejected | Projection and COB tests retain external disagreement without state mutation. |

## Adversarial audit

The audit targets these risks:

- terminal mutation and reused attempt identities;
- stale command revisions and duplicate events;
- provider substitution and missing capabilities;
- ambiguous external outcomes;
- moving source references and divergent Jujutsu changes;
- candidate cycles, moved targets, and stale gate results;
- missing capacity facts and non-monotonic fairness;
- log gaps, cursor mismatch, client deadlines, and secret disclosure;
- stale COB heads and partial conflict resolution;
- remote cache overclaim and identity confusion;
- interchange builder confusion and authority promotion;
- publication rollback, mirror writes, and mixed bundles.

The implementation does not perform Radicle publication. Publication remains blocked until an operator grants explicit authority.
