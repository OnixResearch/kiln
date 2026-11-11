# Implementation work admission

Kiln can run exact candidate CI, but logged intent is not yet an admitted
implementation job. `kiln-core` now owns a bounded, versioned admission contract
that a future worker, Lattice attempt, and forge publisher can consume. It is a
boundary only: admission does not claim, execute, verify, publish, or complete
work.

## Contract

`ImplementationWork` binds:

- the canonical work identity and canonical alias key;
- exact source aliases and their source versions (`github` issue or `cairn`
  task), each a BLAKE3 of the bytes the adapter admitted;
- enrolled repository, project, and actor;
- title, requested outcome, and reviewed acceptance obligations;
- dependency identities, permitted repository-relative paths, and permitted
  effect classes;
- verification profile, attempt budget, terminal policy, and priority.

Identities are canonical BLAKE3 over a length-prefixed field encoding. The
canonical alias key excludes content, so versions of one source unit share a
key and are ordered by an adapter-supplied monotonic `revision`.

## Admission behavior

- A duplicate observation returns the retained identity; a lower revision is
  `stale`; an equal revision with different content is refused as ambiguous.
- A higher revision supersedes the retained version. Both versions remain
  auditable; the old snapshot is never rewritten.
- An explicit `SourceMapping` links one issue to one or more task-level units.
  Every mapped alias must appear in the admitted aliases; duplicate or forged
  mappings, unrelated same-title work, and partial mappings are refused.
- Missing dependencies, empty acceptance obligations, and dependency cycles are
  visible `blocked` reasons. Blocked work is never ranked.
- Only ready work is ranked, by reviewed priority and then canonical key.
- Unenrolled repositories or actors, unknown schemas, unsafe paths, unknown
  effects, missing budgets, and oversized fields fail admission before any
  version is retained.

Archive readiness is deliberately absent. An implementation-ready Cairn task
can be admitted while archive readiness is false, and an archived or deleted
change never proves implementation completion.

## What is not implemented yet

The adapters that produce these records are separate work:

- GitHub issue webhook admission and bounded polling recovery. The existing
  GitHub adapter authenticates push and pull-request events; issue events are
  not yet admitted or wired to this contract.
- Cairn intake. It must consume Cairn `OnixResearch/cairn#1`'s versioned
  implementation-ready snapshot; that producer is unimplemented. This contract
  intentionally does not fork Cairn lifecycle or readiness semantics.
- The Mneme OpenSpec consumer qualification.
- Claims, execution, candidate acceptance, and publication (issues #45–#48).

Until those land, this module is a pure, tested boundary with no dispatch
authority.
