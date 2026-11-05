# Radicle workflow shell

Kiln can accept one bounded Radicle CI broker trigger through an explicitly selected Lattice or Aspen runtime path.

## Published cohorts

The shell pins these inputs:

- Radicle runtime: `radicle-0.24.0`
- Radicle CI broker: `a688afde1f730ef60402a809d9cdadc55e8dd749`
- Lattice MIT contract and transport import: `70496e67c7fd4a8b05914161a8e09de2759bebc8`
- Lattice CI runtime handler and source binding: `85a38b6a4bceeae4bb2057cf157e21752100f1b5`
- Lattice request schema: `lattice.workflow-request.v2`
- Aspen native-host cohort: `22f8ded26ca1907c29948e08b53f35df23080733`

The runtime pin includes `shell_success` from
`6339864d3c4e5de72ba4f0899b11b5d5b76704b6` and observed cancellation from
`813076d7e42d0ddfb65042bee615bc5f12a2b4d3`. The MIT import pin is not the CI
handler. Keep the vendored contract and transport at their reviewed MIT revision;
selecting the newer external runtime does not authorize importing AGPL sources.

The executable contains no Nix executor and no workflow fallback. Runtime selection is mandatory and never falls back.

## Profile

`config/radicle-workflow-shell.ncl` defines the typed production profile. This profile enables only trigger intake.

Source, status, and collaboration capabilities fail closed in this profile. They use separate profiles and authority boundaries.

Export the runtime JSON:

```console
nix run nixpkgs#nickel -- export --format json \
  config/fixtures/radicle-workflow-positive.ncl \
  > /etc/kiln/radicle-workflow.json
```

The profile binds these values:

- one Lattice Unix socket and peer policy;
- finite request, response, connection, poll, and replay bounds;
- one exact workflow graph and operation identity;
- one durable SQLite replay database for the direct Lattice path;
- exact upstream revisions and licenses; and
- explicit non-claims.

## Lattice route

The matching Lattice handler route must select `shell_environment`. It injects the admitted source facts into exact mapped shell nodes:

- `LATTICE_SOURCE_PROVIDER`
- `LATTICE_SOURCE_REPOSITORY`
- `LATTICE_SOURCE_REVISION`

The Lattice route must bind the same profile identity, graph identity, operation identity, saved workflow revision, and node mapping.

## Broker command

Select the direct Lattice runtime explicitly:

```console
kiln-adapter-radicle \
  --profile /etc/kiln/radicle-workflow.json \
  --runtime lattice
```

Select Aspen only with its exact exported v2 profile and deployed Unix endpoint:

```console
kiln-adapter-radicle \
  --profile /etc/kiln/radicle-workflow.json \
  --runtime aspen \
  --aspen-profile /etc/kiln/aspen-runtime.json \
  --aspen-socket /run/kiln/aspen.sock
```

If the selected Aspen socket or any exact profile fact is unavailable, the command fails. It does not dispatch directly through Lattice.

Use the original line-JSON compatibility protocol only when the broker configuration selects it explicitly:

```console
kiln-adapter-radicle \
  --profile /etc/kiln/radicle-workflow.json \
  --runtime lattice \
  --protocol native
```

Diagnostics go to standard error. Standard output contains only protocol response lines.

### Broker 0.31 envelope

The default `defelo` protocol requires broker request kind `trigger` and version `1`. Kiln projects only event kind, repository ID, first push or patch commit, branch, and an exact patch ID when the event is a patch. A patch event without an ID fails closed. Repository description, visibility, delegates, pusher, `before`, and `after` remain producer-owned metadata and do not gain CI authority.

`fixtures/radicle-ci-broker/0.31.0/trigger.json` is the published producer fixture. Malformed envelopes, wrong versions, unknown events, and missing commits fail before runtime dispatch.

## Durable replay and recovery

On the direct Lattice path, Kiln reserves the deterministic trigger identity in SQLite before dispatch.

On the Aspen path, the native extension owns the same replay decision over Aspen-hosted state. The broker does not also reserve or dispatch through SQLite.

An exact duplicate never sends a new dispatch. It sends only `RecoverDispatch` for an unresolved request. If Lattice cannot find an acceptance, Kiln preserves an unknown outcome.

A changed request cannot reuse the same event identity. Unknown and timed-out outcomes remain recoverable and never become success without a linked Lattice observation.

## Capability separation

The trigger profile rejects source acquisition, status publication, and collaboration mutation.

Use `config/radicle-source-profile.ncl` for exact local source reads. Use the dedicated status and patch-review profiles for their declared operations.

No profile inherits another profile's repository, signer, workflow, or mutation authority.

## Explicit revision-bound patch CI

The default push profile and its canonical trigger/journal bytes remain unchanged.
Patch CI is selected separately with `kiln-radicle-ci patch PATCH_PROFILE.json`;
it consumes the existing broker patch envelope, including its exact patch ID.
PatchCreated/PatchUpdated notifications normalized by the broker use this same
route, not a second compatibility protocol. The controller resolves the event
oid through the pinned native patch reducer; broker author/delegate metadata
does not establish trust. See `config/radicle-patch-ci.ncl` for the typed profile.

