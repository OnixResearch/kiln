## Decisions

Polling lives behind a transport port so the same normalization is used for
webhook and recovery paths while HTTP, TLS, credentials and retries stay in a
shell. The port is scoped by owner/repository, page and update sequence.

Polled records have no raw webhook bytes, so the source identity is a
length-prefixed BLAKE3 over the normalized fields (repository, state, actor,
title, number, update sequence, body). That is content-bound and stable for
identical API observations.

Duplicate records across pages keep the highest update sequence. Remaining pages
after the reviewed bound fail recovery rather than returning a truncated set.

## Non-claims

Polling does not authenticate HTTP, schedule itself, persist, claim or dispatch.
It produces the same work observations as webhook intake in deterministic order;
ledger admission and the durable shell still decide deduplication and claims.
