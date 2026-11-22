# Fair admission delta

## ADDED Requirements

### Requirement: Registered host-local owner

r[kiln.admission.host_owner] For one explicitly registered host-local owner, Kiln MUST derive the admission policy, finite root account, queue rows, usage and eligibility observations, per-claim reservation and owner reply from the reviewed registration through the existing ledger transitions and owner port, with no second queue, scheduler or capacity ledger. The owner MUST grant only its declared capacity against the reservations the ledger retains for it, MUST NOT invite more eligible work than its free capacity, and MUST NOT report remote, provider or serving capacity. Unregistered repositories or work classes, a crossed claim, a stale or other-generation observation, a second reservation for the same work and a request beyond capacity MUST be refused before any grant, and an interrupted tentative request MUST be resolved from its own record. Replay and restart MUST NOT duplicate a reservation or reset a budget. A reservation MUST be settled only after the holding attempt is observed terminal and resolved; one that provably never reached transport MUST return without consumption, and once transport may have started the worker MUST be observed stopped, so supersession or lease expiry alone MUST NOT free the slot.

#### Scenario: Sandbox reservation passes the dispatch gate

r[kiln.admission.host_owner.scenario.reserve]
- GIVEN a registration admitting `onixresearch/implementer-sandbox` and one current ready work in its registered class
- WHEN the operator admits, observes, claims and reserves through the host-owner commands
- THEN the owner MUST commit exactly one reservation for that claim
- AND the existing pre-transport gate MUST admit dispatch intent with the owner's fresh observation

#### Scenario: Refused requests make no grant

r[kiln.admission.host_owner.scenario.refused]
- GIVEN an unregistered repository, an observation older than its window, a host whose capacity another reservation holds, or a work that already holds a reservation
- WHEN a reservation is requested
- THEN it MUST be refused with an explicit reason
- AND no new reservation or debit MUST be retained

#### Scenario: Restart and replay keep one reservation and the budget

r[kiln.admission.host_owner.scenario.replay]
- GIVEN a committed host reservation and a store that is reopened
- WHEN admit, observe and reserve run again, or a larger quota is registered
- THEN the retained operation MUST be returned without a new request, the account and quota MUST be unchanged, and the larger quota MUST be refused
- AND release MUST be refused until the attempt is observed terminal, then return a never-dispatched slot without consumption exactly once
- AND a holder whose transport may have started MUST keep its slot until a termination is reconciled or a candidate or the worker observation from Lattice's complete native result is retained, while a refusal before any effect or a crash before the transport start MUST be released once the attempt is terminal
- AND while a claim holds a host-owner reservation, the operator `start` acknowledgement MUST be refused, so host-owner work starts only through a dispatch route that records the transport start before any spawn or delivery

### Requirement: Typed host-owner registration

r[kiln.admission.host_owner_profile] The host-owner registration MUST be authored through a typed Nickel contract whose committed generated export is checked for staleness and admitted again by Rust with the same capacity, quota, window and uniqueness bounds, and an invalid registration MUST fail to export.

#### Scenario: Registration round-trips

r[kiln.admission.host_owner_profile.scenario.exports]
- GIVEN the positive implementer-sandbox registration and negative capacity, window, repository, class, quota and empty fixtures
- WHEN the fixture check and Rust tests run
- THEN the positive MUST match its committed export and admit in Rust, and every negative MUST fail to export and be refused by Rust re-admission

### Requirement: Admission-governed operator claim

r[kiln.admission.operator_claim] When an explicit admission policy is retained, the operator claim MUST select and bind the queue row through the admission claim transition under an explicit authority, so the queue claim and attempt/fence agree. Without a policy the existing claim MUST be unchanged.

#### Scenario: Governed claim binds the queue row

r[kiln.admission.operator_claim.scenario.governed]
- GIVEN a configured policy and one queued, freshly observed work
- WHEN the operator claims without an authority and then with a local-operator authority
- THEN the first MUST be refused without a claim
- AND the second MUST return the attempt whose identity and fence the queue row's claim records
