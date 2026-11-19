## Why

Britton's interactive agents coordinate exclusive work on the private swarm
news server: `CLAIM <repository or machine> <scope>` before editing, `RELEASE`
with `References` to the claim when finished, and a claim without a release after
24 hours is stale. Kiln's implementation attempts are invisible to that swarm, so
an attempt and an interactive agent can edit the same repository and scope at the
same time.

Kiln's durable claims, leases and fences remain authoritative inside Kiln. This
change adds the swarm side: a default-off mirror of attempt claims and releases
over renews/iroh, and overlap deferral from recent `CLAIM` articles. Articles are
untrusted data and can only defer, never authorize.

## What Changes

- Add pure claim rules in core: subject grammar, body formatting with required
  signature, redaction admission, RFC 2822 date parsing, and the overlap/staleness
  decision over subjects and `References` only.
- Add `kiln-adapter-swarm`, a bounded client for the pinned `iroh-news`
  executable: cleared environment, exact argv, no credentials in Kiln, JSON
  article parsing, and post outcomes `accepted`, `rejected`, `unknown`,
  `unavailable`.
- Add the `kiln-swarm` operator CLI (`read`, `post`) so the adapter is usable
  before automatic wiring lands.
- Mirror one `CLAIM` per claimed attempt and one `RELEASE` on its terminal phase
  or lease expiry, with recorded outcomes; defer admission on an open overlapping
  claim by another author.
- Keep the mirror off unless an operator supplies its configuration. No swarm
  connection is made when it is absent, and no swarm failure fails an attempt.

## Impact

- **Core**: `kiln_core::implementation::swarm` (pure rules and tests).
- **Adapter**: new `kiln-adapter-swarm` crate and `kiln-swarm` binary.
- **Shells**: attempt transition hooks and deferral in `kiln-implementer` (follow-up
  tasks in this change).
- **Configuration**: an explicit, default-off operator profile; credentials remain
  with `iroh-news`/SecretSpec.
- **Lifecycle**: Kiln #94 owns the mirror; swarm content never grants authority.
