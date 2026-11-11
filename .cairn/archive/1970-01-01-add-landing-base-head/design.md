## Decisions

The base ref is read through the same fixed-origin client after PR/check/review
evidence. The expected base commit is part of the plan, so a moved base fails
before a caller can act on stale landing evidence. This is a point-in-time read,
not an atomic merge precondition.

## Non-claims

The observation does not prevent a base race during a later merge and is not
merge authority.
