# Nix Store Facts Specification

## Purpose

Defines the `nix-store-facts` capability.

## Requirements

### Requirement: Permissive bounded facts crate

r[kiln.nix_store.facts]
Kiln MUST provide a pure, bounded Nix-store facts crate built from Apache-2.0
`nix-derivation`, `nix-archive` and `nix-narinfo` dependencies. It MUST NOT link
GPL-3.0 Snix crates into Kiln artifacts. The crate MUST perform no ambient I/O;
adapters supply exact bytes, paths and bounds.

#### Scenario: GPL boundary

r[kiln.nix_store.facts.scenario.license]
- GIVEN the Kiln workspace dependency graph
- WHEN Nix store facts are built
- THEN no Snix crate is present and the facts crate declares only permissive
  dependencies.

### Requirement: Additive core build contracts

r[kiln.nix_store.contracts]
`BuildEffectPlan` MAY carry typed constraints and `BuildEvaluationObservation`
MAY carry product store objects, input-closure facts and explicit reference/
input completeness flags. Every new field MUST be
optional or defaulted and omitted when absent so earlier serialized plans and
observations keep their bytes. Kiln BLAKE3 identities MUST remain separate from
native NAR/SHA-256 facts.

#### Scenario: Absent fields

r[kiln.nix_store.contracts.scenario.absent]
- GIVEN an earlier plan or observation without the new fields
- WHEN it is deserialized and serialized
- THEN no new fields appear.

### Requirement: Bounded derivation admission

r[kiln.nix_store.derivation]
Before dispatch, the Nix build path MUST parse the exact `.drv` bytes under a
named byte bound and validate the derivation structure. It MUST require the
admitted platform to equal the derivation system, require every requested output
name to be declared, and reject duplicate, missing or unexpected products. It
MUST compare declared output paths with the task/store binding when the
derivation declares them. It MUST record the exact derivation identity and the
declared input sources and input derivations. Where input-addressed output paths
need input-modulo hashes that are not available, admission MUST mark them
task-bound rather than claim recomputation.

#### Scenario: Mismatched derivation

r[kiln.nix_store.derivation.scenario.mismatch]
- GIVEN a `.drv` whose system, output name or declared output path differs from
  the admitted profile/task
- WHEN admission runs
- THEN it MUST fail before process start and name the mismatched field.

#### Scenario: Valid derivation

r[kiln.nix_store.derivation.scenario.valid]
- GIVEN a structurally valid `.drv` matching the platform and requested outputs
- WHEN admission runs
- THEN it MUST return bounded output, input and system facts and the exact
  derivation identity.

### Requirement: NAR and store-object facts

r[kiln.nix_store.nar]
Product observations MAY carry NAR size, NAR SHA-256, references, deriver,
content address and signatures. Facts MUST be computed from exact output trees
or supplied native metadata under named bounds. Native SHA-256 and NAR spellings
MUST remain distinct from Kiln BLAKE3 identities. A NAR fact MUST NOT claim build
correctness, reproducibility, cache truth or atomic tree snapshot.

#### Scenario: Product NAR

r[kiln.nix_store.nar.scenario.product]
- GIVEN a realized product path with an admitted bound
- WHEN store facts are observed
- THEN the NAR size/hash and references MUST be reported, and an over-bound tree
  MUST fail explicitly without partial facts.

### Requirement: Reference scanning and completeness

r[kiln.nix_store.refscan]
Reference scanning MUST use exact 32-byte Nix-base32 hash parts for every
provable input source, known input derivation and output path. The observation
MUST list matched candidate indexes and MUST state whether the candidate set is
complete. An incomplete candidate set MUST NOT be presented as a complete
reference closure.

#### Scenario: Unresolved input candidates

r[kiln.nix_store.refscan.scenario.incomplete]
- GIVEN a derivation with an input derivation whose output paths cannot be
  resolved
- WHEN references are scanned
- THEN matches MUST be reported with `complete = false`.

### Requirement: Typed build constraints

r[kiln.nix_store.constraints]
Build plans MUST carry a bounded, typed constraint set covering system, minimum
memory, available read-only paths, network access and shell provision, with
canonical ordering and no contradictory duplicates. Absent constraints MUST
serialize as before. Constraints MUST be admitted from exact profile/derivation
facts, never inferred from ambient host state.

#### Scenario: Contradictory constraints

r[kiln.nix_store.constraints.scenario.invalid]
- GIVEN duplicate systems, unsorted read-only paths or conflicting network
  declarations
- WHEN a plan is admitted
- THEN it MUST fail without dispatch.

### Requirement: Read-only cache projection

r[kiln.nix_store.cache]
Kiln MUST be able to render a bounded static Nix binary-cache projection from an
explicit observation and realized output paths: `nix-cache-info`, canonical
`.narinfo` records and NAR bytes. It MUST verify supplied or recomputed NAR
hashes and sizes, MUST reject unsafe paths, unknown schema, duplicates and
bounds, and MUST NOT serve network requests, sign with operator keys, substitute,
or mutate a store.

#### Scenario: Cache projection round trip

r[kiln.nix_store.cache.scenario.roundtrip]
- GIVEN an admitted observation with a realized output path
- WHEN the cache projection renders
- THEN the narinfo parses back to the same store path, NAR hash/size and
  references, and the NAR bytes hash to the recorded value.

### Requirement: Compatibility and non-claims

r[kiln.nix_store.compat]
Observations written before these optional fields MUST replay with unchanged
bytes and behavior. New facts MUST remain separate from canonical Kiln BLAKE3
identities. Passing these checks proves only the observed derivation/NAR/
projection facts, not Nix equivalence, hermeticity, build correctness, source
trust, cache truth or release eligibility.

#### Scenario: Old observation

r[kiln.nix_store.compat.scenario.old]
- GIVEN a build observation without constraints, store objects or inputs
- WHEN it is read and re-serialized
- THEN no new fields appear and its behavior is unchanged.