Each private revision journal retains RID, patch ID, revision entry ID, exact
base, exact oid, native revision author, observed heads, and a normalized trigger
with `before = base`, `after = oid`, `patch = patch ID`, and `event = patch`.
The Kiln run identity additionally binds the native revision ID and reviewed
profile. Distinct revisions are separately journaled even if their oids coincide.
Historical delivery at an already built oid resolves its existing revision;
ambiguous same-oid deliveries resolve the current native revision, never inherit
an earlier revision's approval or result.

The helper verifies the author's current signed refs and exact namespace target.
A signed branch or patch COB tip must reach the revision oid. Native patch COB
commits retain revision base/head as related Git parents. Merely finding an
unsigned Git ref or receiving an arbitrary broker oid is not sufficient.

`trusted_authors` is an explicit operator-selected set of delegate/followed node
keys, not an automatically trusted producer list. Other authors are durably
`blocked` before source execution or resource reservation. Approval is an
interactive, local operator operation; keep the private profile/state inaccessible
to broker producers, workload users and automation principals:

```text
kiln-radicle-ci patch-intake PATCH_PROFILE.json TRIGGER.json
kiln-radicle-ci patch-approve PATCH_PROFILE.json APPROVAL.json
kiln-radicle-ci patch-drive PATCH_PROFILE.json RUN_ID
kiln-radicle-ci patch-replay PATCH_PROFILE.json RUN_ID
```

The approval uses the existing `ApproveUntrustedRun` JSON contract: operator
`actor` (id, label, authority `operator`), exact `run`, and `source_commit`.
The operator id must be in the profile's approver set. The run binds the complete
patch/revision/base/oid tuple; a moved current revision refuses approval or first
dispatch and needs a new run/approval. Repeated events load the original journal.

### Existing isolated execution port, not a direct Nix fallback

Patch dispatch reuses the authenticated `kiln-hosted rpc` `execution.run` port.
Configure the pinned hosted executable, exact private client-profile bytes,
organization/project, reviewed pipeline identity, approved price/class, and the
existing canonical private `work_root/organization/project` directory shared
with that service. The source adapter materializes only the verified oid there
using its bounded Git snapshot collector; base and oid are not a merge result.
The pipeline must already be reviewed by an authorized hosted project principal.
The controller cannot review arbitrary commands, bypass account/financial
admission, or manufacture an execution backend.

An approved untrusted request carries immutable `UntrustedProvenance`, `trust =
hostile`, exact source oid and approving operator. The hosted port enforces the
existing #12 policy: only explicitly untrusted-capable Firecracker runners,
admitted host/kernel facts and independent qualification, no secrets, no workload
token and no shared CI cache. Provenance remains in the durable original request
on every recovery. No native Nix/namespace escape route or confinement override
is introduced. A patched and rebooted host is an external prerequisite for
untrusted runtime verification on a vulnerable development kernel.

Source and the exact request are durable before RPC. Lost/ambiguous RPC outcomes
stay `unknown`; later drives use fresh bounded `execution.get` requests for the
original attempt only. They never speculatively submit a replacement. Successful
or failed terminal observations produce an exact-oid `xyz.radworks.job` result
through the existing job shell, retaining the Kiln revision-specific run ID.
Job publication is local signing under its separately declared SecretSpec profile,
not remote synchronization. A later revision cannot inherit that result, and
patch runs never satisfy push/default-branch CI requirements.

The broker command emits the ordinary triggered/finished response shapes. If a
run is blocked, running or uncertain, it emits no invented terminal success;
approve/reconcile the retained original run ID. Operator commands expose the
bounded journal directly. The old partial patch-helper fields on the push worker
profile have been removed rather than retained as a second dispatch route.

### Local regression procedure and evidence boundary

```console
cargo test --locked --offline -p kiln-radicle-ci
cargo test --locked --offline -p kiln-adapter-radicle --all-features
cargo +stable test --locked --offline --manifest-path tools/radicle-patch/Cargo.toml -- --nocapture
```

`patch_tests` exercises two real exact Git snapshots with a fixture native
projection/transport, separate revision journals, blocked execution, unauthorized
approval rejection, revision reapproval, signed-author reachability rejection,
stale approval rejection, and duplicate updates recovering instead of dispatching.
The helper's separate real native fixture proves concurrent delegate reduction,
crefs merge denial, two-head review append, stale signing refusal and invalid
signature rejection. Existing push replay fixtures exercise unchanged default
profile behavior. Fixture transport results are not real VM qualification;
integrated verification must report which commands actually ran and must not
claim an untrusted Firecracker pass on a vulnerable/unadmitted host.

For disposable local job-signing proof only, build `kiln-radicle-ci` with its
existing `native-fixtures` feature and place `--native-fixtures` before the patch
command. This enables the existing fixture SecretSpec/job-shell admission in
process memory; profile JSON cannot enable it. The terminal fixture test writes
and independently reads real local job COBs for both exact oids. It does not
relax hosted runner, authentication, host/kernel or untrusted-grant policy.

## Non-claims

Passing local checks does not prove event completeness, source trust, revision availability, checkout correctness, workflow correctness, provider availability, distributed exactly-once delivery, external effect success, or release readiness.
