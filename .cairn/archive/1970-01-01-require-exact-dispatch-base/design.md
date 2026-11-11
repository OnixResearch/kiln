## Decisions

Base remains optional at work admission because intake may precede base
resolution, and the existing contract explicitly distinguishes absent from
inferred bases. Dispatch is stricter: it requires a present base and checks it
against both the work and the attempt's frozen base.

Policy base is operator data, not issue text. It is validated as an external
reference and copied unchanged; no branch or revision inference is introduced.

## Non-claims

This change does not resolve a base from a branch, provider, or clock. It does
not dispatch, verify or publish; it only prevents a base-less plan from being
reserved.
