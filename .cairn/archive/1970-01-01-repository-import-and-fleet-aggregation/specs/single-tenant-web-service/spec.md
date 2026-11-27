## ADDED Requirements

### Requirement: Operator-guarded multi-forge repository import, mirror management, and aggregated catalog API

r[kiln.web.repository_import_api]
`kiln-web-service` MUST expose `GET /api/repositories` (Viewer or Operator; `403` for Worker) returning the reconciled repository catalog combining durably persisted imports, `service.json` CI projects, `sources.json` local checkouts, coordinator-local checkouts, and proxied Fleet local copies with explicit per-source health. It MUST expose Operator-only endpoints `POST /api/repositories/import`, `POST /api/repositories/{id}/mirrors`, `POST /api/repositories/{id}/remove`, and `POST /api/repositories/refresh` enforcing `request_id` replay/conflict idempotency, atomic catalog persistence in `state_directory`, SSE `EventHub` invalidation, and strict read-only Radicle discipline (importing or mirroring a Radicle RID MUST NOT run Radicle init, signing, push, sync, or mirror publication, and removing a catalog entry MUST NOT delete any files on disk).

#### Scenario: Importing GitHub, Gitea, and Radicle remotes and attaching mirrors

r[kiln.web.repository_import_api.scenario.import_and_mirror]
- GIVEN an authenticated Operator importing a canonical Radicle RID and attaching GitHub and Gitea mirrors via `POST /api/repositories/import` and `POST /api/repositories/{id}/mirrors`
- WHEN `GET /api/repositories` is queried
- THEN the repository MUST appear under one stable BLAKE3 Kiln identity with all three remotes carrying their exact `canonical` or `mirror` roles
- AND replaying the same `request_id` with identical payload MUST be idempotent while conflicting payload reuse MUST fail with `409`
- AND a Viewer or Worker calling mutation endpoints MUST fail closed with `403`
