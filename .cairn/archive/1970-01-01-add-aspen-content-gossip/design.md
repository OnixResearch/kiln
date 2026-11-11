## Decisions

The content store is derived from the state-root name rather than accepting a
second path argument, keeping the existing command surface explicit and
fail-closed. Content publication happens inside the effect port so an effect
cannot claim success when exact bytes were not durably written.

Gossip hints are data-only. The listener bounds every resource before and during
the event loop and drops malformed/expired hints rather than executing or
retrying. Dedupe keys bind schema, scope, kind, epoch and content reference.
Topic ids use a domain-separated BLAKE3 hash; public scope text is not treated as
a secret.

## Non-claims

A hint is not a reservation, authority grant, delivery proof or execution
request. Content publication does not prove retention, confidentiality or
independent verification. The gossip listener is not a migration or VM-control
channel.
