# Why
Issue #66 asks for stable logical case IDs and comparable retained UI capture evidence for the disposable website explorer. Restart creates fresh runtime identities, so reviewers and agents cannot reliably request the same logical case or compare visual/semantic observations across source revisions.

## What Changes
- Add stable case IDs, a versioned fixture recipe, declared presentation/interaction requirements and resolved runtime subjects to /examples.json.
- Resolve each named case against the freshly seeded workspace into actual returned run/review/worker IDs and routes.
- Add a small case selection/compare view to the examples index with keyboard and narrow/dark support.
- Add a bounded capture path through existing browser tooling with screenshot, semantic/accessibility observation and provenance manifest.
- Record the Kiln-to-Rats migration comparison cohort as blocked on #67/#68.

## Impact
Exploration UX and evidence only. No provider, CI, review-acceptance or deployment authority is created. Synthetic execution remains visibly synthetic.

## Authority
Worker implementation for issue #66 first slice; parent owns integration checks.
