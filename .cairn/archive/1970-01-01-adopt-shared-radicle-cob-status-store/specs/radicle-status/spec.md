# Radicle Status Shared-Store Delta

## ADDED Requirements

### Requirement: Status storage binds an immutable shared cohort

r[kiln.radicle.status_store_pin] Kiln's status and declared patch-review adapter MUST consume an exact, locally available vendored source archive of `onix-radicle-cob-git-store` v0.2.0 bound to a recorded Git source commit and verified archive BLAKE3. It MUST match the adapter's existing COB 0.21.1, crypto 0.19.0, and git2 0.21.0 cohort and MUST NOT resolve runtime dependencies from a sibling path or a mutable source name. An import of an existing vendor snapshot MUST verify its bytes against the declared pin and MUST NOT replace edited files.

#### Scenario: Vendor source identity drifts

- GIVEN the declared source commit, archive digest, or vendored bytes differ
- WHEN source import or dependency admission runs
- THEN the mismatch MUST fail before the Kiln adapter is accepted.

### Requirement: Shared Git mechanics do not own Kiln policy

r[kiln.radicle.status_store_boundary] The shared store MUST own the local signed COB Git ref and storage-trait mechanics for both status and declared patch review; Kiln MUST retain CI state, status reducer, patch review policy, allowlists, SecretSpec signer resolution, diagnostic classification, and provider observation authority behind the existing adapter boundary.

#### Scenario: Core and signer remain independent

- GIVEN a core-only build or an unresolved signer declaration
- WHEN the status integration is checked or status dispatch is admitted
- THEN core MUST NOT import the shared provider store and unresolved signing MUST fail before a COB write.

### Requirement: Exact expected-head CAS survives shared storage

r[kiln.radicle.status_store_concurrency] Updating one status ref MUST compare against the precise preflight-admitted expected ref target, not merely a parent of the new commit. Creating a ref MUST not replace an existing ref. A failed or ambiguous post-store ref update MUST remain unknown until read-only reconciliation; a rejected stale plan MUST not sign or create a substitute status.

#### Scenario: Reference moves after preflight

- GIVEN a previously admitted head and a different current ref target before the final update
- WHEN the shared store publishes the new change
- THEN its exact expected-target comparison MUST refuse to replace the moved ref
- AND Kiln MUST classify the post-store ref result as unknown for reconciliation rather than report a successful write.

### Requirement: Status discovery remains bounded by distinct objects

r[kiln.radicle.status_store_scan] Before first-write replay search, the adapter MUST reject zero bounds and stop once the declared positive maximum of distinct COB object identities is exceeded, without materializing an unbounded all-types object map. Refs from different namespaces for the same object MUST count once. Existing stable diagnostic codes for bound, malformed/unreadable refs, foreign or multiple heads, replay conflicts, and missing/stale objects MUST remain classified at Kiln's adapter boundary.

#### Scenario: More distinct objects than admitted

- GIVEN local refs identify one more distinct ci-job object than the profile maximum
- WHEN replay discovery runs
- THEN Kiln MUST return `radicle_object_scan_bound` without publishing a status.

### Requirement: Shared-store cutover has native fixture evidence

r[kiln.radicle.status_store_fixtures] Native ephemeral Git/COB fixtures with generated test-only signers MUST exercise create, exact update, replay, reconciliation, foreign and multiple heads, stale or racing refs, duplicate-namespace distinct bounds, malformed refs, invalid signatures, and ambiguous publication with positive and negative assertions. No fixture may use an operator key or hosted ref.

#### Scenario: Cutover native conformance runs

- GIVEN an isolated bare repository and a test-only signer
- WHEN the adapter executes create, replay, and exact update followed by negative cases
- THEN the fixture MUST establish the declared local observation and deny/unknown behavior without network or operator mutation.

### Requirement: Local status evidence preserves non-claims

r[kiln.radicle.status_store_nonclaims] A passing pinned-source check, local Git ref update, signature test, or status fixture MUST NOT be interpreted as actor authorization, a stable filesystem snapshot, universal concurrency safety, peer convergence, remote visibility, CI correctness, or release readiness.

#### Scenario: Local write is overclaimed

- GIVEN a passing local status fixture
- WHEN its evidence is described
- THEN the description MUST stay scoped to the tested local repository and explicit source cohort.
