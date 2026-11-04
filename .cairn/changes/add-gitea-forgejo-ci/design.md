## Context

Gitea and Forgejo expose the same core webhook and commit-status shapes. Gitea
documents `X-Gitea-Delivery`, `X-Gitea-Event`, `X-Gitea-Signature`, and
GitHub-compatible aliases. Forgejo emits `X-Forgejo-*` in addition to the Gitea,
Gogs, and GitHub aliases. Both send the raw-body HMAC-SHA256 digest as lowercase
hex; the GitHub-compatible `X-Hub-Signature-256` value adds a `sha256=` prefix.

The existing GitHub shell is intentionally provider-specific. Gitea differs in
webhook payload field names, event action names (`synchronized`), status API
paths and response fields (`status` rather than `state`), authentication header
syntax (`token` rather than `Bearer`), and instance-supplied HTTPS origins.

## Decisions

### Separate adapter shell

Add `kiln-adapter-gitea` rather than adding provider branches to
`kiln-adapter-github`. `kiln-core` remains provider-neutral, GitHub behavior and
profiles replay byte-identically, and the Gitea shell can be reviewed and
qualified independently. Shared security-sensitive helpers that are already
provider-neutral may be reused only through a public, pinned interface.

### Explicit HTTPS origins

The service profile supplies `status.api_base` (for example
`https://git.example.com/api/v1`) and the job profile supplies `git_base` (for
example `https://git.example.com`). Both MUST be absolute HTTPS origins with no
userinfo, query, or fragment. The adapter derives status paths and clone URLs
from those admitted values. It never follows a webhook-supplied clone, compare,
HTML, or API URL.

Subpath installations are supported by including the subpath in both values.
The status resolver is pinned to the exact `api_base` host and port; DNS is
performed in the same bounded, killed/reaped child used by the GitHub shell.

### Webhook aliases and exact bytes

The HTTP shell accepts the first present non-conflicting header from each alias
family:

- delivery: `X-Gitea-Delivery`, `X-Forgejo-Delivery`, `X-GitHub-Delivery`;
- event: `X-Gitea-Event`, `X-Forgejo-Event`, `X-GitHub-Event`;
- signature: `X-Gitea-Signature`, `X-Forgejo-Signature`,
  `X-Hub-Signature-256`.

Duplicate critical headers, conflicting alias values, unsupported methods, and
oversized framing fail before event admission. Authentication covers the exact
original body before JSON decoding. Raw lowercase hex and the `sha256=` prefix
are accepted; every other digest form is rejected.

### Event admission

Push events require a `refs/heads/` or `refs/tags/` reference and a nonzero
40-hex `after` commit. A zero `after` is ignored as a deletion, never executed.

Pull-request admission accepts `opened`, `reopened`, `synchronized`, and, when
configured, `closed`. It also accepts the dedicated `pull_request_sync` event.
The selected revision is `pull_request.head.sha`, never a mutable branch or an
API-selected tip. The base repository must match the admitted repository id and
owner/name. A fork head is denied by default. Standalone profiles admit only the
`deny` untrusted policy because they have no durable approval surface; a
same-repository pull request is trusted under that policy, while a fork is
refused before any run or effect.

The payload's `before`, `after`, `compare_url`, `clone_url`, and other native
URLs remain observations only. Candidate identity is derived from the admitted
repository key/id, exact revision, selector, and closing action.

### Status projection

Status requests bind the exact `api_base`, repository, commit, fixed context,
Kiln run identity, and admitted CI state. A durable reservation precedes the
single POST. The request body uses Gitea's `CreateStatusOption` shape
(`state`, `context`, `description`); the response uses `CommitStatus` with the
`status` field. Reconciliation performs bounded read-only `GET
/repos/{owner}/{repo}/statuses/{sha}` pages and never reposts an ambiguous
write.

A lost, malformed, mismatched, rate-limited, or conflicting response leaves the
projection failed or unknown. Publication failure never changes the canonical
CI outcome.

### Exact-source job

The job profile binds the repository, exact Git base, reviewed argv, declared
environment, optional SecretSpec source credential, snapshot limits, transfer
bound, deadlines, output bound, and teardown grace. The helper acquires only
`{git_base}/{owner}/{repository}.git`, verifies the exact commit and bounded
tree, materializes a fresh checkout without Git metadata, and executes the
reviewed argv with a cleared environment. Source credentials cross only the
bounded acquisition child's stdin and never enter the checkout, command
environment, saved remote, or diagnostics.

### Replay and durability

Replay uses a separate `kiln.gitea-replay.v1` SQLite schema and table namespace.
Delivery identity is bound to the exact body and candidate/run. Equal deliveries
or equivalent candidate/pipeline requests reuse the original run; changed bytes
under one delivery id fail closed. Stored records bind the exact admitted
profile identity, so a changed profile cannot adopt old state.

### Scope boundary

This change does not wire Gitea into the optional `kiln-hosted` account and
connection composition. That composition currently stores GitHub-specific
profiles and run records; adding a provider-neutral hosted source profile is a
separate change that must preserve existing GitHub replay and financial
evidence. The standalone adapter is the supported self-hosted Gitea/Forgejo
path in this change.

## Risks

- Gitea and Forgejo payloads are similar but not identical across versions.
  Admission is fail-closed and only admits the documented selected fields;
  unsupported events are ignored without execution.
- Gitea can be configured for SHA-256 repositories. This adapter admits 40-hex
  SHA-1 revisions only; a SHA-256 repository fails before any effect.
- A fork PR can carry a same-named head repository. Identity checks use the
  numeric repository id and exact owner/name, and a mismatch fails closed.
- The status API returns `status` on reads but accepts `state` on writes. The
  adapter models both explicitly and rejects unknown states.
- Subpath installations require both `api_base` and `git_base` to include the
  same reviewed subpath; a mismatch produces a failed profile admission rather
  than a guessed URL.
