## Context

At 1024px, the original header is 945px wide but uses tracks of 390.422px, 144px and 400.391px. Both outer tracks have max-content minima, and the account item cannot shrink. The document reaches 1071px. Removing only the location minimum is not a fix: the earlier browser experiment collapsed that track to zero. The existing Bombadil property is the regression oracle and must not be relaxed. The last storage observation for this request shows about 5GiB free on the shared pool and 47GiB available on the secure root-owned `/run` tmpfs.

## Decisions

### Decision: Preserve useful controls while constraining layout

Keep a visible, nonzero location region and a reachable account control. Use existing CSS layout, ellipsis and mobile patterns to let search and labels yield space before the document overflows. Preserve full accessible names and account/menu behavior, including pinned-rail layouts. Do not hide document overflow or remove content to make the property pass.

### Decision: Separate host policy from bounded build invocation

Read configured GC settings and their source without mutating host configuration. Every Nix command uses `--option min-free 0`, including root registration and inspection. Use one local build with secure memory-backed scratch and retained temporary GC roots; avoid shared cleanup and the unrelated invalid remote-builder route. The observed larger automatic request is investigated as host policy, not silently treated as covered by the earlier bounded cleanup approval.

### Decision: Exercise real rebuilt assets and unchanged properties

Rebuild the example because CSS is embedded in the Rust product. Verify actual DOM geometry and account/search/menu reachability at 390, 768, 1024 and 1440px in the pinned browser. Run the unmodified property specification against a fresh owned example and the existing deployed example in explicit read-only attach mode. A deployed run describes its existing cohort, not deployment of the local CSS fix. Keep native outcomes and diagnostics honest; no sync, archive, publication or release action is authorized.

## Risks / Trade-offs

- Over-aggressive shrinking can erase the breadcrumb or account affordance; measure their usable bounds and operate the controls, not only document width.
- The shared pool is constrained and host GC is independently configured; build scratch and per-invocation GC suppression must be explicit, with no global policy edits or further cleanup implied.
- Example IDs are volatile. Reproduce through the native scenario link rather than copying an old object identity, and do not promise replay across resets.
- A passing bounded campaign is not exhaustive coverage. Record each target/cohort, trace, violation, teardown and managed-service observation.
