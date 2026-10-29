## Decisions

### Append-only profile authority

A pure admission function checks both profiles, requires a nonempty appended suffix, preserves every existing project in order by canonical identity, and allows only a nondecreasing max_projects bound. All other top-level fields, TLS/token/state roots and service bounds remain exact. New projects stay unenrolled until existing authenticated enrollment intake accepts them.

### Offline checked transition

The shell opens only an existing versioned store under the existing owned-state lock and validates the old profile, replay/linkage and retained bytes using the normal loader. Require terminal existing runs before extension. A single SQLite transaction compare-and-swaps the exact old binding to the new binding without serializing or rewriting state. Normal startup still requires exact identity. A held lock, stale profile, malformed state, missing database or rejected transaction fails closed. Operator backup and quiescence precede activation.

### Real project operation

Review each repository local notes, committed default branch and documented CI command. Use explicit reviewed Nickel worker/service inputs, exact Git identities, immutable source snapshots, the existing qualified Lattice runtime and explicit route mappings. Preserve the kiln-self-ci worker/helper/graph verbatim. Run initial jobs serially under existing host limits. Record actual outcomes, including genuine CI failures; enrollment is not a passing build claim.

## Risks

Local same-account reviewed commands are not hostile-code confinement. A project extension grants explicitly reviewed command/source authority and is not an unrestricted profile migration. Source checkout state is not the committed candidate. Repositories needing unavailable prerequisites remain explicitly unconfigured, not represented by fabricated successful runs.
