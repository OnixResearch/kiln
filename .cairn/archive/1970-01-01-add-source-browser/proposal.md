## Why

The operator can inspect retained CI runs, COBs and worker targets, but cannot read the repositories those runs came from. Git, Jujutsu and Pijul checkouts are all used in this workspace, and inspecting them currently means leaving the dashboard for a shell.

## What Changes

- Add a read-only **Source** view with an operator-declared source policy that admits local Git, Jujutsu and Pijul repositories by exact path.
- Show each source's observed head, a bounded revision/change log, directory listings and file content. Git and Jujutsu resolve an exact commit in the repository's Git object store; Pijul reads its current channel state through a separately executed `pijul` program and its working copy.
- Restore source, revision, directory and file selection from the URL, distinguish unavailable stores from empty ones, and keep all paths and executables server-owned.

## Impact

- **Files**: `crates/kiln-web-service/src/sources.rs`, service wiring and CLI, the live workspace UI, `config/web-sources.ncl` with fixtures, operator procedure.
- **Testing**: paired policy and route rejection coverage, exact-revision Git and Jujutsu reads, explicit Pijul unavailability, packaged service checks and the repository gates.
- **Owner / consumer**: Kiln service maintainers own the reader and view; the local operator owns repository selection, the `--sources` policy and activation. No fetch, checkout, mutation, signing, publication, CI or release claim.
