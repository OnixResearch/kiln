## Decisions

Nickel predicates mirror the Rust admission bounds but remain the authoring
boundary, not the authority: Rust still validates every admitted work record.
Keeping both is intentional because exported JSON can be edited by hand.

The policy builder emits `allow_fork_pull_requests = false` and no untrusted
pull-request policy; implementation work is not a pull-request execution grant.

Enrollment uniqueness prevents ambiguous actor/repository admission.

## Non-claims

Nickel export does not admit, claim or execute work. A valid file is input to the
operator command, which still authenticates deliveries and applies the durable
ledger.
