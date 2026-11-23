## Context

| Input | Before | After | Consumer |
|---|---|---|---|
| `vendor/basalt` | `264d248912e58760b5ae1eced51b7c7d50eb5a96` | `fac0d7f1fd7ac936ca4d3f251f3287666e93d4bb` | `kiln-adapter-basalt` (implementation authority) |
| `vendor/ucan` | `417f74662b9de2de3bdabe93ad5db996e1bd2861` | `fca62a6ed6895689ff18112b497bc7a986516c20` | Basalt, HTTP observation UCAN auth |
| Registry `iroh` | `=0.95.1` | `=1.1.0` | `kiln-adapter-iroh`, `kiln-adapter-iroh-observe`, `kiln-hosted` |

Lattice `main` `39936a19` vendors Basalt `fac0d7f1` in
`vendor-cargo-deps/basalt`, and that tree's `vendor/ucan-revision.txt` names
`fca62a6e`. Basalt's nested 697-file UCAN snapshot is byte-identical to the
corresponding files of Kiln's UCAN import, so Kiln and Lattice compile the same
UCAN code.

Archive identities (`b3sum` of `git archive --format=tar`):

| Import | Full archive | Selected archive (recorded `archive_paths`) |
|---|---|---|
| Basalt `fac0d7f1` | `68ce460383046f3b985772dce0ea20dfcc49f95a1c4e5c67d1d37d4dd4692b48` | `8f5d3c4b89d7309c74c49707669e3ad163c0bce606b919df1eaf733e8bd47ecb` |
| UCAN `fca62a6e` | `4a518b3900a9a18d97174f4fd9c675e3509333bef78b92110d5ea7e824aace49` | `55c68cddbaa7dc62bbb9c65ba254917911bda54ce82436534423d1a9f85b6722` |

## Decisions

### Decision: Import through the existing importer

**Choice:** Pin both revisions and selected-archive digests in
`scripts/import-pinned-source.sh` and the source records, delete the old trees
and run the importer for `ucan` and then `basalt`; a second run verifies each
existing tree byte for byte. The archive paths and Basalt's three literal
`vendor/ucan` to `../ucan` manifest rewrites are unchanged and still match
exactly one occurrence each.

**Rationale:** The importer is the only repository-owned transform; it refuses
digest, record or tree drift and has no force mode. Against the extracted
archives the vendored trees differ only by the documented normalizations: the
three Basalt manifest rewrites and the root `rustfmt.toml` in each tree. UCAN's
workspace members are all present, so no member is pruned.

### Decision: Adapt the Basalt API without adopting v2

**Choice:** Basalt's `Request` gains `framing` and `Verifier` gains
`max_delegation_depth` and `implementation_v1`. `kiln-adapter-basalt` sets them
from three constants: `IMPLEMENTATION_FRAMING = ImplementationFraming::V1`,
`IMPLEMENTATION_V1 = LegacyFraming::Accept` and `MAX_DELEGATION_DEPTH = 0`. The
production verifier, the revalidation verifier, the holder signer and the
adapter's unit tests use them.

**Rationale:** V1 and `Accept` are Basalt's defaults and keep every resource
Kiln binds byte-identical. Basalt consults the depth bound only for attenuable
families, so `0` affects no v1 decision; it is the most restrictive value if an
attenuable request ever reached this verifier. Choosing framing, delegate rule
and depth is Wave 2's work.

Basalt itself tightened one v1 case: a v1 chain whose covering grant is wider
than `kiln://implementation/v1/` now needs `kiln/delegate` at every delegator.
Kiln's fixtures issue v1-scoped grants, and its authority tests pass unchanged.

### Decision: Move Iroh to `=1.1.0`

**Choice:** Pin `iroh = { version = "=1.1.0", default-features = false,
features = ["portmapper", "tls-ring"] }` in the three crates.

**Rationale:**

- `0.95.1` cannot share a lock with `k256` 0.14 (stable `crypto-common` 0.2
  against `aead` 0.6.0-rc.2's `crypto-common` release candidate with
  `os_rng`).
- `0.96.0` resolves but fails to compile: its `ed25519` 3.0.0-rc.1 cannot
  convert errors into stable `pkcs8` 0.11.0. `0.96.1` to `0.98.0` fail to
  resolve on `sha2`.
