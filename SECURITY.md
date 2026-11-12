# Security policy

Kiln runs reviewed jobs with the authority of the worker account that executes
them: it is not a sandbox. Bounds, file roots and temporary directories reduce
specific failure modes and do not confine arbitrary commands. The optional
hosted namespace backend is for trusted jobs only, every hosted receipt reports
`hostile_isolation_claim:false`, and Kiln claims no hostile-code confinement or
hostile multi-tenant isolation. A signature does not establish source trust or
action authority.

## Supported versions

Security fixes are applied to `main`. `0.1.0` is the only version line in this
repository, and no tagged release exists on any remote. No response or
remediation time is offered.

## Reporting a vulnerability

Do not open a public issue that contains vulnerability details.

Use GitHub private vulnerability reporting on the GitHub projection:

<https://github.com/OnixResearch/kiln/security/advisories/new>

If that route is not enabled for the repository, open a minimal public issue on
the GitHub projection that asks the maintainers for a private channel and
contains no vulnerability details.

Include:

- the exact revision you tested;
- the affected binary or adapter;
- the relevant profile or configuration with secrets removed;
- a minimal reproduction;
- the observed impact;
- preconditions such as a required credential, role or network position.

Maintainers reproduce the report against the reported revision and coordinate
disclosure. No timeline is promised. Do not include secrets or third-party
personal data beyond what the reproduction requires.

## Security-relevant surface

- **Trigger authentication before admission.** A trigger shell verifies its
  protocol signature before it creates a command. The GitHub adapter
  authenticates the bounded original webhook body with HMAC-SHA256 before JSON
  parsing or durable admission.
- **Capability admission before dispatch.** A provider profile is admitted
  before an external effect. The selected provider cannot change without a new
  admitted command.
- **Service authentication.** The single-tenant web service requires
  authentication. The HTTPS observation shell accepts Ed25519 or UCAN
  holder-invocation auth, and the iroh shell serves only allowlisted peers with
  BLAKE3-verified payloads.
- **Secret handling.** Secret references resolve through the pinned SecretSpec
  executable in the provider shell. Tokens are never stored in Nickel, JSON,
  workflow argv, URLs or CI environment, and source credentials are not
  command environment.
- **Bounded inputs, outputs and logs.** Log bytes carry stream, segment, total,
  read and retention bounds. Gaps and truncation remain explicit.
- **COB mutation plans.** A mutation plan binds exact current heads, the shell
  reloads the object before signing or writing, and concurrent heads remain
  visible.
- **Canonical identities.** Kiln artifacts use `kiln.packed-preserves.v1` with
  BLAKE3 identities. Provider references keep a namespace, native kind and
  native value, and protocol digests stay separate from Kiln identities.
- **Durable effect records.** The journal uses expected revisions; the outbox
  uses effect identities, leases and idempotency keys. A transport failure
  after acceptance stays `Unknown` until reconciliation.
- **Exact source acquisition.** A GitHub job verifies BLAKE3 over the original
  job-profile JSON bytes and acquires an exact commit over HTTPS before it
  admits a bounded snapshot.
- **Local host sockets.** The standalone Aspen host requires an owned private
  final socket directory, an owner-only socket, and clients that run as the
  same user.
- **Untrusted pull requests.** The standalone GitHub and Gitea adapters refuse
  fork pull requests. The web service holds untrusted pull-request runs until
  an operator approves them.

## Not a vulnerability by itself

- A reviewed job acting with the authority it was granted. Kiln grants no
  ambient authority beyond the selected worker account.
- The trusted-only namespace backend not confining hostile code, and the
  absence of hostile multi-tenant isolation. Both are documented limits.
- A defect in a third-party dependency that is not reachable through a Kiln
  boundary. Report it upstream and note the affected Kiln revision if Kiln
  exposes it.
- A passing gate or run being read as source trust, build correctness or
  release eligibility. A passing decision proves only the checked relation
  between one admitted state, command, event set and effect set.

A report that a documented property does not hold, for example an untrusted run
receiving a secret, is in scope.

## Hardening expectations for changes

- Add positive and negative tests together, including unsupported capabilities,
  stale identities, replayed triggers, timeouts, cancellation and ambiguous
  outcomes.
- Unsupported capabilities fail before any external effect.
- Use named limits, windows, bounds and retry policies instead of unexplained
  numeric values.
- Keep pure core decisions free of filesystem, process, network, clock and
  credential dependencies.
- Do not edit `flake.lock` manually.
- Vendor imports change only through their import scripts, with the exact
  source revision recorded; do not edit vendored snapshots by hand.

## Deployment guidance

Read [the operator guide](docs/operator.md) for local procedures and
[the hosted-ci procedure](docs/hosted-ci.md) before exposing any service. Local
qualification is not production qualification. A retained receipt covers one
recorded cohort; it does not qualify a changed revision, executable or
deployment.
