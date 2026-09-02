# Cairn gate adapter

`kiln-adapter-cairn` evaluates one exact Kiln gate plan through Cairn's versioned machine and gate-interchange contracts.

<!-- r[impl kiln.cairn_gate.nonclaims] -->

## Pinned cohort

The adapter pins:

- Cairn source `ssh://git@github.com/OnixResearch/cairn.git`;
- Cairn revision `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f`;
- `cairn-gate-contract` under `MIT OR Apache-2.0`;
- `cairn-cli` under `AGPL-3.0-or-later`;
- request schema `cairn.gate-request.v1`;
- result schema `cairn.gate-result.v1`;
- machine schema `cairn.cli-machine.v1`;
- diagnostic schema `cairn.cli-machine-diagnostic.v1`;
- policy schema `cairn.policy-schema.v1`;
- native receipt cohort `cairn.gate-receipt.v1`;
- executable BLAKE3 `9ade7248f9f48a0a83fc21365cc45550b09a048621ac925292ed974dbc4951d0`;
- Kiln policy BLAKE3 `be930a118f405d176ea56de5c72ba05b8061ab3a4f996113cde505710cce5c74`.

The reviewed x86-64 Linux executable is:

```text
/nix/store/bvz6kl88qkfi15n8ml1xr7p22syykggd-cairn-0.1.0/bin/cairn
```

The version-one profile admits only this immutable Nix store path. The adapter hashes the complete bounded file before construction and before each evaluation. A different path or byte identity requires a new reviewed profile.

## Process mechanism

The process shell uses Bounded Exec from the canonical Radicle publication:

- source: `https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git`;
- revision: `29dac88ecded94457572db3fdfaaaab95fa91525`;
- imported scope: the `bounded-exec` process shell and its `bounded-exec-core` dependency;
- public license: `AGPL-3.0-or-later`;
- reuse basis: OnixResearch owner-authorized source reuse for this Kiln integration.

This owner grant does not change the public license. It does not apply to third-party material or other recipients.

`cairn-gate-contract` is a linked permissive library, while `cairn-cli` is an external executable reached through the machine protocol. Bounded Exec is different: `kiln-adapter-cairn` links its Rust library and `bounded-exec-core`; it is not just invoking an independent AGPL command. The adapter's own `MIT OR Apache-2.0` declaration does not change those dependencies' public terms or make a combined output permissive-only. Public recipients must use the public licenses and satisfy the applicable copyleft obligations; they cannot rely on the owner-specific grant. See the repository [license map](../LICENSES.md) and included [AGPL version 3 text](../LICENSE-AGPL-3.0).

Bounded Exec owns explicit argv, clear environment setup, deadlines, cancellation, bounded output capture, process-group termination, and owned teardown. Kiln retains executable authorization, output parsing, evidence meaning, and retry policy.

## Request planning

The pure Kiln core admits a `GateEffectPlan`. It binds:

- effect and idempotency identities;
- candidate and candidate-root identities;
- ordered changes and frozen source facts;
- one pipeline revision;
- the exact policy identity;
- sorted unique evidence identities;
- the selected provider profile;
- the Cairn change and stage.

The adapter lowers these facts into `cairn.gate-request.v1`. Tagged BLAKE3 identities map Kiln labels into the producer contract without replacing canonical Kiln identities.

## Execution

The shell runs:

```text
cairn --machine gate <stage> <change> --root <absolute-root> --policy <absolute-policy>
```

The profile supplies every process and collection bound. The shell clears the ambient environment. It does not parse display text.

The adapter checks the process status, machine schema, exit class, command, diagnostics, machine non-claims, native policy identity, native receipt shape, and gate-interchange result identity.

## Outcomes and reconciliation

Kiln keeps these outcomes distinct:

- passed;
- blocked;
- policy error;
- provider error;
- timed out;
- cancelled;
- unknown.

Only passed and blocked outcomes contain an admitted result and native receipt identity.

Version one uses `rerun_policy = "never"`. A repeated effect returns its recorded exact observation. An unknown outcome remains unknown during reconciliation. The adapter never redispatches after an ambiguous execution.

A new candidate, policy, evidence set, profile, tool cohort, target, or effect requires a new request identity.

## Fixtures

Cairn-owned producer fixtures are copied from the pinned revision into:

```text
fixtures/cairn/gate-contract/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/
```

Run `scripts/sync-cairn-gate-fixtures.rs SOURCE_ROOT --check` against an exact source checkout.

Captured machine fixtures contain one passing and one blocked task-gate result. Independent Kiln tests admit both and reject candidate, policy, executable, schema, and receipt substitution.

## Authority boundary

Cairn owns lifecycle gate meaning. Kiln owns candidate identity, gate composition, CI state, process policy, persistence, and provider selection.

A passing Cairn result can satisfy one configured Kiln gate condition. It cannot mutate lifecycle files, merge source, publish an artifact, execute a release, or override another gate.

## Non-claims

Passing adapter checks do not prove:

- that candidate facts describe the evaluated filesystem;
- source correctness;
- test completeness;
- process reliability;
- sandboxing or hermeticity;
- merge or publication authority;
- release execution;
- deployment success;
- future applicability.
