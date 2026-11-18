# Revision-bound trace pointer design

## Source contract

The primary Git-hook documentation at
https://traces.com/docs/sharing/git-hooks was read in an actual browser: one
trace per line, `traces:<externalId>` optionally followed by a shared URL.
Multiple traces may name one commit, and one trace may name multiple commits.
Kiln consumes that pointer grammar only, not the hosted service or session body.

The first profile is a boolean `session_traces`, absent/false by default, in the
operator-owned code-review profile. Disabled means no note read, no new revision
field, and no trace UI. Nickel exports an absent option as absent. The optional
provider method uses the existing Inspect/read boundary; unsupported providers
report an explicit unavailable observation rather than a fictitious empty note.

## Identity and history

`Revision.session_traces: Option<RevisionSessionTraces>` is serde-defaulted and
omitted when absent. The observation carries the exact commit and a tagged
absent/attached/unavailable state. Attached state retains a BLAKE3 digest of the
admitted note and a bounded list of declared source kind, external ID, optional
shared URL host and Kiln identity. Each identity hashes an unambiguous framed
binding of commit, note digest, source kind, external ID and host. It identifies
the admitted pointer, not unseen session-body content. Provider IDs remain
separate from Kiln identities.

The pure core checks exact commit/digest bindings and bounds at revision
admission. It does not read notes or grant authority. The trusted shell captures
notes when constructing the immutable revision, before durable command
reservation. Replays reuse that snapshot; later notes never rewrite old history.
Legacy revisions are not backfilled or reattributed. Copying an observation or
identity to another commit fails binding validation. The same declared trace
can legitimately occur on multiple commits with distinct Kiln bindings.

## Bounds and privacy

At most 16 KiB of note content, eight retained identities, and 256-byte external
IDs/hosts. IDs are bounded ASCII identifier tokens, not arbitrary prose. Shared
URLs must be absolute HTTPS without userinfo; only the normalized host survives.
No path, query, fragment, prompt, tool result or credential is copied into
metadata. Unknown/malformed lines make the observation explicitly incompatible,
not absent; oversized inputs/identity sets have distinct bounded reasons.
Missing notes are absent. All parsing and provider errors remain observational:
none can satisfy or bypass source, CI, ownership, review or submit policy.

## Surface and verification

Show selected revision trace IDs/digests/hosts as non-navigating text beside
separately labeled declared authorship and attested submission. Explicitly mark
historical context as untrusted and non-authoritative. No session body rendering
or remote trace request is supported in this slice. Disabled behavior has no new
DOM or serialized revision fields.

Check legacy byte replay, enabled present/absent/malformed notes on real exact
Git commits, crossed commit/digest refusal, bounded metadata, durable restart and
unchanged checklist/approval/submit decisions. Exercise the review surface in a
disposable local browser fixture, not by changing operated profiles or creating
live review objects.
