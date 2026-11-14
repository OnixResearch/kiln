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

### Independent implementation acceptance (#46)

Implementation acceptance uses `kiln_implementer::verification::verify_candidate`,
not a worker success manifest. Its consumer must load an operator-admitted
`kiln.verification-profile.v1` and `VerifierAuthority`, execute that profile,
retain the returned observation even when nonpassing, and only then admit the
candidate. `VerificationRun` is intentionally not deserializable. Replaying a
reserved handoff does not execute it again; `reservation.json` remains explicit
unknown evidence if the controller dies before retaining its observation.

`materialize_candidate(work, attempt, dispatch, profile, authority, candidate_root,
host_receipt_bytes)` bridges the real Lattice host snapshot to Git before
verification. It independently reads the exact Git baseline and retained worker
workspace, ignores only root `.git`/`.jj`, and recomputes native
`lattice.candidate-receipt.v1` BLAKE3 tree/diff framing and complete changed paths
(including deletions). Non-UTF8 paths, symlinks, hardlinks, special files,
unsupported mode changes, more than 50,000 entries or 512 MiB are refused. A
produced host receipt must match the retained dispatch/workspace/work/fence in
full. Its success claims never become acceptance evidence. The bridge writes a
real deterministic commit/tree through a private in-memory Git index, retaining
only `refs/kiln/implementation-candidates/<attempt>` in trusted acquisition. No
worker `.git`, hooks, filters, index, refs or commit claims are consulted. Exact
objects are fetched into fresh verification snapshots from that trusted local
acquisition; no remote or target branch is changed. Empty produced receipts
remain invalid. No-change acquisition requires the distinct honest host
`lattice.unchanged-candidate-observation.v1`, matching actual baseline/candidate
bytes, both explicit no-change policies and candidate equal to the real baseline
commit. It remains a proposal for independent verification, never native worker
success or an invented produced receipt.

The canonical service consumer is
`Engine::verify_implementation_candidate`, which reads current durable work,
attempt and handoff, executes the admitted profile, then calls
`Engine::record_implementation_verification`. The latter accepts only the actual
non-deserializable in-process `VerificationRun`, rechecks current fencing and
source enrollment, invokes the native Cairn evaluator where required, and
persists ordinary `RunRecord` canonical events, `WorkflowObservation` and retained
artifact/report bytes. Its result is derived through the existing
`code_review_ci` projection before implementation-ledger admission. Failed and
unknown observations remain visible; no shadow success table is used.

The reviewed project's workflow must exactly match the executed check names,
kinds and prerequisite edges. Pin each operation with
`Engine::implementation_verification_operation(profile, check_id)`; a Cairn
workflow additionally includes the reserved native-completion node depending on
all acceptance nodes. This prevents an acceptance-only subset from claiming full
unrelated project CI. Native `SourceResolution` stays unchanged. A serde-default
optional controller-created provenance record binds the exact acquisition path,
enrolled repository alias, handoff/work/version/fence/profile, actual observation
and independent receipt; only validated protected-verifier records receive that
alias in canonical CI. Missing historical provenance is not an invented mapping.
Ordinary runs are unchanged. Protected `kiln-independent-verifier-v1` records
cannot enter Lattice polling/recovery, automatic/manual retry or provider
controls; fresh attempts require fresh frozen verification through the controller.

For qualification configuration, the native constructor avoids reimplementing
profile or operation identities in another language:

```console
cargo run --locked -p kiln-web-service --example implementation_verification_profile -- \
  --template dedicated-service-template.json \
  --verification-profile frozen-verification-profile.json \
  --project verification-project --output canonical-service-profile.json
```

Supply an explicit dedicated ServiceProfile template whose selected workflow
`nodes` and `edges` are empty. The constructor refuses to replace any existing CI
graph, preserves all other projects and service/transport/source/bounds fields,
derives every frozen check and native Cairn dependency, and uses
`Engine::validate_implementation_verification_project` (the runtime all-check
relation) followed by native service-profile admission. It writes a new file
without overwriting an existing one. Its output is configuration only with
`checks_executed: false` and `effects_executed: false`, never accepted evidence.
No unrelated project CI may be relabeled as this dedicated verification graph.

