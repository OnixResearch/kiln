# Aspen Host Bridge Production Specification Delta

## ADDED Requirements

### Requirement: Durable hosted-operation state

r[kiln.aspen_host.production_durability] The deployable host MUST persist each admitted ingress, exact trigger, Aspen native journal record, callback value, provider certainty state, and terminal acknowledgement before that fact is required after process loss.

#### Scenario: Terminal operation survives process loss

r[kiln.aspen_host.production_durability.accepted]
- GIVEN a hosted operation reached one admitted terminal acknowledgement
- WHEN the host process stops and reopens the same state root
- THEN a repeated submit or reconcile MUST return the exact acknowledgement
- AND it MUST NOT dispatch another provider effect

#### Scenario: Durable state is missing or changed

r[kiln.aspen_host.production_durability.rejected]
- GIVEN a record, callback value, journal entry, profile identity, generation, or content identity is missing, truncated, malformed, or substituted
- WHEN host recovery opens the state root
- THEN startup or the affected operation MUST fail closed
- AND the host MUST NOT infer a terminal result or safe retry

### Requirement: Exact recovery without redispatch

r[kiln.aspen_host.production_recovery] The host MUST classify a process-lost pending or uncertain provider operation from durable facts and MUST use the exact Lattice recovery or reconciliation operation before any terminal transition.

#### Scenario: Accepted operation completes after restart

r[kiln.aspen_host.production_recovery.accepted]
- GIVEN the provider accepted an exact operation before the host lost its process
- WHEN the restored host receives the same submit or reconcile request
- THEN it MUST use the original request, effect, provider, and idempotency identities
- AND it MUST recover or reconcile without calling provider dispatch again

#### Scenario: Provider acceptance cannot be established

r[kiln.aspen_host.production_recovery.unknown]
- GIVEN the durable ledger proves intent but Lattice cannot prove rejection or terminal completion
- WHEN recovery reaches its finite bound
- THEN Kiln MUST retain `Unknown`
- AND it MUST NOT retry, select another provider, or erase the uncertain record

### Requirement: Bounded hosted Radicle Nix provider

r[kiln.radicle.hosted_nix_provider] Kiln MUST provide one shell adapter that admits an exact typed profile and explicit Radicle source facts, runs only the fixed Nix wrapper through Bounded Exec, and publishes bounded deterministic reports.

#### Scenario: Exact revision completes

r[kiln.radicle.hosted_nix_provider.accepted]
- GIVEN the admitted user, repository, revision, source root, wrapper identity, report root, and positive limits
- WHEN Lattice starts the provider for one source-bound workflow node
- THEN the provider MUST execute only the admitted argv with a cleared child environment
- AND it MUST publish a bounded log and JSON report bound to the repository, revision, command, outcome, and BLAKE3 run identity

#### Scenario: Provider input or execution escapes policy

r[kiln.radicle.hosted_nix_provider.rejected]
- GIVEN a wrong user, repository, revision shape, relative or escaping path, changed command, inherited environment, timeout, cancellation, output flood, or report conflict
- WHEN provider admission or execution runs
- THEN it MUST fail without false success
- AND every owned child process MUST be terminated within the named teardown bound

### Requirement: Production-host evidence boundary

r[kiln.aspen_host.production_evidence] Production eligibility evidence MUST cover real-process durability, recovery, corruption, provider teardown, finite load, backup/restore, explicit rollback, authority, and exact publication identities.

#### Scenario: Production evidence is accepted

r[kiln.aspen_host.production_evidence.accepted]
- GIVEN every focused and broad gate passes for the immutable cohort
- WHEN the release receipt is evaluated
- THEN it MUST link the exact source, executable, profile, host, provider, restart, corruption, load, backup, and rollback evidence
- AND it MAY claim only the bounded local production-host behavior that was observed

#### Scenario: Local durability is promoted into a global claim

r[kiln.aspen_host.production_evidence.rejected]
- GIVEN only machine-local journal, ledger, callback value, workflow, and report observations
- WHEN evidence claims global durability, exactly-once delivery, CI correctness, external provider truth, host sandboxing, or release eligibility by implication
- THEN validation MUST reject the stronger claim
