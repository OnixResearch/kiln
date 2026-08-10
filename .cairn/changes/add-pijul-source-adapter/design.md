## Context

Kiln has a neutral source model and a Jujutsu adapter boundary. The source model
also names Pijul, but only one profile-name test covers it.

Pijul uses repositories, channels, states, and content-addressed changes. A
channel state cannot become a Git commit or branch without losing native
meaning. The adapter must preserve Pijul facts and still produce Kiln's common
immutable source resolution.

## Goal and Success Contract

The goal is a versioned, read-only Pijul source adapter for local repositories.

Completion requires these results:

- `kiln-adapter-pijul` exists outside the core crates.
- The adapter opens one explicit local repository and resolves one channel.
- The result binds an exact native state identity and bounded change facts.
- Resolution and acquisition use one consistent repository view.
- The snapshot produces a deterministic canonical manifest and BLAKE3 identity.
- The profile admits only discovery, exact resolution, acquisition, and identity
  observation.
- Missing channels, moving states, malformed repositories, unsupported formats,
  and exceeded bounds fail without partial success.
- Positive and negative tests use local repositories created by the pinned
  Pijul cohort.
- Core and standalone builds remain independent of Pijul runtime software.

False completion includes these results:

- adding only another profile-name fixture;
- adding only a trait with no concrete repository shell;
- parsing human-oriented command output without a versioned protocol contract;
- using the latest channel name as the immutable revision;
- using one change hash as the complete channel state;
- computing the tree identity from provider metadata instead of acquired bytes;
- importing `libpijul` into a core crate;
- enabling source mutation through the read-only profile; or
- claiming history or trust from equal snapshot bytes.

## Approach Registry

These serial analysis passes are correlated. Repository gates remain the
completion authority.

| Family | Mechanism | State | Reason |
|---|---|---|---|
| model only | Add Pijul variants and fixtures to `kiln-core` | rejected | This does not resolve or acquire a repository. |
| trait only | Add a `PijulRepository` trait with fixture implementations | rejected | This repeats the current gap and provides no usable shell. |
| command shell | Run the Pijul CLI and parse its normal output | rejected for v1 | Human output can drift and can hide exact state facts. |
| library shell | Use a pinned `libpijul` cohort in an optional adapter crate | selected | It exposes native repository facts without provider types in the core. |
| remote service | Call a separate Pijul snapshot service | deferred | It adds transport and service authority that local source acquisition does not need. |

The adversarial audit targets channel movement, inconsistent reads, missing
changes, damaged repositories, oversized histories, unsafe paths, format drift,
and accidental mutation authority.

## Decisions

### Decision: Add an optional native library shell

**Choice:** Add `crates/kiln-adapter-pijul`. Its standard-library shell uses one
pinned Pijul and `libpijul` release cohort.

The adapter defines a narrow repository port for pure mapping tests. It also
contains a concrete implementation for local repositories. Provider handles,
transactions, database types, locks, and errors stay inside the adapter crate.

**Rationale:** The library exposes native identities directly. It avoids a text
protocol that can change with user-facing CLI output.

### Decision: Keep the first profile read-only

**Choice:** The `pijul-source-v1` profile supports `Discover`, `ResolveExact`,
`Acquire`, and `ObserveIdentity` only.

The adapter does not request credentials. It does not expose record, apply,
unrecord, channel mutation, push, pull, or clone operations.

**Rationale:** Snapshot acquisition needs repository read authority only. Source
mutation needs separate effects, credentials, and failure contracts.

### Decision: Represent Pijul facts explicitly

**Choice:** Add bounded Pijul facts for these values:

- repository identity;
- requested channel name;
- provider-produced immutable state identity;
- exact observed change frontier or equivalent pinned native state facts;
- Pijul and repository format cohort; and
- observation identity for the consistent read.

Refactor provider-specific source facts into one explicit enum if this gives a
cleaner invariant than multiple optional fields. Update the Jujutsu adapter and
its tests in the same task.

The pure core rejects missing state identities, empty required facts, wrong
namespaces, duplicate facts, exceeded bounds, and mixed provider facts.

**Rationale:** A common tree identity does not replace provider history facts.
One tagged provider-fact value also prevents impossible mixed states.

### Decision: Resolve and acquire from one consistent view

