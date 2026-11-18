## Context

The live console reads its 21 destinations from `KilnUi.destinations` and has eight run-detail tabs. It composes Engine, signed local work history, SQLite-backed exact-revision review, and source/receipt/raw-history browsers. Its existing offline dashboard preview is a different product surface. Reusing the native APIs preserves admission, exact identity, replay and permission semantics while keeping the example separate from deployed service authority.

## Goals and Success Contract

Every console destination has populated, linked example data or a meaningful example of its state-only controls. Each run-detail tab can be explored with suitable retained data. Core feature families include successful/failed/partial/running/queued/unknown/cancelled runs, logs and failure navigation, artifact/report downloads, comparisons, project history and worker capacity; issue discussion/relationships and linked CI; planning projects/milestones/cycles/initiatives; inbox, requests/triage and private/shared saved views/boards; exact-revision changes, diffs, comments, approvals/checklists and review status; source trees/history/file/diff views; receipts; collaboration history; feed reading and subscriptions; automation and operations; settings, guide, palette, themes and density.

False completion includes a navigation-only index with empty pages, fabricated CI success presented as real execution, data outside the disposable root, commands routed to real providers, or silently substituting sample data when a live API fails.

## Decisions

### Decision: A standalone example, not a production bypass

`cargo run --locked -p kiln-web-service --example explore -- [--port PORT]` creates one fresh private temporary root, binds IPv4 loopback only, initializes native stores and serves the current working-tree assets. Production authentication, service routes and profiles remain unchanged. The root is owned by the process and removed on ordinary exit. Restart is reset. The example does not accept an existing state directory or provider profile.

A visible synthetic-example notice and index link accompany every console page. The index names each page and deeper scenario, what to try and limitations. It reads the destination register rather than maintaining an independent destination list. Named sample links come from the seed modules and use exact returned identities.

### Decision: Real local state with an explicitly synthetic workflow provider

The CI module implements a bounded in-process demonstration `WorkflowPort`, supplies admitted observations and log chunks, and records worker reports/artifacts/test data through the real Engine APIs. It never starts a process, performs network I/O, or uses a real workflow transport. Terminal outcomes and long-lived running/queued/unknown examples are intentional scenarios. User-triggered rerun/cancel/retry controls affect only this simulator and the local journal. Examples and provider references state that execution is simulated.

Work and review data are created through their production command APIs against fresh disposable signed Git/SQLite stores. Source examples use disposable local Git objects and refs. Receipt files explicitly identify synthetic producer observations. Raw collaboration examples use newly created local COB histories, not imported operator repositories. Feed/automation demonstrations use bounded retained data and local-only handling; no external subscription fetch, webhook delivery or notification transmission is authorized.

### Decision: Preserve boundaries in the shell

The example routes requests to the same public APIs used by the production shell. It serves only an explicit static-asset allowlist, rejects foreign Host/Origin values and unsupported request framing, bounds request bytes and deadlines, and never serves the temporary filesystem. A synthetic local operator session is limited to the loopback example. Unknown routes fail explicitly. No permissive empty JSON fallback.

### Decision: Operator-authorized deployment preserves the separate boundary

The dedicated `website-exploration` Nix package builds only the `explore` example, installs it as `kiln-website-exploration` and includes a user-unit template with an immutable executable path. On an explicit commit/deploy request, install that package and supervise the loopback listener at port 18787 as `kiln-website-exploration.service`. Each start still creates fresh private disposable stores. Do not replace or restart the authenticated `kiln-web-service.service`, expose the synthetic operator through its HTTPS origins, or modify its profiles, credentials or durable state. The operator owns activation and the web-service/dashboard maintainers own packaging and seeds.

### Decision: Optional tailnet exposure is a separate registered-user origin

When explicitly requested, render a maintained Nginx HTTP-context fragment for a dedicated HTTPS origin on port 9444, binding only the declared Tailscale IPv4/IPv6 addresses. Reuse the existing certificate/private CA and `proxy-auth` resolver with the actual TCP peer socket; only existing registered user-ID/login bindings are admitted. Keep the original 9443 server and its Host/Origin maps unchanged. The example's own exact Host/Origin maps translate only its public origin to `http://127.0.0.1:18787`; missing-Origin writes remain rejected by the example. Clear browser authorization, cookies and forwarded identity/credential headers before proxying to the synthetic workspace. Send the private proxy credential only to the resolver, never to the example backend. Keep Tailscale Serve and Funnel state unchanged. Validate before graceful Nginx reload, retain a private rollback copy, and verify CA-checked access from an owned tailnet peer plus actual browser reads/mutations at the new origin. Clients must already trust the same private CA as the existing Kiln site.

Seed modules own distinct files: `ci.rs` owns Engine/simulator/run and capacity examples; `work.rs` owns signed work/planning/inbox/triage/views; `review.rs` owns code review/source/receipts/raw collaboration. The entrypoint owns HTTP composition, index, labeling, feeds and integration. Shared helpers provide exact response admission and a simple `Example { title, href, detail }` index record.

## Verification Strategy

Validate proposal, design and tasks gates before implementation. Compile the example with the repository's native prerequisites. Launch the actual program and inspect the index and all console destinations in Chromium. Check the eight run tabs, representative exact detail links, logs, report/artifact downloads and safe local mutations, URL/history restoration, light/dark appearance and a mobile-width layout. Fail on missing routes, unhandled browser errors or falsely successful unsupported actions. Exercise foreign-origin and path-traversal rejection alongside valid requests.

Run focused existing Node behavioral rails and the web-service crate checks proportional to the changed example. New permanent tests cover only uncertain example-specific safety boundaries or state transitions, never source-text wiring or copies. Record exact exercised commands, browser evidence and unexercised external effects. Update launch and coverage documentation after smoke proof. Run Cairn package validation and policy freshness. No accepted-spec synchronization, archive, commit or deployment is implied.

## Risks and Trade-offs

Synthetic execution makes UI exploration repeatable but is not worker, provider or CI verification. Native local stores can be modified through the example; isolation and an always-visible label prevent confusion with production. The explicit shell repeats a small amount of HTTP composition instead of opening a production authentication bypass. Future features must add coherent seeds and index scenarios; the index's destination-register read makes missing view coverage visible.
