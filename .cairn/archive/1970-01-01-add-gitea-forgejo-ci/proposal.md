## Why

Kiln has a reviewed GitHub CI shell, but self-hosted Gitea and Forgejo
installations have no first-class source, webhook, status, or exact-source job
path. Operators currently have to bridge those forges through an external
service or treat them as generic Git checkouts, which loses signed event
admission, durable replay, exact-revision status, and bounded source evidence.

Gitea and Forgejo share the same webhook payloads and commit-status API. Forgejo
adds `X-Forgejo-*` delivery headers while retaining the Gitea and GitHub aliases.
One adapter can serve both without changing `kiln-core`.

## What Changes

- Add the standalone `kiln-adapter-gitea` shell and `kiln-gitea-job` exact-source
  helper.
- Add typed `kiln.gitea-ci.v1` and job profiles with explicit HTTPS instance,
  API, Git, repository, secret, workflow, retention, and bound inputs.
- Authenticate `X-Gitea-Signature`, `X-Forgejo-Signature`, and
  `X-Hub-Signature-256` HMAC-SHA256 headers over the exact original body.
- Admit Gitea/Forgejo `push`, `pull_request`, and `pull_request_sync` events for
  opened, reopened, synchronized, and closed/merged pull requests.
- Persist exact delivery/candidate replay, dispatch through the existing Lattice
  workflow port, and publish bounded Gitea commit statuses through
  `/api/v1/repos/{owner}/{repo}/statuses/{sha}`.
- Acquire the exact Git commit from the operator-admitted HTTPS Git base,
  materialize a fresh checkout, and execute reviewed argv under bounded
  process, output, deadline, and teardown policy.
- Add positive and negative event, status, replay, profile, and job tests, typed
  Nickel authoring profiles, operator documentation, and Nix package/app checks.
- Retain the documented read-only canonical observation export for local
  dashboard consumption, with exact-profile replay and non-destructive output
  admission; this adds no forge or workflow authority.

## Impact

- **Files**: new `crates/kiln-adapter-gitea`, `config/gitea-*.ncl`,
  `docs/gitea-ci.md`, workspace membership, flake packages/apps/checks,
  architecture and integration references.
- **Testing**: focused adapter tests, Nickel typechecks, workspace tests,
  Clippy, rustfmt, and Cairn validation.
- **Authority**: signed event admission and exact source/status projection only.
  No repository write, merge, release, or hosted-connection authority.
- **Scope**: the standalone self-hosted adapter. The optional `kiln-hosted`
  account/connection composition remains GitHub-only in this change; wiring a
  provider-neutral hosted source profile is a separate reviewed change.
