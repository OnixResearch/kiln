# Radicle workflow shell

Kiln can accept one bounded Radicle CI broker trigger and dispatch one exact source-bound workflow through Lattice.

## Published cohorts

The shell pins these inputs:

- Radicle runtime: `radicle-0.24.0`
- Radicle CI broker: `a688afde1f730ef60402a809d9cdadc55e8dd749`
- Lattice contract, transport, handler, and source binding: `70496e67c7fd4a8b05914161a8e09de2759bebc8`
- Lattice request schema: `lattice.workflow-request.v2`

The executable contains no Nix executor and no workflow fallback.

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
- one durable SQLite replay database;
- exact upstream revisions and licenses; and
- explicit non-claims.

## Lattice route

The matching Lattice handler route must select `shell_environment`. It injects the admitted source facts into exact mapped shell nodes:

- `LATTICE_SOURCE_PROVIDER`
- `LATTICE_SOURCE_REPOSITORY`
- `LATTICE_SOURCE_REVISION`

The Lattice route must bind the same profile identity, graph identity, operation identity, saved workflow revision, and node mapping.

## Broker command

Use the deployed broker protocol by default:

```console
kiln-adapter-radicle --profile /etc/kiln/radicle-workflow.json
```

Use the original line-JSON compatibility protocol only when the broker configuration selects it explicitly:

```console
kiln-adapter-radicle \
  --profile /etc/kiln/radicle-workflow.json \
  --protocol native
```

Diagnostics go to standard error. Standard output contains only protocol response lines.

## Durable replay and recovery

Kiln reserves the deterministic trigger identity in SQLite before dispatch.

An exact duplicate never sends a new dispatch. It sends only `RecoverDispatch` for an unresolved request. If Lattice cannot find an acceptance, Kiln preserves an unknown outcome.

A changed request cannot reuse the same event identity. Unknown and timed-out outcomes remain recoverable and never become success without a linked Lattice observation.

## Capability separation

The trigger profile rejects source acquisition, status publication, and collaboration mutation.

Use `config/radicle-source-profile.ncl` for exact local source reads. Use the dedicated status and patch-review profiles for their declared operations.

No profile inherits another profile's repository, signer, workflow, or mutation authority.

## Non-claims

Passing local checks does not prove event completeness, source trust, revision availability, checkout correctness, workflow correctness, provider availability, distributed exactly-once delivery, external effect success, or release readiness.
