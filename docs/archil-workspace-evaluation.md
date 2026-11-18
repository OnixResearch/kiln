# Archil as an optional durable workspace provider

<!-- r[related kiln.workspace_provider.evaluation] -->
This is the documentation-review phase of
[#82](https://github.com/OnixResearch/kiln/issues/82): which parts of Kiln's
projected-workspace contract an Archil disk could supply, and which stay with
Kiln's own materialization, authority and acceptance boundaries. Every Archil
capability below is a **vendor claim** from Archil's documentation, checked on
17 November 2026; none was exercised. No Archil dependency, credential, adapter
or default is added.

## Profile contract

[`config/workspace-provider-profile.ncl`](../config/workspace-provider-profile.ncl)
defines the versioned profile `kiln.workspace-provider-profile.v1` behind the
pure-core/provider-shell boundary. Canonical source, projection and candidate
identities stay BLAKE3 and Kiln-owned; Archil disk, branch, checkpoint, mount and
sandbox identifiers are provider references bound to them. The contract refuses a
profile that:

- claims qualification from anything other than an adapter execution;
- declares checkpoints or branches on a native-format bucket disk;
- uses branch-per-attempt isolation without branch support;
- gives local materialization a provider credential;
- enforces a restricted view inside a disk while the worker holds a whole-disk
  credential;
- keeps undeletable branches or checkpoints outside a disposable-disk lifecycle;
- treats the bucket copy as a durable acknowledgement or forced ownership as a
  retry, uses forks that share an attached disk, names a non-BLAKE3 canonical
  identity, or has a zero or over-bound limit.

The fixtures in `config/fixtures/` declare the local baseline
(`workspace-provider-local-positive.ncl`), an Archil branch-per-attempt profile
and an Archil disk-per-view profile, and one negative fixture per refusal.

## Documented capabilities against the contract

| Kiln requirement | Documented Archil behavior | Consequence |
| --- | --- | --- |
| Canonical identities stay Kiln-owned | Disks, branches and checkpoints have provider names and IDs | Bind provider references to BLAKE3 identities; never adopt them as identities |
| Independent writable attempts | A branch forked from a checkpoint is isolated from its parent and siblings and can be mounted on another VM. Forking a sandbox copies its root disk, but forks mount the same attached disk | Branch-per-attempt or disk-per-attempt topologies only; a sandbox fork with an attached workspace disk shares writes |
| Restricted views | Mount authorization covers the whole file system; POSIX permissions are not an isolation boundary; credentials cover every branch and checkpoint of a disk; API keys reach every disk of the organization | A restricted view needs its own disk, or Kiln's trusted materialization boundary with no Archil credential in the worker; branches cannot restrict |
| Executable modes and origins | The mount keeps mode bits and links; the bucket copy of a bring-your-own-bucket disk drops mode bits, ownership and extended attributes, stores symlinks as text and hard links as copies | Reconstruct from Kiln's manifest or the mount, never from the bucket copy |
| Durable acknowledgement | `fsync`, `archil sync`, `archil checkin` and `archil unmount` return after writes are durable; `close` does not wait on exclusive or shared mounts | A provider sync or `fsync` observation can stand behind publication; `close` cannot |
| Reader freshness | Opening a file revalidates it; attributes are cached up to 30 s and directory listings up to 5 min unless the cache expiry is changed or invalidated | Handoff to a reader on another client must revalidate or set the expiry before reading listings |
| Bucket publication | The bucket copy starts after 15 s without a write or within 10 min of the first unsynchronized write, cannot be forced and reports no progress | Bucket export is never completion evidence; complete manifest adoption remains #80's decision |
| Stale writers | Exclusive and shared mounts allow one writer per path; a crashed client's delegations stay until revoked; forced ownership drops the holder's unsent writes and its later `fsync` fails; mount tokens can expire or be single-use | Revocation by token expiry or delegation is an effect boundary only once observed; never force ownership as an implicit retry |
| Checkpoints and branches | Require the flat-block format: managed disks on every plan, bring-your-own-bucket only on Enterprise; individual branches and checkpoints cannot be deleted | A portable native-format bucket disk cannot branch; branch experiments need a disposable disk |
| Continuation | Sandboxes pause with memory and processes; files on a disk outlive the sandbox | Retained files do not qualify agent or process continuation; that stays with Lattice #12 |

## Decision

**Retain as reference; adapt one mechanism after qualification.** The candidate
mechanism is one branch per attempt on a disposable managed flat-block disk, for
unrestricted full-source profiles only, behind Kiln's existing manifest,
candidate sealing and publication owners. Restricted profiles are ineligible
unless every view gets its own disk, because Archil documents no in-disk access
boundary. Nothing is adopted: no profile is qualified and no default changes.

## Shared corpus and local baseline

<!-- r[related kiln.workspace_provider.evaluation.scenario.baseline] -->
[`config/workspace-corpus-contracts.ncl`](../config/workspace-corpus-contracts.ncl)
types the shared corpus (`kiln.workspace-corpus.v1`). The frozen example has
two projects that share a protocol directory and its declared dependency, an
executable script, equal-byte license files at two origins, an excluded
environment file and a file neither project selects. Each projection is an
ordinary `kiln.source-projection.definition.v1` definition, and
`fixtures/workspace-corpus/corpus.json` is its checked export.

```console
cargo run -p kiln-web-service --example workspace-baseline -- \
    --corpus fixtures/workspace-corpus/corpus.json --out receipt.json
```

The local baseline writes the corpus into a temporary bare Git repository with
one commit at a fixed time. It then resolves and plans each projection with the
eager local-directory materializer profile, and materializes inputs and pinned
overlays for two attempts through #78's materializer. The
`kiln.workspace-baseline.receipt.v1` receipt records each projection's
manifest, input, entries and overlays, plus cold-input, verified-reuse and
overlay timings. For each check it records the local outcome beside an Archil
outcome of `not_run`:

| Check | Local evidence |
| --- | --- |
| `reproducible_input` | Two stores hold byte- and mode-identical inputs |
| `independent_attempts` | An edit in one attempt's overlay leaves the other overlay and the input unchanged |
| `verified_input_reuse` | A second preparation re-verifies and reuses the published input |
| `executable_modes` | Sealed 0555/0444 input modes and writable 0755/0644 overlay modes |
| `distinct_origins` | Equal bytes at two paths keep two origin identities |
| `excluded_entry_absent` | An excluded file is absent; each input holds exactly its manifest entries |
| `shared_dependency` | Shared entries and the declared dependency are bound in both projects with identical bytes |
| `damaged_input_refused` | A file changed after publication is refused on reuse |
| `exact_pin_reopen` | An overlay reopens with its edits only under its exact dispatch pin |

The `workspace_baseline` test compares the receipt, without timings, to
`fixtures/workspace-corpus/expected.json`; timings are recorded, not asserted.

## Remaining work

- The baseline materializes its corpus without building it. #77's build-input
  qualification ([operator guide](operator.md#source-projection-definitions))
  builds a separate two-project Cargo corpus through the same materializer; an
  Archil comparison of builds would reuse that corpus.
- The live phase needs an explicitly configured disposable Archil environment:
  region, client and SDK versions, storage format, mount mode, cache policy and
  named time, resource, cost and retention bounds. It must exercise independent
  attempts, sealing and reopening on another worker, restricted-view refusals,
  stale writers, revoked tokens, lost acknowledgements, partition and partial
  publication, and measure preparation, command completion, durable publication,
  checkpoint and branch creation and verified reopening separately. Until then
  each Archil profile stays a vendor claim.

## Non-claims

No Archil account, credential, disk, sandbox or network call was used. The
profile contract checks declared capabilities, not provider behavior. Vendor
latency, durability and isolation figures are not Kiln measurements.

## References

Archil documentation, checked 17 November 2026:
[architecture](https://docs.archil.com/details/architecture),
[consistency](https://docs.archil.com/details/consistency),
[authorization](https://docs.archil.com/concepts/authorization),
[branches and checkpoints](https://docs.archil.com/concepts/branches-and-checkpoints),
[sandboxes](https://docs.archil.com/compute/sandboxes/introduction) and
[data ownership](https://docs.archil.com/reference/data-ownership).
