## Decisions

Metrics live on the existing attempt row as an optional receipt, so no new store
or trace backend is introduced. Claim and terminal timestamps are additive
fields with serde defaults; they do not participate in identity, fencing or
state admission.

Observation is pure and separate from recording. The CLI previews unless
`--record true` is explicit. Recording keeps the first receipt so a later clock
or usage update cannot rewrite history; callers needing a later snapshot must
create a new attempt/evidence record.

Usage is two typed observations. An absent observation carries a reason and
serializes as `status: absent`; observed tokens/calls are separate, so missing
data cannot be confused with zero.

## Non-claims

No cost is measured by Kiln itself, no token attribution is imported from
Clankers, and no incident/factory-health score is computed. Trace references are
pointers to existing evidence, not a new tracing backend.
