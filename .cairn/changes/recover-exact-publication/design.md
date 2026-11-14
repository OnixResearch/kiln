## Decisions

Use the existing SQLite ledger transaction and exact attempt transitions, with a separate durable effect journal. A dispatch reservation survives transport failure and restart; a repeated caller receives reconciliation-only authority. Branch creation never overwrites, and PR reconciliation requires complete unambiguous all-state results. Ordinary GitHub merging is unsupported because its head precondition cannot preserve an exact base invariant. Independently observed external integration requires the exact merge parents and candidate tree, with final target readback. Cairn source updates must already be reviewed in the candidate; lifecycle store effects bind the authoritative result through the outer exact attempt envelope.

## Verification

Add crash/replay/stale/pagination/base-race/external-merge/unknown fixtures without running them mid-flight. Parent runs coordinated narrow crate checks. Live remote mutation requires exact operator approval and protected-repository qualification.

## Risks and Non-claims

No checked-in positive fixture qualifies a live provider atomicity guarantee. Unsupported landing remains PR-ready; a successful PR or closed-unmerged state cannot complete landed work.
