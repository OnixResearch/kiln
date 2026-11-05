## Context

`CiFact` is the canonical engine projection for one exact CI run. It already
contains the exact commit, project, run ordinal and outcome. Workflow nodes and
retained reports provide the missing check detail without accepting browser
assertions.

## Decisions

### Derived checks

For each run node, the shell emits `CiCheck`:

- `name`: the admitted node ID;
- `kind`: the admitted node kind;
- `outcome`: `pass`, `fail`, `cancelled`, `skipped`, or `unknown`, derived from
  the exact node observation;
- `artifact_digest`: the first retained report or artifact BLAKE3 for that node.

The field is omitted when empty. The UI renders it under the Canonical CI run,
keeping the run and commit binding visible.

### Exact requirement evaluation

`Policy.required_checks` is optional and serialized only when non-empty. Known
kinds are bounded; names are optional, non-empty, trimmed and bounded; duplicate
`(kind, name)` pairs are rejected. A requirement is satisfied only by a passing
check whose `CiFact.repository` and `CiFact.revision` match the exact reviewed
commit. The checklist gains a `checks` item only when the policy declares
requirements.

### Remaining #35 scope

Agent comment outcomes (`addressed`/`dismissed`/`open`) are not implemented
here. They need exact anchored-hunk history and remain a separate slice.
