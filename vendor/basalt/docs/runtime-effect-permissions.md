# Runtime effect permissions

Basalt evaluates portable runtime-effect permission facts. It does not execute effects or provide a sandbox.

## Policy flow

```text
consumer native request
  -> consumer-owned parsing and normalization
  -> typed Basalt scope, operation, and subject
  -> reviewed Nickel-exported policy
  -> deterministic exact or prefix rule selection
  -> explicit contract and verified-UCAN facts
  -> metadata-only allow or deny receipt
  -> consumer currentness recheck and native effect gate
```

The pure core reads no file, network, process, environment, binding, clock, or approval state.

## Closed scopes

The policy supports these scopes:

- `filesystem`
- `network`
- `process`
- `environment`
- `binding`

Each scope has a closed operation vocabulary. Unknown scope and operation values fail during DTO admission.

Subjects use explicit namespaces:

- `cap://` for consumer-admitted logical filesystem subjects
- `net://` for consumer-parsed network endpoints
- `blake3:` for reviewed executable identities
- `env://` for consumer-selected environment names
- `binding://` for named host bindings

A `cap://` prefix is a policy fact. It does not replace Cap Root, capability-relative I/O, or no-follow filesystem operations.

## Decision rules

Every profile declares `deny` as its default.

Rules declare an exact or prefix match, priority, scope, operation, subject pattern, and decision. Equal-priority conflicting matches deny as ambiguous.

Human approval is a separate input. Approval cannot override a policy denial or missing authority.

The default profile requires both an admitted Basalt contract decision and an opaque verified UCAN input.

## agentOS reference

The bounded comparison source is [`rivet-dev/agentos`](https://github.com/rivet-dev/agentos) revision `44a6c8022f26ef0edd56e3d224f8296ee6387232`.

Reviewed blobs:

- VM permission types: `e38172cfa97b21922fafcb822d254929bf1c191f`
- permission evaluation: `1ab663a8a2f8bab31218f39c49193bd63d31f82e`
- default-deny guards: `0fd804e703fe38d50efd353514ebea16f36452ac`
- Apache-2.0 license: `f23ef9d48af638ddf215b378916767349a9591a8`

Basalt reuses no agentOS schema, glob behavior, virtual kernel, V8 runtime, or sandbox claim.

## Consumer fixtures

`contracts/runtime-effect/consumer-mappings.ncl` records frozen, dependency-free fixtures for:

- Animus revision `4f08f89d652a6e6c83b2a41fd9d3c7651daf5d68`
- Molten revision `4734913c1c230a35fc121b948cdf54ac0619dd2a`

The fixtures test mapping shape only. They do not prove complete native mappings or consumer enforcement.

## Receipts

Receipts bind policy, request, selected rule, authority facts, decision, diagnostic, approval, and reference identities with BLAKE3.

They omit credentials, environment values, bearer tokens, raw host paths, and effect payloads.

A passing receipt is not live authority and does not prove effect execution.