**Choice:** The shell opens the selected repository and starts one supported
read view. It resolves the channel state and acquires the snapshot through that
view.

If the selected library cannot provide a stable read view, the shell records
the state before and after acquisition. It rejects a changed state.

The shell returns no successful resolution until all bytes and native facts are
available and within bounds.

**Rationale:** A channel can move between separate reads. A mixed snapshot and
state identity is not an immutable resolution.

### Decision: Compute identity from canonical snapshot entries

**Choice:** The shell converts acquired content into bounded snapshot entries.
A pure function checks paths, entry kinds, ordering, duplicate paths, sizes, and
total bounds.

The canonical manifest uses explicit framing and deterministic path order. The
shell computes the BLAKE3 identity from this manifest.

Symlinks, subrepositories, special files, and unsupported entry kinds fail
unless the profile defines their exact canonical form.

**Rationale:** Provider state identifies Pijul history. Kiln's tree identity
identifies the admitted source bytes.

### Decision: Use typed Nickel configuration

**Choice:** A typed Nickel profile records these values:

- profile and contract versions;
- Pijul and `libpijul` versions;
- source revision and repository format cohort;
- enabled capabilities;
- maximum changes, entries, path bytes, file bytes, and total bytes;
- allowed entry kinds and platform set; and
- non-claims.

Missing values, zero bounds, unsupported capabilities, or version drift fail
before repository access.

**Rationale:** The adapter compatibility surface must be reviewable before the
shell opens a repository.

### Decision: Keep provider failures explicit

**Choice:** The shell maps provider failures into stable diagnostic classes.
Classes include unavailable repository, missing channel, missing change,
damaged state, unsupported format, busy repository, exceeded bound, changed
state, unsupported entry, and provider error.

The pure result contains no partial success. A provider error cannot become an
empty snapshot or a safe retry decision.

**Rationale:** Provider failures have different operator actions. One generic
not-found result can hide damage or drift.

## Functional Core and Imperative Shell

Pure functions own:

- Pijul profile and capability admission;
- provider-fact validation;
- exact-state consistency decisions;
- snapshot-entry validation and ordering;
- canonical manifest framing;
- BLAKE3 identity input construction; and
- stable diagnostic classification from bounded adapter facts.

The shell owns:

- repository opening and locking;
- `libpijul` calls and provider transactions;
- channel and change observation;
- snapshot byte reads;
- filesystem staging when required;
- provider error capture; and
- conversion into bounded core inputs.

No pure core function reads files, opens a repository, starts a process, or
uses provider types.

## Data Flow

```text
explicit repository root + channel + admitted profile
  -> local Pijul shell opens one consistent read view
  -> exact Pijul state and bounded native facts
  -> acquired bounded snapshot entries
  -> pure canonical manifest construction
  -> BLAKE3 tree identity
  -> pure SourceResolution validation
```

## Validation

Positive tests cover:

- one repository with one channel and one change;
- one channel with dependent changes;
- two channels with different native states;
- equal snapshot bytes with different Pijul histories; and
- repeated acquisition with identical native and canonical identities.

Negative tests cover:

- a missing repository or channel;
- a missing or damaged change;
- a channel that changes during acquisition;
- a wrong provider namespace or format cohort;
- duplicate or unsafe snapshot paths;
- unsupported entry kinds;
- exceeded change, entry, file, path, or total bounds; and
- an unavailable or incompatible provider library.

The fixture gate creates repositories with the pinned Pijul cohort. Static
handwritten fixtures cannot be the only interoperability evidence.

## Risks and Trade-offs

- `libpijul` can change its API or repository format. Exact pins and drift tests
  make this change visible.
- Native library dependencies increase adapter build cost. Core and standalone
  packages remain independent.
- A consistent read view can hold repository resources. Named bounds and prompt
  release limit this cost.
- Snapshot canonicalization can mishandle provider entry types. Unsupported
  types fail until the profile defines them.
- Local fixtures do not prove remote availability or history completeness.

## Claim Boundary

Passing this change proves only bounded local repository observation, exact
state binding, snapshot acquisition, and canonical identity under the pinned
profile.

It does not prove source trust, signature validity, complete history, remote
availability, collaboration status, mutation success, or equivalence with Git
or Jujutsu history.
