## Why

The website now has maintained interactive examples, but Kiln's existing Bombadil rails cover the terminal dashboard and the separate static HTML report, not the live console. The operator explicitly requested adding and using antithesishq/Bombadil. Reusing the existing exact pin avoids a second testing cohort and makes the live website's navigation and state invariants repeatable.

## What Changes

- Reuse the existing Bombadil revision `832faf4d953e2c56d1d901aca1bb2c3c3170221d` as a separately executed tool; do not change the pin, lock or production dependencies.
- Add browser properties for the current example workspace: scoped navigation, meaningful index search, exact selected-run/tab restoration, visible readiness and narrow layout, with the standard HTTP/JavaScript error properties.
- Generate property context from the example's admitted native catalog/session APIs. Drive real browser controls and public URLs rather than application internals or mocked responses.
- Add a repeatable bounded runner that starts and cleans up an owned disposable example by default, or attaches read-only to an explicitly selected example URL. Preserve traces, context, commands, tool version and outcomes.
- Expose the pinned tool and website runner through the existing flake conventions and a dedicated browser-testing development environment.
- Run the new rail against an isolated real workspace and the deployed private-tailnet examples; retain observed results and diagnose any violations rather than weakening the properties.

## Impact

- **Consumer:** operators and contributors checking the current Kiln website.
- **Immediate outcome:** actual Bombadil campaigns exercise the live-console examples and produce inspectable evidence.
- **Durable capability:** one maintained property specification and owned-lifecycle runner, using the already reviewed tool cohort.
- **Owner:** Kiln web-service/dashboard maintainers; operators own explicit attach URLs, credentials and private-CA inputs.
- **Files:** `checks/bombadil/`, a runner under `scripts/`, flake tool/app/shell exports, launch/coverage documentation, changelog and this change package.
- **Verification:** planning gates before implementation, actual native tool invocation and trace review, positive/negative target admission, owned-process teardown, focused lifecycle/coverage checks and actual website observation where a violation requires diagnosis.
- **Authority:** local implementation and tool execution over disposable examples and read-only access to the already authorized tailnet example origin. No live CI mutations, credential changes, provider execution, source publication, release acceptance or archive.

## Out of Scope

No Bombadil upgrade, second dependency pin, production fuzzing, automatic account enrollment, real source-command execution, broad provider verification, guaranteed replay across reset volatile example identities, or exhaustive coverage claim from a bounded campaign.
