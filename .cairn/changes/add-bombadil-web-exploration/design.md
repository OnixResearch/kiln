## Context

Kiln already pins Bombadil at revision `832faf4d953e2c56d1d901aca1bb2c3c3170221d` and has terminal/static-report specifications under `checks/bombadil/`. The current console has a separate disposable `explore` program and a registered-user tailnet proxy. The new rail must exercise that real console without turning a testing request into production mutation authority.

## Goals and Success Contract

A maintainer can launch a bounded property-based browser run with one repository-owned command. The default target is a newly owned disposable example process. An explicit URL attaches only after the example catalog/session admission checks and never transfers ownership of that service to the runner. The properties observe real DOM state and generated public URL selections; generated actions are read-only, same-origin browser interactions. Tool output, trace, context, command and outcome remain available for inspection. A normal tool exit without evaluated useful properties is not sufficient evidence. HTTP/JavaScript failures or semantic property violations must be diagnosed and reported, not suppressed by changing expectations to match a bug.

False completion includes merely downloading Bombadil, rerunning only the old static report rail, testing a mock page instead of the actual console, exporting only no-op properties, silently attaching to live CI, or reporting exhaustive coverage from elapsed time or a zero exit alone.

## Decisions

### Decision: Reuse the exact separately executed tool

Use `bombadilPackageFor` and the existing input. Export tool/runner apps and a dedicated browser-testing shell without adding Bombadil to core crates or forcing the browser into the default Rust shell. Preserve the existing terminal and offline HTML rails. Do not edit `flake.lock` manually or advance the dependency.

### Decision: Native context and read-only browser actions

The runner reads `/examples.json` and `/api/session` from one explicitly admitted origin with bounded reads and normal TLS/hostname validation. It requires the native example catalog and synthetic local session before launching a browser. It stages `web-exploration-context.json` with `{ origin, examples }` beside a copy of the live-console specification. Context is generated runtime input, not a hand-authored second fixture convention.

The specification derives candidate public routes from the catalog and observes public DOM controls. It checks index filter semantics, selected run/tab agreement with the public URL after loading, finite loading, same-origin scope and viewport usability, alongside upstream error properties. Its generators may navigate, reload, traverse safe history, search/filter and adjust viewport/appearance. They must not submit forms or invoke run, issue, review, subscription, session-clear or other server mutations, nor call private application functions or export unrestricted default click/input generators.

### Decision: Owned imperative lifecycle and retained evidence

The Python runner is the thin shell for command resolution, files, HTTP, processes, deadlines and reports. Pure helpers decide target/context admission and argument shape. By default it starts the selected packaged example with port 0, discovers the printed URL and sends a normal termination signal in `finally`; escalation and cleanup failure are explicit. Attach mode never stops or restarts the supplied service. Reject output overwrite rather than removing unrelated artifacts.

Invoke the native `bombadil browser test` command with headless operation, an explicit time limit, exit-on-violation and an output path. Use `CHROME` for the selected browser. Preserve the native exit outcome (including violation exit 2) and trace rather than replacing the fuzzer with a scripted tour. Record actual tool version and commands. Browser options are process-local; no system trust or operator browser profile changes. TLS admission must validate the private CA and hostname before an attached private-CA browser campaign; any process-local certificate handling used by Chromium must be explicit in evidence.

Child subprocess environments remove ambient proxy and dynamic-loader override variables. A development shell must not substitute libraries from another cohort into the pinned example, tool or browser; the report records removed loader-variable names without copying their values.

### Decision: Scope replay and outcomes honestly

Bombadil traces can be inspected and best-effort reproduced using its native CLI. The context, starting origin and browser/tool options must be retained. A reset example generates volatile identities, so the runner must not promise identical replay against a different fresh cohort. Deployed checks remain read-only and leave the existing example/CI processes and ingress unchanged. Any actual website defect found is separately diagnosed and verified through the narrowest reproducer.

## Verification Strategy

Validate proposal/design/tasks before implementation. Obtain the existing pinned tool and inspect its actual CLI. Integrate the specification and runner, then run the tool once on a fresh real example and once against the deployed private-tailnet example with CA-checked admission. Inspect native traces, actions and property outcomes to distinguish useful evaluation from initialization failure or vacuity. Exercise a refused non-example target and verify that a default-owned example process exits cleanly while attach mode preserves managed service PIDs. Keep all effects within the owned example or read-only origin; run focused lifecycle/traceability checks and document exact commands, maintenance ownership and non-claims after smoke proof.

## Risks and Trade-offs

Live rendering is asynchronous, so readiness and selected-detail properties must distinguish visible loading from a settled wrong selection. Random actions cannot establish exhaustive coverage; report the actual observed campaign. Browser traces can be large and can contain page data, so use only synthetic examples and keep artifacts in the selected output directory. A private-CA test must not become a global browser/TLS bypass. The existing Bombadil/browser cohort may have its own defects; distinguish those from Kiln failures using retained evidence rather than silently swapping versions.
