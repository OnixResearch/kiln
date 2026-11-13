## Why

The operator wants examples for every Kiln website feature and view so the console can be explored without configuring providers or using operational data. The existing dashboard preview covers the separate offline renderer, not the live console's 21 destinations and eight run-detail tabs. Empty or unconfigured pages do not provide an exploration path.

## What Changes

- Add a loopback-only `kiln-web-service` exploration example using the real console assets, Engine, work/review workspaces and source/receipt/history browsers over newly created disposable local stores.
- Seed coherent examples for every registered destination and run-detail tab: CI outcomes, queue, logs, artifacts, test reports, comparison, worker capacity, issues and planning objects, inbox and triage, saved views/boards, exact-revision review and source, Patchgate receipts, raw collaboration history, feeds, automation, operations, evidence and settings.
- Provide a browsable example index with direct deep links, what to try and explicit simulation/non-claim boundaries. Keep a visible example label in the console.
- Simulate workflow observations only in the example. Never execute source commands, contact remote providers, reuse installed credentials/state, or change production authentication or routing. Local work/review commands operate only on disposable example repositories.
- Document the launch/reset procedure, feature coverage, owner and verification evidence.
- Package the standalone example and its supervised user unit separately from the authenticated web service; install and activate it only on explicit operator request.

## Impact

- **Consumer:** operators and contributors exploring Kiln's website.
- **Immediate outcome:** one local command opens a populated console with an example for every destination and nested feature family.
- **Durable capability:** a maintained, repeatable exploration workspace built on real production reducers and local adapters, not a second UI or silent mock fallback.
- **Owner:** Kiln web-service/dashboard maintainers.
- **Files:** `crates/kiln-web-service/examples/explore.rs`, its `examples/explore/` modules, focused example behavior checks where necessary, the dedicated flake package and user-unit template, README/operator documentation, changelog and this package.
- **Verification:** planning gates before implementation; compile and launch the example; actual browser coverage of destinations, run tabs, representative local mutations, deep links and narrow layout; focused existing checks and Cairn validation.
- **Authority:** the subsequent explicit user request to commit and deploy authorizes a local jj commit and installation/activation of the separate loopback example user service. Existing authenticated service configuration, credentials, routing and durable state remain unchanged. No source publication, remote signing, pushes, synchronization, release acceptance or lifecycle archive.

## Out of Scope

No new production CI semantics, production demo mode, remote-provider activation, hosted billing/payment effects, hostile-execution qualification or source-hosting feature. Unsupported external effects remain explicitly unavailable, never reported as successful execution.
