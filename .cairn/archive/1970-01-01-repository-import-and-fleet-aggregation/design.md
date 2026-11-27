## Context

Kiln already keeps provider implementations behind versioned ports (`AGENTS.md`), uses BLAKE3 for Kiln-owned identities, treats GitHub/Gitea/HTTPS mirrors as declared projections alongside canonical Radicle RIDs, and separates read-only observation from external mutation. Currently, `sources.rs` observes only a flat list of coordinator-local paths in `sources.json`, and `kiln-fleet` exposes machines, claims, work, attempts, and decisions without a cross-host repository checkout census.

## Decisions

### Decision: Pure `kiln-core::repository_catalog` with BLAKE3 canonical identities and explicit `canonical` vs `mirror` roles

**Choice:** Implement `crates/kiln-core/src/repository_catalog.rs` (`no_std + alloc`) owning `RepositoryRecord`, `RemoteEndpoint` (`github | gitea | radicle`, `role: canonical | mirror`), `FleetLocalCopy` (`machine`, `path`, `vcs: git | jj | pijul`, `head_commit`, `branch_or_bookmark`, `dirty`, `cairn_count`, `remotes`, `health`), and pure `reconcile_catalog()` matching local copies to imported records via normalized remote keys (`github:<owner>/<repo>`, `gitea:<host>/<owner>/<repo>`, `radicle:rad:z...`). Assign each repository a deterministic BLAKE3 `ContentId` derived from its Kiln-owned canonical key; allow at most one `canonical` remote per repository while permitting multiple `mirror` projections across GitHub, Gitea, and Radicle.

**Rationale:** Keeps provider identities strictly as explicit facts rather than primary Kiln keys, guarantees deterministic matching and drift classification (`synced | diverged | dirty | unknown | local_only`) testable without I/O or external services, and prevents duplicate or credential-bearing URLs from entering catalog state.

### Decision: Read-only Fleet checkout census (`GET /v1/repositories`) with explicit offline host reporting

**Choice:** Add a read-only checkout scanner in `crates/kiln-fleet` exposed at `GET /v1/repositories` (and proxied at `/api/fleet/repositories` via `fleet.rs`) that inspects declared checkout roots and configured repository paths across fleet hosts using bounded read-only inspection (reading HEAD, branch/bookmark, dirty state, remote URLs, VCS kind, and `.cairn/changes` directory count). Unreachable hosts emit explicit `health: "unavailable"` entries with error diagnostics rather than being omitted or reported as clean.

**Rationale:** Gives operators a single live inventory of every local working copy across `britton-desktop`, `aspen3`, and peer hosts without mutating working trees, running `git fetch`, or touching Radicle signing/push paths.

### Decision: Durable operator-guarded Catalog API (`kiln-web-service`) with read-only Radicle guardrail

**Choice:** Persist imported repositories atomically in `<state_directory>/repositories.json` (bounded `MAX_CATALOG_REPOS = 256`, `MAX_REMOTES_PER_REPO = 16`, `request_id` replay/conflict protection). Expose `GET /api/repositories` (Viewer/Operator), `POST /api/repositories/import`, `POST /api/repositories/{id}/mirrors`, `POST /api/repositories/{id}/remove`, and `POST /api/repositories/refresh` (Operator-only). Importing a Radicle RID (`rad:z...`) records it strictly as a read-only declaration/observation and NEVER invokes Radicle init, signing, push, sync, or delegate mutation.

**Rationale:** Enables 1-click repository and mirror onboarding from the Factory Control Plane while preserving Kiln's strict security boundaries (`AGENTS.md` Radicle hosting rules, zero shell execution of untrusted repo hooks, and explicit separation of catalog observation from CI/Fleet execution grants).

### Decision: Unified `Repositories & Fleet Copies` workbench in `3.2 System & Source`, `1.1 Factory`, Slide-Over, and `Ctrl+K`

**Choice:** Add a `Repositories` tab/section inside `3.2 System & Source` (`source.js`), surface a concise cross-fleet repository drift summary on `1.1 Factory`, support `?inspect=repo:<id>` in the `44rem` Slide-Over Inspector, and register `> import repo` in `Ctrl+K` (`work-interaction.js`). Keep all mutations behind `#control-dialog` Tier-2/3 confirmations with explicit Radicle read-only notices and CLI/config parity.

**Rationale:** Fits cleanly inside P1's consolidated 7-destination IA without adding rail sprawl or breaking `docs/console-port-inventory.json`.

## Risks / Trade-offs

- Scanning large local checkout trees must remain strictly bounded (`MAX_SCAN_REPOS`, bounded subprocess timeouts, shallow depth 1–2) so refresh calls never stall `kiln-web-service` or `kiln-fleet` request threads.
- Importing a repository into the catalog does not automatically grant CI workflow execution or Fleet agent dispatch; the UI must explicitly label catalog entries as `Observed` vs `Enrolled (Fleet)` vs `CI Configured`.
- Remote Gitea instances may use custom hostnames/ports; URL normalization must require `https://` (or `ssh://` / `git@`) with valid hostnames and reject embedded passwords, IP-literal SSRF probes, and control characters.
