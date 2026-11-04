# Gitea and Forgejo CI adapter

`kiln-adapter-gitea` is the standalone Kiln shell for Gitea and Forgejo
repositories. It accepts signed webhook deliveries, admits exact push and
pull-request revisions, persists durable replay state, dispatches the reviewed
Lattice workflow, and publishes bounded commit statuses back to the forge.
`kiln-gitea-job` is the exact-source job helper used by that workflow.

Gitea and Forgejo share the same documented webhook payloads and commit-status
API. Forgejo additionally emits `X-Forgejo-*` headers while retaining the Gitea,
Gogs, and GitHub-compatible headers. The adapter accepts all of the documented
signature/delivery/event aliases.

## Scope and non-claims

This is a self-hosted adapter. It is not part of the optional `kiln-hosted`
account/connection composition, which remains GitHub-only until a separate
provider-neutral hosted source profile is admitted.

The adapter supports SHA-1 repositories (`40`-hex revisions) and JSON webhook
deliveries only. A SHA-256 repository, form-encoded delivery, unknown event
shape, unsupported action, or missing head repository fails closed before any
run or effect.

Passing local tests does not prove a live Gitea/Forgejo deployment, source
trust, workflow correctness, host confinement, release eligibility, or
exactly-once distributed effects. Status publication is a projection, not
canonical Kiln CI state. No repository write, merge, release, or administrative
authority is granted.

## Required forge permissions

Use a repository-scoped access token with:

- repository contents read for exact source acquisition;
- commit status read/write for status publication.

Use a separate webhook secret. Do not grant repository contents write,
administration, package, or organization authority. Keep the webhook secret,
API token, and source token in distinct SecretSpec references.

## Profile authoring

The service profile builder is `config/gitea-ci.ncl`. It expects:

- the canonical repository owner and name;
- the numeric Gitea/Forgejo repository id;
- the API base, exactly `https://HOST[/SUBPATH]/api/v1`;
- the replay database path;
- webhook and status SecretSpec references;
- a reviewed Lattice workflow profile.

The job profile builder is `config/gitea-job.ncl`. It adds the normalized
instance Git base, for example `https://git.example.com` or
`https://example.com/gitea`.

```ncl
# /etc/kiln/gitea-deployment.ncl
{
  owner = "acme",
  repository = "app",
  repository_id = 1234,
  api_base = "https://git.example.com/api/v1",
  replay_database_path = "/var/lib/kiln/gitea/replay.sqlite",
  webhook_secret = {
    program_path = "/nix/store/.../bin/secretspec",
    program_blake3 = "...",
    project_root = "/etc/kiln",
    profile = "prod",
    provider = "systemd",
    secret_key = "KILN_GITEA_WEBHOOK_SECRET",
  },
  status_secret = {
    program_path = "/nix/store/.../bin/secretspec",
    program_blake3 = "...",
    project_root = "/etc/kiln",
    profile = "prod",
    provider = "systemd",
    secret_key = "KILN_GITEA_STATUS_TOKEN",
  },
  workflow = import "/etc/kiln/lattice-workflow.ncl",
}
```

```ncl
# /etc/kiln/gitea-service-export.ncl
(import "config/gitea-ci.ncl") (import "/etc/kiln/gitea-deployment.ncl").service
```

```console
nickel export --format json /etc/kiln/gitea-service-export.ncl > /etc/kiln/gitea-service.json
```

The API and Git bases must be normalized: no trailing slash, user information,
query, fragment, or explicit default `:443` port. A subpath installation must
include the same subpath in both bases. Payload-supplied clone, compare, HTML,
and API URLs are observations only and are never followed.

## Forge webhook setup

In the repository settings, create a Gitea or Forgejo webhook:

- **Target URL**: the externally reachable HTTPS URL ending in
  `/gitea/webhook`;
- **HTTP method**: `POST`;
- **POST content type**: `application/json`;
- **Secret**: the same value referenced by `webhook_secret`;
- **Trigger**: push events and pull-request events (including synchronization).

The listener binds only a loopback address. Terminate TLS in a reviewed reverse
proxy and forward the exact body, `Host`, `Content-Type`, and the forge's
delivery/event/signature headers without rewriting them.

Accepted header aliases:

| Meaning | Headers |
| --- | --- |
| delivery | `X-Gitea-Delivery`, `X-Forgejo-Delivery`, `X-GitHub-Delivery` |
| event | `X-Gitea-Event`, `X-Forgejo-Event`, `X-GitHub-Event` |
| signature | `X-Gitea-Signature`, `X-Forgejo-Signature`, `X-Hub-Signature-256` |

`X-Gitea-Signature` and `X-Forgejo-Signature` contain lowercase HMAC-SHA256 hex
without a prefix. `X-Hub-Signature-256` is accepted with its `sha256=` prefix.
Duplicate or conflicting aliases fail before JSON decoding.

## Run the service

```console
kiln-adapter-gitea --profile /etc/kiln/gitea-service.json
```

Inspect or reconcile one retained run:

```console
kiln-adapter-gitea status --profile /etc/kiln/gitea-service.json --run "$RUN_BLAKE3"
kiln-adapter-gitea reconcile --profile /etc/kiln/gitea-service.json --run "$RUN_BLAKE3"
```

Export the canonical observation journal:

```console
kiln-adapter-gitea export --profile /etc/kiln/gitea-service.json \
  --run "$RUN_BLAKE3" --output /var/lib/kiln/gitea/run.json
```

The worker polls the existing Lattice workflow socket. A pending status is
reserved before dispatch; terminal status publication is reconciled by exact
run identity and exact commit. Ambiguous writes remain unknown and are never
blindly reposted.

## Run an exact-source job

`kiln-gitea-job` verifies the BLAKE3 identity of the original job-profile JSON,
consumes the fixed Lattice source context, fetches only the admitted repository
and exact commit, materializes a fresh checkout without Git metadata, and runs
the reviewed argv.

```console
kiln-gitea-job --profile /etc/kiln/gitea-job.json --identity "$JOB_PROFILE_BLAKE3"
```

The job profile's `git_base` must be the normalized HTTPS instance base. The
source credential is optional; when present it is resolved through the pinned
SecretSpec resolver and crosses only the bounded acquisition child's stdin.
Credentials never enter the checkout, command environment, saved remote, or
diagnostics.

## Failure behavior

- A changed signature, missing header, duplicate/conflicting header, oversized
  body, unsupported method, or non-JSON content type is rejected before event
  admission.
- A repository-id/name mismatch, fork pull request, zero/deleted push, malformed
  commit, unsupported pull-request action, or SHA-256 revision creates no run.
- An equal delivery or equivalent candidate reuses the original run; changed
  bytes under one delivery id conflict and fail closed.
- A failed, lost, or ambiguous status write never changes the canonical CI
  outcome; reconciliation is read-only and bounded.
- Missing teardown evidence or an unknown workflow outcome remains explicit.

## Maintenance

Kiln maintainers own the adapter, conformance tests, and repeatability rail.
Operators own the forge instance, webhook secret, API/source credentials, TLS
reverse proxy, worker isolation, retention, and deployment evidence.
