# Fair admission delta

## ADDED Requirements

### Requirement: Budget lineage sent to Lattice

r[kiln.admission.lattice_lineage] When `kiln-lattice-dispatch` builds the request for a fresh dispatch, it MUST derive Lattice's `lattice.kiln-budget-lineage.v1` from the admission ledger. The root MUST be the campaign root and the account MUST be the queue row's account, with that account's parent and depth. The remainder MUST be the per-dimension sum of the attempt's owner-committed reservations for the same work, attempt and fence, with dimension names unchanged so that Lattice lists every dimension it cannot charge as unaccounted. Every dimension the account tracks MUST have a positive remainder, and each amount MUST fit the work budget and the root allocation. The lineage MUST be emitted only when the attempt holds a committed reservation; without one, the request MUST keep its bytes and identities. When the lineage cannot be derived consistently, the dispatch MUST be refused with a typed reason before the owner-grant gate and any transport start, and a guessed lineage MUST NOT be sent.

#### Scenario: The worker route carries the lineage

r[kiln.admission.lattice_lineage.scenario.carried]
- GIVEN an attempt whose host-owner or nested-account reservation is committed
- WHEN the `--worker-request` route builds its request
- THEN the request MUST be the admitted bytes with one appended `lineage` member matching the committed golden bytes
- AND Lattice's published schema MUST accept that lineage, with the dimensions it cannot charge listed as unaccounted
- AND the frozen request MUST hold those bytes before the owner-grant gate, and recovery MUST reuse only a frozen request that is the admitted bytes, or the admitted bytes plus one well-formed lineage

#### Scenario: The exchange route checks but never carries it

r[kiln.admission.lattice_lineage.scenario.exchange]
- GIVEN the same ledger and a `--dispatch-plan` workflow-exchange dispatch
- WHEN the route builds its request
- THEN it MUST derive the lineage with the same function and keep the `lattice.workflow-request` bytes unchanged when the lineage is consistent
- AND it MUST refuse with the same typed reason when the lineage is inconsistent

#### Scenario: Inconsistent lineage is refused before transport

r[kiln.admission.lattice_lineage.scenario.refused]
- GIVEN a tentative or unknown reservation, a reservation on another account, a queue row claimed by another attempt, a grant beyond the work budget, a tracked dimension the attempt does not hold, a revoked or expired lineage, an uppercase identity, or an admitted request that already names a lineage
- WHEN a dispatch route builds its request
- THEN it MUST return the matching typed `LineageRefusal` and start nothing
