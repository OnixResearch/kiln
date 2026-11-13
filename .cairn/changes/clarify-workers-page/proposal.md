## Why

The operator says the Workers page works but its UI/UX is bad. The live page shows fifteen configured targets. Its large filter panel and six overlapping counters push the inventory below the fold. Each narrow target card repeats a provider identifier, a target hash, slot counts and an unknown-heartbeat sentence. Selection opens another wall of identities and counters before the execution and assigned projects a reader needs. The empty inspector wastes the wider column. Search requires an Apply action and every filter application redraws the page.

The response describes workflow targets and project admission slots, not physical machines. The redesign must not turn free project slots into online or healthy workers. The existing API can also return online/offline heartbeat observations; those must not be lost or mistaken for current free capacity when expired.

## What Changes

- Give the page a compact capacity readout and a full-width target register rather than a narrow card list beside an empty inspector. Target names lead; slot state, heartbeat observation, queued runs and latest execution can be scanned together.
- Filter the already bounded inventory immediately by metadata and slot state, with counts, clear controls and debounced URL restoration. List selection stays exact and independent of filters.
- Open a selected target as a readable detail view: project name and state first, project-slot capacity and heartbeat facts, the latest execution, assigned projects and active-run links, then a disclosure for exact binding/snapshot identities. All targets returns to the selected row. A labelled Browse targets disclosure retains the register and filters while inspecting a target.
- Retain unknown, missing, expired and unavailable states explicitly. Reported heartbeat counts are distinct from admission slots; an expired observation cannot establish currently available worker capacity. All run links use exact retained run IDs.
- Put the page in `workers.js` and `workers.css`, following the Source and receipts module convention. Remove the obsolete renderer, static filter markup and worker-specific CSS from the shell. The module's status vocabulary and existing inventory validation serve both the page and the Overview caller.
- Modify `kiln.dashboard.workers.view`; leave inventory, identity, admission and heartbeat backend behavior unchanged.

## Impact

- **Consumer:** operators and viewers inspecting configured execution targets in the console.
- **Immediate outcome:** scan the target fleet and recent execution, find a target without decoding hashes, and inspect its work without confusing slots with host health.
- **Durable capability:** one Workers presentation module, one shared worker-state vocabulary, pure selection/ordering/heartbeat presentation helpers and repeatable browser observations.
- **Owner:** Kiln web-service/dashboard maintainers.
- **Files:** `crates/kiln-web-service/src/{workers.js,workers.css,web.html,web.js,web.css,server.rs,ui.js}`, `tests/{live-console-ui.test.mjs,kiln-identity.test.mjs}`, `cairn-policy/development-coverage.ncl`, operator documentation, changelog and this change package.
- **Verification:** focused Node behavioral tests; crate build/tests for the embedded assets; Rust formatting/Clippy for the small route change; browser review of real working-tree assets over the read-only live inventory plus bounded synthetic edge cases; Cairn validation and active-delta traceability.
- **Authority:** working-copy implementation and verification only. No worker operation, enrollment, job dispatch, credential or configuration change, commit, publication, deployment, specification synchronization or archive.

## Out of Scope

No API/schema, capacity policy, scheduling, heartbeat production, provider effect, runtime repair or worker-management feature. In particular this does not repair the existing inactive Lattice service. No new font, icon or remote dependency.
