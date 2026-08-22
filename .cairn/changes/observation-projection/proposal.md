# Add a read-only observation projection core

## Why

Kiln owns CI meaning, but operators have no way to observe committed Kiln state directly. The existing OpenTelemetry and CDEvents adapters are interchange projections, not operator surfaces. Kiln has no logging framework and no transport, and every view of a running system must be rebuilt from the same canonical event journal.

An observation projection gives operators bounded, rebuildable, read-only views of admitted CI state. It is the first rung of a larger surface: later HTTP and iroh shells will serve these views. This change builds the pure projection core only. It adds no transport, no runtime, and no new external dependency.

## What Changes

- Add a pure `kiln-observe-core` crate with a bounded view vocabulary: state summary, paged run list, run detail, attempt detail, admitted event-tail window, and admitted log-range window.
- Build every view through `project_observation(state, request, window) -> ObservationView` under an admitted bounded profile.
- Register every view with a `kiln.observe.view.v1` BLAKE3 identity bound to the exact source state revision and window.
- Enforce named bounds: page limits, cursor windows, value and attribute caps, unknown-target rejection, and redaction before a view is returned.
- Keep observation strictly read-only: projection never mutates state and never requests effects.
- Add typed Nickel observation profiles and deterministic positive and negative fixtures.
- Add maintained conformance and non-claim tests.

## Dependencies

This change depends on the accepted `ci-domain`, `durable-operations`, and `task-operations` contracts for state, runs, attempts, outcomes, products, and metrics. It consumes the accepted `sqlite-event-store-adapter` event records and the reference standalone log store windows as explicit admitted shell inputs. It does not depend on any network or provider adapter.

## Impact

- **Adapters**: adds `kiln-observe-core`; no existing crate changes behavior.
- **Core**: `kiln-core` state types are consumed as immutable inputs; no `kiln-core` surface change.
- **Configuration**: adds typed Nickel observation profiles, view bounds, redaction, and non-claims.
- **Testing**: covers deterministic rebuild, read-only enforcement, unknown targets, page and cursor bounds, redaction, missing negative fixtures, and authority promotion.

## Out of Scope

- HTTP or iroh transport shells (later changes).
- Control endpoints (submit-command intake is a later change; designed for, not included).
- otel/cdevents interchange changes, runtime logging, dashboards, or UI.
