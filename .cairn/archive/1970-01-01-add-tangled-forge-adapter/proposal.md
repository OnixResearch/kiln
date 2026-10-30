## Why

Issue #14 asks for a Tangled forge adapter. Tangled keeps repositories on knots and runs CI on spindles, which are reached through a small XRPC surface. Kiln has GitHub and Radicle adapters but nothing for Tangled. Tangled projects therefore cannot use Kiln's exact-source gating. The issue first asks for an investigation, then for an adapter if the investigation supports one.

## What Changes

- **Design note:** `docs/tangled-integration.md` covers the following, and states what was read and what still needs verification:
  - knot → spindle event delivery;
  - spindle authentication and status;
  - the repository DID identity model;
  - exact-commit acquisition;
  - the mapping onto Kiln ports.
- **New crate `kiln-adapter-tangled`:**
  - **Pure intake:** admission of `org.tangled.event.notifyEvent` push and manual events, and of `org.tangled.event.pullRequest` triggers. It includes AT Protocol service-auth JWT verification (ES256K and ES256 against the issuer's DID-document key). Status deliveries are signed and verified in the Tangled webhook HMAC format.
  - **Repository identity:** operator-pinned repository DID → knot, clone URL, trusted actors and approvers. Unknown DIDs are refused.
  - **Replay:** push replay (`repo, ref, oldSha, newSha`) returns the existing pipeline. A token `jti` cannot be reused.
  - **Workflow mapping:** a reviewed, fail-closed `.tangled/workflows` subset with a `when` → trigger-filter mapping.
  - **Runs:** one Kiln run per matched workflow, bound to the exact commit. Tangled identities are kept as provider facts.
  - **Untrusted pull requests:** handled as #12 requires, blocked until `approvePipeline`.
  - **Spindle-compatible shell:** XRPC routing for `notifyEvent`, `listWorkflowDefinitions`, `getWorkflowDefinition`, `retryPipeline`, `approvePipeline` and `getPipeline`.
  - **Source:** a Git CLI shell that acquires exact SHA-1 and SHA-256 commits.
  - **Status:** a status publisher that posts the `#pipeline` view (HMAC-signed) and returns `Unknown` when the outcome is uncertain.
  - **Server:** a bounded HTTP server.
- **Configuration:** a typed Nickel profile `config/tangled-spindle.ncl`, with positive and negative fixtures.

## Impact

- **Consumers:** operators who want Tangled repositories built by Kiln, with Kiln acting as their spindle.
- **Owners:** Kiln adapter maintainers own `crates/kiln-adapter-tangled` and the profile.
- **Affected areas:**
  - root `Cargo.toml` (workspace member);
  - `Cargo.lock` (new crates `k256` and `p256` features, already cached);
  - new crate files;
  - `docs/tangled-integration.md` and `docs/integrations.md`;
  - `config/tangled-*.ncl` and the fixtures.
- **Testing:** focused `cargo test`, `cargo clippy` and `cargo fmt` for `kiln-adapter-tangled`; Nickel typecheck and export of the positive and negative fixtures.

## Out of Scope

- Federation, AT Protocol identity or Tangled types in Kiln core crates.
- Jetstream consumption, `org.tangled.secret.*`, `org.tangled.webhook.*` management, log streaming, `cancelPipeline`, `queryPipelines`, schedules and quotas.
- Live interoperability with a production knot. This is listed as not established.
- A deployment binary, systemd units and a flake check. These are listed as wave-2 hookups.