The Linux shell checks its actual effective filesystem UID through `/proc/self`;
it never switches credentials. The enrolled acquisition repository and scratch
directory, including ancestors, must not be worker-owned or group/world writable.
Worker and verifier UIDs must differ. Programs, Cairn policy and registry inputs
are bounded, byte-hashed immutable `/nix/store` inputs, not executables selected
from the candidate. The environment is cleared and reconstructed from the frozen
profile. `derived_environment_identity()` and `derived_toolchain_identity()`
identify the actual environment map and hash-checked verifier program cohort.
This is an authority boundary, **not hostile-code sandbox qualification**; a live
operator must additionally isolate candidate execution and protect verifier
credentials and retained evidence.

The shell independently resolves exact Git baseline/candidate commits and tree,
the complete NUL-delimited non-renamed changed-path set, and protected source
objects. Missing paths, substituted commits/trees, unsupported submodules and
weakened protected definitions fail closed. Each check gets a fresh private
snapshot. Required artifacts must be absent before execution, then present as
nonempty regular files without symlink components. Controller-retained bytes,
stdout/stderr and execution receipts are re-read before admission; a worker's
proposed artifact identity cannot fill a missing result. Timeout, crash, skipped
prerequisites, output-limit and incomplete evidence never become passing checks.

Every acceptance obligation must map to a frozen executed case. A bug profile's
selected reproducer uses identical admitted program/argv/environment on baseline
and candidate: ordinary repair needs a known nonzero baseline exit and a passing
candidate. An empty patch additionally requires
`VerifiedOrAlreadySatisfied`, profile authorization, candidate equal to baseline,
and independently passing cases (including the same reproducer on both snapshots
when present). Empty source, task checkboxes and process exit zero alone are not
already-satisfied evidence.

For Cairn work, call
`kiln_adapter_cairn::acceptance::verify_cairn_completion` with the actual returned
`VerificationRun`. It converts retained execution observations to Cairn's
`AgentExecutionResult` and calls the pinned producer's
`evaluate_authoritative_agent_completion`; there is no second completion checker.
The admitted contract binds current immutable registry/policy JSON, exact
required argv/prerequisites, selection, and the independently derived final
subject. The outer handoff additionally binds work/version, full change/task
aliases and source/spec versions, attempt/fence, repository, base and candidate
tree. Neither native report nor typed check can transfer to another envelope.
The retained producer report is `cairn-completion.json`; its bare BLAKE3 receipt
identity becomes the canonical `CiCheck` named
`cairn-authoritative-completion`, kind `cairn-agent-completion-v2`. Other checks
remain canonical `CiFact`/`CiCheck`, consumed through existing attempt admission
and review/integration queues. A moved target still needs a fresh queue candidate
and applicable CI; this adapter grants no landing authority.

Cairn lifecycle source defaults to exact baseline `.cairn` tree protection.
Source-changing sync/archive may be staged **before review** only when the trusted
controller has independently prepared and frozen its exact `.cairn` Git tree in
`CairnAcceptance.prepared_lifecycle_tree` in the admitted profile. Other explicit
protected paths remain unchanged; acceptance, policy, registry and check selection
remain frozen outside the worker. This field authorizes exact reviewed source,
not proof that a lifecycle command ran. A worker deletion/archive or arbitrary
task edit has no such authority. No post-review source sync/archive is performed.
Without independently prepared exact source, that lifecycle mode is unsupported.

The authoritative evaluator is from the same pinned Cairn source revision above;
`cairn-core` is a linked AGPL producer dependency, not just the permissive gate
interchange crate. A CLI response with `checks_executed: false` remains validation
only and is never execution evidence. The operator must supply the frozen
verifier programs/material, private separate-UID execution service and enrolled
repository before the real bug/feature pilot. No pilot, test execution, remote
effect, lifecycle execution or hostile-isolation qualification is claimed here.

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
