# Mantle Foreign Realize Adapter

## ADDED Requirements

### Requirement: Pinned foreign-realize cohort and profile

r[kiln.mantle.foreign_realize.profile] Kiln MUST admit Mantle's
`foreign-import realize` operation only through a second typed profile bound to
one exact published revision, one exact clean-export executable identity, one
Bounded Exec revision, one logical store prefix and one platform, separate
from the build cohort. The profile MUST bind every frozen input that is not in
the build effect plan by BLAKE3 identity and keyed by plan identity: the plan,
import receipt, source bundle file and manifest, execution profile,
cache-closure policy and remote endpoint binding config. It MUST bind the
physical store, state directory, logical store prefix `/nix/store`, the named
limits, the required non-claims and one explicit product-name to selected-root
path map per plan.

#### Scenario: The pinned cohort is admitted

r[kiln.mantle.foreign_realize.profile.scenario.match]
- GIVEN a complete realize profile for the pinned clean-export revision and its bound inputs
- WHEN profile admission runs
- THEN Kiln MAY make the realize capability available

#### Scenario: Cohort or binding drift is refused

r[kiln.mantle.foreign_realize.profile.scenario.drift]
- GIVEN a profile with another revision, executable or builder identity, schema, command, platform, capability set, limit or non-claim set, or a binding with a malformed or non-absolute path
- WHEN profile admission runs
- THEN it MUST reject the profile before dispatch

### Requirement: Exact bounded realize dispatch

r[kiln.mantle.foreign_realize.dispatch] Before each realize dispatch Kiln MUST
verify the executable and every bound input identity. It MUST run exactly one
bounded `foreign-import realize` command with a cleared environment, one
Nix-free `PATH` directory, the bound physical store, state directory, logical
store prefix, endpoint binding config, plan, import receipt, source bundle and
manifest BLAKE3, execution profile, cache-closure policy, the exact attempt
receipt path and `--substitute`. It MUST NOT pass `--remote`, `--offline` or
any extra flag. A plan without an exact binding, another provider profile,
policy or platform, or an unmapped required product MUST be refused before
spawn.

#### Scenario: One exact command is dispatched

r[kiln.mantle.foreign_realize.dispatch.scenario.command]
- GIVEN one admitted plan with its exact realize binding
- WHEN dispatch runs
- THEN it MUST spawn the pinned executable with only the reviewed arguments and a cleared environment

#### Scenario: A changed input identity stops dispatch

r[kiln.mantle.foreign_realize.dispatch.scenario.identity]
- GIVEN an executable, plan, import receipt, source bundle, execution profile, policy or binding config whose observed BLAKE3 differs from the profile
- WHEN dispatch runs
- THEN it MUST refuse before spawning the provider

### Requirement: Closed-world realization completion

r[kiln.mantle.foreign_realize.completion] Kiln MUST admit only
`mantle-foreign-realization-receipt-v1`, closed-world over every top-level and
nested field the pinned producer serializes. It MUST recompute the producer's
`receipt_blake3` and derive its own receipt identity from the receipt bytes
with domain-separated BLAKE3. Completion MUST require `status = "complete"`,
`strongest_state = "realized"`, no `failure`, the bound plan, import-receipt,
source-bundle-manifest and cache-closure-policy identities, exactly one
selected root under the logical prefix, and every required product mapped
explicitly to that root. It MUST reject an unknown field, a missing mandatory
field, another schema, a mismatched identity, zero or several roots, a root
outside the prefix and an unmapped or missing required product.

#### Scenario: The pilot receipt is admitted

r[kiln.mantle.foreign_realize.completion.scenario.pilot]
- GIVEN the retained pilot receipt and its exact bound identities
- WHEN completion admission runs
- THEN it MUST admit one success observation with the pilot's selected root and product mapping

#### Scenario: Malformed or drifted evidence is refused

r[kiln.mantle.foreign_realize.completion.scenario.drift]
- GIVEN an unknown field, a missing field, another schema, a non-complete status, a failure envelope, a mismatched bound identity, zero or two roots, a root outside the prefix, or an unmapped required product
- WHEN completion admission runs
- THEN it MUST NOT record a completing observation

#### Scenario: Oversized output stays a bounded provider error

r[kiln.mantle.foreign_realize.completion.scenario.oversized]
- GIVEN a receipt longer than the named byte bound or malformed JSON
- WHEN completion admission runs
- THEN it MUST record a bounded provider-error observation, never success

### Requirement: Realize recovery and failure classes

r[kiln.mantle.foreign_realize.recovery] A repeated dispatch MUST return the
recorded observation without another realization. Reconciliation MUST return
only the recorded observation and MUST refuse a changed plan or binding;
Kiln MUST never re-realize during reconciliation. Timeout, cancellation,
provider error and unknown MUST remain distinct outcomes. A restored unknown
observation MUST stay unknown and a terminal outcome MUST carry one bounded
receipt identity.

#### Scenario: Replay and reconciliation do not realize again

r[kiln.mantle.foreign_realize.recovery.scenario.replay]
- GIVEN one recorded realization
- WHEN the same plan is dispatched again or reconciled
- THEN the recorded observation MUST be returned and no provider process MUST start

#### Scenario: Uncertain and cancelled outcomes stay distinct

r[kiln.mantle.foreign_realize.recovery.scenario.classes]
- GIVEN a timed-out, cancelled, unknown or malformed provider result
- WHEN the outcome is recorded
- THEN it MUST keep its class and MUST NOT become success

### Requirement: Realize adapter non-claims

r[kiln.mantle.foreign_realize.nonclaims] A passing realize admission MUST NOT
prove realization correctness, cache truth, signature or key authority beyond
Mantle's checks, product semantics, or release readiness. Mantle MUST own
realization and cache behavior.

#### Scenario: Realization is not release authority

r[kiln.mantle.foreign_realize.nonclaims.scenario.release]
- GIVEN an admitted complete realization receipt
- WHEN Kiln records the observation
- THEN no merge, publication, release or provider effect MAY follow from it alone