- `1.1.0` resolves, compiles, and is the line `runtime/kiln-aspen-host`
  already pins, so Kiln does not bump twice.
- 0.95.1 always used ring for TLS and always ran the port mapper. The two
  features keep both; `metrics` stays off as before.

Endpoint construction mapping, applied identically in all four builders
(workflow transport, observation shell, hosted transport, migration bridge):

| 0.95.1 | 1.1.0 |
|---|---|
| `Endpoint::builder()` then `.clear_discovery()` | `Endpoint::builder(presets::Minimal)`: ring crypto provider only, no address lookup, relays disabled until `relay_mode` |
| `TransportConfig::default()` and in-place setters | `QuicTransportConfig::builder()`, the same setters and values, then `.build()` |
| `.bind_addr_v4(a).bind_addr_v6(loopback)` (and the v6 mirror) | `.clear_ip_transports().bind_addr(a)?.bind_addr(loopback)?`; an invalid address maps to the existing bind error |
| `conn_type(id)` returning `ConnectionType::Direct(addr)` | `connection.paths()`: every open path must be `TransportAddr::Ip`, and the selected path's address is returned; any relay or custom path, or no selected path, denies |

ALPNs `kiln.workflow.v1`, `kiln.observe.v1` and `kiln/ch-migration/precopy/1`,
frame formats, stream and window bounds, idle timeouts, peer allowlists and
relay selection are unchanged. `EndpointAddr` keeps its `{id, addrs}` JSON shape
with `Ip` and `Relay` entries.

### Decision: Rebind the version-pinning profiles

**Choice:** `IROH_VERSION` in both adapters, the `Exact` cohort contracts in
`config/iroh-workflow.ncl` and `config/iroh-observation-profile.ncl`, their
fixtures and inputs, `config/live-ci-worker.ncl`, the two scripts and the
retained-host capability example name `1.1.0`.
`fixtures/iroh-observation/profile-v1.json` is regenerated with
`nickel export`. `iroh_blobs` and `bao_tree` stay `deferred`: Iroh 1.1.0 needs
neither, and the profile check still binds both. The negative cohort fixtures
(`0.95.0`, `0.96.0`) still differ from the pin.

**Rationale:** Admission compares the profile's `iroh_version` with the
compiled constant, so profiles must name the version the binary links; a
profile naming `0.95.1` is refused before any endpoint binds.

## Specification classification

`r[kiln.iroh.workflow.profile]` names Iroh 0.95.1, so the change carries one
MODIFIED delta that changes only that version to 1.1.0; the ALPN, admission
inputs, bounds and both scenarios are unchanged. No other accepted requirement
names a Basalt, UCAN or Iroh revision. The accepted
`kiln.implementation.authority.cohort` requirement already requires one
vendored UCAN snapshot and a recorded reproducible Basalt import; this change
keeps both without changing its text, so the Basalt tasks depend on it rather
than modify it.

## Failure behavior

- Importer drift (archive digest, record or existing bytes) stops the import
  without replacement.
- `cohort.rs` fails if either source record names another revision, if Basalt's
  compiled UCAN revision differs from the UCAN record, or if a recorded rewrite
  no longer matches the vendored manifest.
- A profile naming another Iroh version, ALPN or blob cohort is refused before
  an endpoint binds.
- An invalid bind address now fails the builder before `bind`, through the same
  bind error each caller already returned.
- The migration bridge refuses a connection with any non-IP path or no selected
  path, as it refused relay and mixed connections before.

## Risks / Trade-offs

- Iroh 1.x is a different QUIC stack (`noq`, multipath). Peers on 0.95.1 are not
  promised to interoperate, so every Kiln Iroh endpoint upgrades together.
- The lock delta is wider than the two source cohorts: the forced Iroh move
  replaces Iroh's QUIC, DNS, netwatch and port-mapper trees and stabilizes the
  signing crates. `evidence/lock-delta.txt` lists every entry; no unrelated
  package was updated by hand.
- The dated 2026-10-23 `cargo audit` record in `docs/reference-pins.md` is
  marked as describing the 0.95.1 cohort; no new audit is claimed.
