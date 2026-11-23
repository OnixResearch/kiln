# Fair admission delta

## MODIFIED Requirements

### Requirement: Budget lineage sent to Lattice

r[kiln.admission.lattice_lineage] When `kiln-lattice-dispatch` builds the request for a fresh dispatch, it MUST derive Lattice's `lattice.kiln-budget-lineage.v1` from the admission ledger. The root MUST be the campaign root and the account MUST be the queue row's account, with that account's parent and depth. The remainder MUST be the per-dimension sum of the attempt's owner-committed reservations for the same work, attempt and fence, with dimension names unchanged so that Lattice lists every dimension it cannot charge as unaccounted. Every owner the queue row requires MUST have committed a grant for the attempt. Every dimension Lattice charges that the account tracks MUST have a positive remainder; a dimension Lattice cannot charge that the attempt does not hold MUST be omitted. Each amount MUST fit the work budget and the root allocation. The lineage MUST be emitted only when the attempt holds a committed reservation; without one, the request MUST keep its bytes and identities. When the lineage cannot be derived consistently, the dispatch MUST be refused with a typed reason before the owner-grant gate and any transport start, and a guessed lineage MUST NOT be sent.

#### Scenario: The worker route carries the lineage

r[kiln.admission.lattice_lineage.scenario.carried]
- GIVEN an attempt whose host-owner, nested-account or shared-allocation reservation is committed
- WHEN the `--worker-request` route builds its request
- THEN the request MUST be the admitted bytes with one appended `lineage` member matching the committed golden bytes
- AND Lattice's published schema MUST accept that lineage, with the dimensions it cannot charge listed as unaccounted
- AND the sent bytes MUST be frozen atomically under their own BLAKE3 before the owner-grant gate, so a retry refused before any transport start writes its own file and a torn file under the name is replaced, and the transport intent's transaction MUST record that file's path and BLAKE3 for the launch in a record separate from the launch record, which MUST keep its published shape
- AND recovery and candidate export MUST read exactly the recorded file, whatever the current profile's output directory, and recovery MUST refuse a missing file or one that no longer hashes to the recorded BLAKE3 or is not the admitted bytes plus at most one well-formed lineage, retaining the unknown outcome

#### Scenario: The exchange route checks but never carries it

r[kiln.admission.lattice_lineage.scenario.exchange]
- GIVEN the same ledger and a `--dispatch-plan` workflow-exchange dispatch
- WHEN the route builds its request
- THEN it MUST derive the lineage with the same function and keep the `lattice.workflow-request` bytes unchanged when the lineage is consistent
- AND it MUST refuse with the same typed reason when the lineage is inconsistent

#### Scenario: Inconsistent lineage is refused before transport

r[kiln.admission.lattice_lineage.scenario.refused]
- GIVEN a tentative or unknown reservation, a reservation on another account, a required owner without a committed grant, a queue row claimed by another attempt, a grant beyond the work budget or root allocation or in an untracked dimension, a missing root or root allocation, overflowing amounts, a charged dimension the attempt does not hold, a revoked or expired lineage, a serving gate that does not admit, an uppercase identity, or an admitted request that already names a lineage
- WHEN a dispatch route builds its request
- THEN it MUST return the matching typed `LineageRefusal` and start nothing

#### Scenario: The sent request is reported read-only

r[kiln.admission.lattice_lineage.scenario.reported]
- GIVEN an attempt whose transport intent records a launch, with or without the request that launch sent
- WHEN a caller runs `kiln-lattice-dispatch --worker-request ADMITTED --sent-request true` with the ledger, enrollment and transport profile
- THEN it MUST print exactly one `kiln.lattice-sent-request.v1` object naming the request file and state root the recorded launch ran, exactly as recorded and with `recorded` true, or for a launch recorded without its request the admitted request frozen in the profile's output directory with `recorded` false, after checking that the file is a bounded ordinary file hashing to the reported BLAKE3 that `verify_frozen` accepts
- AND it MUST exit non-zero with nothing on stdout for an attempt or fence the ledger does not hold, admitted bytes that are not the request sealed at bind-dispatch, no recorded transport intent, a missing, unbounded or crossed file, or a ledger in WAL mode, which a read-only SQLite open could change by creating its `-wal` and `-shm` files
- AND it MUST write nothing: no ledger version or byte changes, no file is created, and no owner grant, credential read or Lattice start occurs
