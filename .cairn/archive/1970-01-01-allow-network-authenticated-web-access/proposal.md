## Why

The current Kiln self-CI consumer is exposed on a private Tailscale listener, but its browser still requires an operator token. The operator explicitly requests tokenless tailnet access while keeping localhost and worker credentials intact.

## What Changes

- Let the browser discover already-authorized same-origin access through a bounded unauthenticated read of the existing project endpoint; retain manual bearer login when that request returns 401.
- Keep frontend session admission separate from bearer-token presence so live observations, mutations, logs and artifacts work without a client-held token.
- Configure only the operator-owned tailnet reverse proxy to supply its backend credential, without disclosing that credential to clients or changing canonical service/worker profiles.
- Verify actual browser access, read/write and origin boundaries, localhost/worker authentication, history preservation and controlled restarts.

## Impact

Kiln dashboard maintainers own the reusable browser behavior. The local operator owns the tailnet listener, peer reachability, private proxy credential material and activation. Every peer permitted to reach the listener gains operator authority. This is not an individual-account login, a sandbox, public Funnel authorization, provider requalification or lifecycle acceptance.
