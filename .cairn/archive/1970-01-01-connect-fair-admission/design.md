## Context

Reuse operations rank_tenants and existing implementation current claims, fences and SQLite state transactions. Intake deliveries remain separate.

## Goals / Non-Goals

Bound ready items, bytes, age, deadlines, children and active work. Include held owner-confirmed debits immediately. Do not invent physical grants, inference routing, automatic attempts or model fallback.

## Decisions

A pure serializable ledger stores immutable policy, ordered work, finite lineage accounts and operation/generation receipts. Preclaim waiting creates no attempt; postclaim waiting retains consumed attempt budget and lease. Eligibility filters precede fairness. Observed usage explicitly identifies covered operations; only exact matching settled receipts remove conservative debit, preventing double charging. Unknown effects retain debits and active slots. Parent quota transfer is allocation, not consumption. Physical deployment operations are deduplicated independently from worker request operations. Shell mutations use immediate transactions with existing work/attempt state, save before external calls and revalidate after owner grant. Named bounded retry-after never creates an attempt.

## Risks / Trade-offs

Finite journal capacity fails closed; no unbounded garbage collection. Fairness applies only to fresh comparable eligible finite workloads within one admitted observation window; non-preemptible and heterogeneous work has no strict latency guarantee. Real producer cohorts must provide exact operation, generation and reconciliation evidence.

## Migration Plan

No policy means existing path unchanged. Explicit policy selects new admission.

## Open Questions

Live cross-provider qualification remains an operator prerequisite, not a code permission.
