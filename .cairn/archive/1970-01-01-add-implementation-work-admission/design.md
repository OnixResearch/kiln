## Decisions

The contract lives in `kiln-core` as a pure, no-I/O module. Source adapters
supply exact source bytes, an adapter-supplied monotonic sequence, and explicit
acceptance obligations; free-form issue text never becomes execution permission.

Identity excludes the source sequence so an edit with identical content admits as
a duplicate rather than a new version. The canonical alias key excludes content
so versions of one source unit group together. Superseded versions stay in
history; the prior version is never rewritten.

The ledger treats an equal sequence with different content as ambiguous rather
than choosing a winner. This refuses races that the adapter failed to order.

Dependencies must reference currently retained identities. Superseding a
dependency therefore blocks its dependents visibly instead of silently rebinding
them to the new version.

Archive readiness is deliberately absent. It is a Cairn lifecycle fact, not
implementation readiness, and an archived or deleted change never proves work
completion.

## Non-claims

This module admits and ranks work. It does not authenticate webhooks, poll
providers, parse Cairn lifecycle, claim work, dispatch workers, verify candidates,
publish pull requests, or grant release authority. Until the adapters and #45–#48
land, admitted work has no dispatch consumer.
