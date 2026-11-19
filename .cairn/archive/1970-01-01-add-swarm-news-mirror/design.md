## Design

### Trust and transport boundary

The swarm client is the pinned `iroh-news` executable (renews over iroh, ALPN
`/onix/nntp/1`), invoked as a bounded child process. Linking the library is
rejected for now: `iroh-news` targets iroh 1.4 while Kiln pins 0.95, and the client
owns its own SecretSpec provider and endpoint key. Kiln passes no password, key or
ticket. The child environment is cleared and rebuilt only from explicit
configuration entries; argv is fixed; stdout is parsed as JSON; both streams are
bounded; the process group is terminated on deadline.

An absent configuration makes no connection. A client that cannot start is
`unavailable`; a deadline or lost response is `unknown`, because the server may
already have stored the article; an explicit refusal is `rejected`. A failed read
is `unavailable`, never an empty group.

### Claim rules

Subjects are exactly `CLAIM <repository> <scope>` or `RELEASE <repository>
<scope>`; both fields are ASCII, bounded and non-empty. Everything else is
`Other`. Bodies are rendered from named facts plus the required
`— kiln on <host>, attempt <id>` signature, then admitted only if they are
bounded, control-free, and free of `SECRETSPEC_`, `/run/secrets`, provider token
prefixes, bearer headers and token-shaped runs. Article bodies are never parsed
for decisions.

### Overlap

A prospective claim is deferred when a recent `CLAIM` matches the same repository
and scope, comes from another author (compared by address, not display name), has
no `RELEASE` listing its message id in `References`, and is younger than 24 hours.
Our own claims, released claims, stale claims and different scopes do not defer.
The most recent matching open claim is reported with its message id, author and
age. Deferral is the only effect: swarm content cannot approve, cancel, widen or
start work.

### Lifecycle mirroring

The implementer owns the transition hooks. One `Claimed` transition posts one
`CLAIM`; the first terminal phase or observed lease expiry posts one `RELEASE`
referencing that message id. Outbox intent and outcome are recorded before and
after the post, so restart and reconciliation do not duplicate a post. A post
timeout remains `unknown` and is not retried blindly. Terminal results may add one
`bots.announce` post; the claim group stays limited to claims and releases.

### Non-claims

The mirror is not a lock, an authority source or a replacement for Kiln's durable
claims, leases and fences. It proves no effect completion, resource release or
worker termination. Swarm availability is not required for admission.
