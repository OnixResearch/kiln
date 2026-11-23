# Independent Review Evidence (`wave2-base` @ `6a2fcda7`)

- **Reviewer:** `@kiln-reviewer` (independent read-only swarm reviewer, board `#27`, `2026-11-23T18:55Z`)
- **Reviewed range:** `83e064934244..6a2fcda7ece8` (`origin/wave2-base`: `fb32858e` + `6a2fcda7`)
- **Verdict:** **APPROVE** (0 P0, 0 P1, 0 P2; 3 non-blocking P3 notes)

## Prior blocking findings — verified resolved

1. **Optional supplemental loopback bind:** All four Iroh 1.1.0 builders (`crates/kiln-adapter-iroh/src/transport.rs`, `crates/kiln-adapter-iroh/src/migration.rs`, `crates/kiln-adapter-iroh-observe/src/shell.rs`, `crates/kiln-hosted/src/transport.rs`) call `.clear_ip_transports()`, bind the configured address as required (`is_required = true`, `/0` default route), and bind the opposite-family loopback via `.bind_addr_with_opts(..., BindOpts::default().set_is_required(false).set_is_default_route(false).set_prefix_len(128|32))`.
2. **`MigrationChannel` endpoint/stream provenance:** `crates/kiln-adapter-iroh/src/migration.rs` seals `(connection, send, receive, local)` inside non-`Clone` `MigrationChannel` with private fields, constructible only via `DirectMigrationEndpoint::connect`/`accept` (`presets::Minimal` + `RelayMode::Disabled`) and consumed by value in `bridge()` before `reservation.consume(plan)`.
3. **Stale pin note scoping:** `docs/reference-pins.md` explicitly scopes the historical 2026-10-23 audit/maintenance text to the `0.95.1` cohort.

## Cohort integrity

- Basalt `fac0d7f1` + UCAN `fca62a6e` source records, importer pins, `crates/kiln-adapter-basalt/tests/cohort.rs`, `LICENSES.md`, and `CHANGELOG.md` agree.
- `Cargo.lock` matches `evidence/lock-delta.txt` (119/119 entries; single `iroh 1.1.0`, no `aead 0.6.0-rc.2`).
- All 6 BLAKE3 artifact/lock hashes in `evidence/verification.json` match `6a2fcda7` byte-for-byte.

## Non-blocking P3 follow-up notes

- `DirectMigrationEndpoint::accept()`/`connect()` per-handshake/stream timeout and stream open sequencing when Aspen #15 integrates callers.
- Tightening `DirectMigrationEndpoint::endpoint(&self)` to `id()`/`addr()`/`close()` once callers exist.
- Re-running `cairn validate`/gates and `octet` on `6a2fcda7` at landing onto `main`.
