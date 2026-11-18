# Removed GitHub mutation requirements verification

## Delivery and ownership

The accepted `github-landing`, `github-source-status` and `github-publication`
specifications now describe the clients as they are: unattended landing is
refused and only an exact external merge is recovered, source closure is
observe-only, and candidate upload is a defined scenario. The stale scenarios
`kiln.github.landing.merge.scenario.landed` and
`kiln.github.source_status.scenario.close` left the accepted specifications and
were replaced by `kiln.github.landing.merge.scenario.recovered` and
`kiln.github.source_status.scenario.open`. GitHub adapter maintainers own them.
No client behaviour changed.

## Exercised checks

| Check | Observed result |
| --- | --- |
| `cairn gate proposal\|design\|tasks retire-removed-github-mutations ...` | PASS before the specifications were synced, and PASS after the tasks were ticked |
| `cargo test --locked -p kiln-adapter-github --lib -- landing source_status publication` | 32 passed |
| Scoped `rustfmt --check --edition 2024` on `landing.rs` and `source_status.rs` | PASS; only comments changed |
| `cairn sync retire-removed-github-mutations ... --execute` | Three `modified` operations merged into the accepted specifications; nothing was archived |
| `cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | Valid, 0 issues |
| `cairn traceability coverage ... --profile kiln-active-development --json` | 1,974 requirements, all referenced, none missing. One dangling marker remains: `kiln.dashboard.campaign.projection` |
| `cairn traceability coverage ... --json` (accepted profile) | 1,904 requirements, all referenced; the remaining dangling markers belong to changes not yet synced |

Verification markers were added only where existing tests already exercise the
scenario:

- `reconciles_external_exact_merge_and_advanced_target_without_mutation`
  recovers an external exact merge with GET requests only;
- `refuses_open_source_without_racing_a_patch` refuses an open issue after one
  GET request;
- `uploads_and_reads_back_the_exact_candidate_objects_before_pr_transport`
  already carried the upload marker.

`docs/implementation-runbook.md` now references the new scenario IDs.

## Boundaries

- The remaining dangling `kiln.dashboard.campaign.projection` marker belongs to
  the selected-campaign change. Its delta is written as a full specification
  rather than `ADDED`/`MODIFIED` sections, so it cannot sync. Adding its root to
  the active profile exposes about 18 unreferenced campaign IDs. That rewrite is
  separate work.
- GitHub landing (#48) and conditional source mutation stay unqualified; nothing
  was reintroduced.
