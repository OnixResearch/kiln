## Phase 1: Pure Core Catalog & Fleet Aggregation Model

- [ ] [parallel] Implement pure `no_std + alloc` `crates/kiln-core/src/repository_catalog.rs` with BLAKE3 canonical `RepositoryRecord` identities, GitHub/Gitea/Radicle remote & mirror validation/normalization (`canonical` vs `mirror`), cross-fleet `FleetLocalCopy` matching, and honest sync/drift classification (`synced | diverged | dirty | unknown | local_only`) plus positive and negative unit tests. r[kiln.source.repository_catalog_core]
- [ ] [parallel] Register `GET /v1/repositories` in `crates/kiln-core/src/implementation/authority/entry_points.rs` so `authority_coverage` stays green. r[kiln.fleet.local_copy_census]

## Phase 2: Fleet Local-Copy Census & Proxy

- [ ] [parallel] Implement read-only local checkout census (`crates/kiln-fleet/src/repositories.rs` and `GET /v1/repositories` in `crates/kiln-fleet/src/api.rs`) inspecting configured checkout roots and fleet hosts (`git`, `jj`, `pijul`, HEAD, branch/bookmark, dirty state, remotes, `.cairn/changes` count) with explicit `unavailable` host diagnostics and positive/negative tests. r[kiln.fleet.local_copy_census]
- [ ] [parallel] Extend `crates/kiln-web-service/src/fleet.rs` `upstream_path` allowlist to proxy `GET /api/fleet/repositories` -> `/v1/repositories` with strict method and path validation tests. r[kiln.fleet.local_copy_census]

## Phase 3: Web Service Import & Aggregated Catalog API

- [ ] [serial] Implement durable atomic repository catalog storage and aggregation in `crates/kiln-web-service/src/repository_catalog.rs` and wire `GET /api/repositories`, `POST /api/repositories/import`, `POST /api/repositories/{id}/mirrors`, `POST /api/repositories/{id}/remove`, and `POST /api/repositories/refresh` in `server.rs` (combining imported remotes/mirrors, `service.json` projects, `sources.json` checkouts, coordinator-local checkouts, and `kiln-fleet` `/v1/repositories`, triggering `EventHub` SSE updates, enforcing `request_id` idempotency and read-only Radicle discipline). r[kiln.web.repository_import_api]
- [ ] [parallel] Add end-to-end positive and negative HTTPS integration tests in `crates/kiln-web-service/tests/repository_import.rs` covering GitHub/Gitea/Radicle imports, canonical vs mirror attachment, credential/malformed-RID rejection, duplicate canonical rejection, unauthorized access (`403`), offline Fleet graceful degradation, and removal without touching disk checkouts. r[kiln.web.repository_import_api]

## Phase 4: Factory Control Plane UI & Verification

- [ ] [parallel] Add the **Repositories & Fleet Copies** hub in `3.2 System & Source` (`source.js`, `source.css`, `web.html`, `web.js`, `web.css`), **Import Repository / Add Mirror** Tier-2 guarded form (GitHub, Gitea/Forgejo, Radicle + `canonical`/`mirror` role + read-only Radicle notice), per-machine Fleet local-copy matrix (`synced`/`diverged`/`dirty`/`offline`, VCS, HEAD, Cairn count), `?inspect=repo:<id>` Slide-Over view, `1.1 Factory` summary strip, and `Ctrl+K` `> import repo` verb (`work-interaction.js`), updating `docs/console-port-inventory.json`. r[kiln.web.repository_import_ui]
- [ ] [parallel] Add positive and negative Node UI tests in `tests/repository-import-ui.test.mjs` covering GitHub/Gitea/Radicle import validation, mirror attachment, multi-machine local-copy aggregation display, offline host honesty, and `?inspect=repo:<id>` Slide-Over rendering. r[kiln.web.repository_import_ui]
- [ ] [serial] Run `cargo test --locked -p onix-kiln-core -p kiln-fleet -p kiln-web-service`, `cargo test --locked -p kiln-implementer --test authority_coverage`, `node --test tests/*.test.mjs`, and `cairn validate --root . --strict`. r[kiln.web.repository_import_ui]
