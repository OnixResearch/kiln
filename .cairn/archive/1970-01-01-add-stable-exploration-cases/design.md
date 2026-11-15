# Context
The explorer currently identifies examples by title and runtime-specific URL. /examples.json exposes title/href/detail only. Consumers in index.js and the Bombadil admit_catalog read those three fields.

# Decision
Extend Example with stable case_id, recipe, recipe_revision (BLAKE3 over actual source inputs), source_revision, presentation and interaction requirements, and resolved subjects. Keep title/href/detail for compatibility. Resolve logical cases to actual seeded IDs rather than hand labels; the worker case resolves to the exact target whose only heartbeat expires. `GET /examples/cases/<case_id>?recipe=&workspace=` resolves a case server-side: an unknown case is 404, and a moved recipe or another workspace's identity is 409. Plain examples receive no case identity.

The examples index gains a case selection form (case, theme/density, viewport) that builds a linkable URL containing case plus recipe/source identity. `scripts/explore-capture.py` drives a supplied Chromium executable over the DevTools pipe (`--remote-debugging-pipe`, fds 3/4) with a disposable profile, an exact emulated viewport and named count, size and time limits. It resolves the case first, waits until the document is complete, the main landmark is present, every exact subject identity is in the document, no region is busy and fonts are loaded, then writes screenshot, semantic observation and manifest. `scripts/explore-compare.mjs` classifies two manifests and reports subject changes without normalizing them.

# Verification
Cairn gates; Rust example tests for case identity, recipe revision and resolution outcomes; node tests for the manifest classifier; captures of every named case from two clean starts, compared with the classifier; capture refusals for unknown, moved, foreign and out-of-bounds inputs; and the Bombadil rail over the rebuilt explorer.

# Non-claims
No real provider, CI, review acceptance, deployment or pixel parity claim. Migration cohort blocked on #67/#68.
