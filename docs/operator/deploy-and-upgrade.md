# Deploy, workspace configuration, and live web service

This runbook covers single-tenant live web service deployment (`kiln-web-service`), the unified workspace manifest (`kiln.json`), TLS/authentication/tailnet access, console navigation and GPUI design tokens, supervised deployment, the internal developer loop, and optional interactive change review.

For the operator runbook index, see [the operator guide](../operator.md).

## Workspace manifest (`kiln.json`)

The unified Factory workspace manifest (`config/factory-workspace.ncl`, schema `kiln.factory-workspace.v1`) consolidates `profile`, `access`, `sources`, `fleet`, and optional `ci`, `review`, `cairn`, `observations`, and `repository_roots` paths in one JSON file with relative paths resolved from the config directory:

```console
kiln init --repo /path/to/repo --out /tmp/my-kiln
kiln doctor --config /tmp/my-kiln/kiln.json
kiln check --config /tmp/my-kiln/kiln.json
kiln serve --config /tmp/my-kiln/kiln.json
```

Explicit CLI flags (`--profile`, `--access`, `--fleet`, `--repository-root`, and so on) override or extend `--config` values cleanly while preserving existing flag-based invocations.

## Single-tenant live web service

<!-- r[related kiln.web.evidence] r[related kiln.web.deploy.evidence] r[related kiln.web.deploy.install] -->

`kiln-web-service` composes existing Kiln core, Git snapshot and Lattice workflow
ports into a dedicated HTTPS application. The current consumer is Kiln self-CI;
Kiln application/dashboard and provider-adapter maintainers own this composition
and its repeatability rail. The existing `kiln-dashboard` HTML remains a separate
offline, read-only report.

**Current qualification:** the corrected immutable Lattice executable passes the
unchanged strict native exercise: source-bound `Success`, intentional
`DomainFailure`, and `Cancelled` after the actual command starts. Restart keeps
the original execution without redispatch; logs, artifact bytes/hash,
authentication, TLS and exact control checks pass. The measured executable,
source and profile identities and resource retirement are retained in
[the native receipt](../../.cairn/archive/1970-01-01-prepare-web-service-deployment/evidence/native-corrected/verification.json).
The earlier cancellation misclassification and compiler failures remain
historical evidence, not a verdict on this corrected cohort. Lattice's broader
repository checks and Kiln's separate readiness checks are not all green; this
local qualification grants no production activation or lifecycle acceptance.

The packaged systemd VM rail and supported Onix/NixOS assembly pass for the
frozen inputs retained in deployment evidence. Kiln's complete flake check also
passes. These checks do not exercise the corrected native cancellation cohort
or override the separate failing Cairn release-readiness verdict.

### Install reviewed profiles and authority

Use `config/web-service.ncl` and `config/web-worker.ncl` as the closed typed input
builders. Export operator-reviewed Nickel expressions that apply those builders
to real installed inputs; conformance fixtures are not deployment configuration.
The independently installable `packages.${system}.web-service` contains the binary
and supervised unit; `apps.${system}.kiln-web-service` selects that package.
The default workspace package also contains the binary.

- The service profile declares one registered HTTPS origin/listen address,
  certificate/private-key paths, separate operator/worker token files, an owned
  state directory, allowlisted local Git repositories/references and reviewed
  workflow profiles. Browser enrollment selects only this allowlist; it cannot
  submit paths, commands or replacement providers.
- Token files contain exactly 64 hexadecimal bytes without a newline. Use
  independent random tokens in owned private regular files, mode `0400` or `0600`,
  outside source and worker checkout roots. The HTTPS key is likewise private. Install a
  valid server certificate chain and an explicit worker CA; do not bypass TLS
  validation in a deployed worker.
- The `kiln.web-worker.v2` profile binds the service origin, project, CA/token-file
  references, canonical Git `source_repository` identity, worker-local
  `repository_path`, source snapshot bounds, an owned checkout root, explicit
  executable/argv and environment for each step, and declared relative regular-file
  artifacts. Commands receive no ambient environment or service credential.
  Reviewed commands still retain their worker account's authority: this is not a sandbox.
- Workflow, HTTP, state, log and artifact bounds are positive and explicit.
  Lattice's native serve connection limit counts lifetime exchanges, not just
  concurrent clients; provision a finite budget for polling and worker uploads.
  Exhaustion remains provider uncertainty, never successful completion.

With the reviewed expression and output paths supplied by the operator:

```console
nickel export --format json --output "$KILN_WORKER_PROFILE" "$KILN_WORKER_NCL"
b3sum "$KILN_WORKER_PROFILE"
nickel export --format json --output "$KILN_SERVICE_PROFILE" "$KILN_SERVICE_NCL"
kiln-web-service reviewed-request --profile "$KILN_SERVICE_PROFILE" --project "$KILN_PROJECT"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE"
kiln-web-service serve --profile "$KILN_SERVICE_PROFILE"
```

Retain the original worker JSON and its exact BLAKE3; equivalent reserialization
does not preserve that identity. Register the reviewed request's exact profile,
graph, node mapping and worker operation identity with the existing Lattice
`route_exact`/`shell_environment` handler. Its saved job invokes:

```console
kiln-web-service job --profile "$KILN_WORKER_PROFILE" --identity "$KILN_WORKER_BLAKE3"
```

Lattice supplies the exact `LATTICE_SOURCE_PROVIDER`,
`LATTICE_SOURCE_REPOSITORY` and `LATTICE_SOURCE_REVISION`. The helper verifies that
source context, admits the exact Git snapshot, and executes in an owned temporary
checkout rather than the mutable worktree. Follow the existing
[workflow application contract](../lattice-workflow.md) for native handler setup;
profile generation does not install or start it.

### Explore the website without providers

<!-- r[impl kiln.web.exploration.scenario.repeat] -->
The maintained exploration workspace starts the real console over fresh private
temporary stores. It needs no installed service profile, bearer credential,
source checkout, Lattice process or remote account:

```console
nix develop path:. -c cargo run --locked -p kiln-web-service --example explore -- --port 18787
```

Open <http://127.0.0.1:18787/examples>. The index reads the console's own
destination register, links every view and names deeper feature scenarios with
what to try. A visible **Synthetic examples** link and disposable-workspace
notice remain in the console. The port is IPv4 loopback only; choose another
`--port` if it is occupied (`0` selects a free port and prints its URL). Without
the flag, the example uses port 8787. It cannot accept an existing profile or
state directory. Ctrl-C/SIGTERM stops it and removes its stores; each restart
creates a clean workspace. Rebuild/restart after editing example seeds or the
embedded assets.

<!-- r[impl kiln.web.exploration.scenario.coverage] -->
| Surface | Examples and things to try |
| --- | --- |
| Guide and global navigation | Numbered destinations, command/search palette, keyboard reference and run-state legend. |
| Overview, CI projects and project history | Latest project failure, active and queued work, unenrolled sandbox, exact-source starts, retained history, streaks and duration trend. Planning projects remain separate. |
| Runs: pipeline and logs | Success, failed test, successful partial retry, running, queued, unknown, cancellation and an exact-commit untrusted approval hold; dependency graph, node selection, measured-step sample, stdout/stderr, search, first-error navigation and retained-log download. |
| Runs: artifacts, tests, reviews, comparison, activity and evidence | Downloadable text/report bytes, passed/failed/skipped JUnit cases, LCOV coverage, a native-shaped synthetic Patchgate receipt, gate/provenance documents, passing baseline comparison, canonical events and exact source/execution identities. |
| Workers | Busy/available/unenrolled targets, latest execution, project slots, a refreshed synthetic heartbeat and an expiring report. Heartbeat counts and project admission slots remain distinct; expired reports never establish current capacity. |
| Issues and inbox | Markdown, assignees, priorities, labels, parent/subissue, blockers, duplicates, snoozing, comments, mentions, reactions, updates and exact CI run links; unread assignments/mentions/watched items, a due reminder and favorites. |
| Planning | Initiative, project, milestone, cycle and team with a custom review/blocked workflow, dates and related issues. |
| Requests, saved views and boards | Incoming/accepted/declined requests with original customer/source retained; private/shared/favorite filters, status board with keyboard/drag move controls and a CI-project board retaining unlinked issues. |
| Code review | Requested review, multiple exact revisions/interdiff, line/split/structural comparison, historical blocking discussion, private draft and publication, viewed files, votes, unmet exact-CI requirements, dependency stack, WIP, submitted/abandoned changes, queue registration and self-declared agent authorship. |
| Cairn changes | Full-width List/Board, lane spread and filters over synthetic A→B (failed prerequisite and ready dependent with review wait), C (superseded historical attempt and stale binding) and completion-only D (awaiting work). Open a unit for its timeline/dependencies and follow campaign links; any inline answer/issue uses only the admitted local campaign inbox. |
| Source | Two disposable Git repositories, branches/tag, directories, highlighted Rust/Nickel/Markdown, exact file/line links, commit history, rename and binary-file changes. |
| Patchgate receipts and raw collaboration history | All nine receipt decisions with recorded reasons/paths/times/coverage; exact retained documents; issue, patch and Cairn-change raw histories with multiple local ref tips. |
| Feeds | Repository Atom/RSS and OPML publication; parsed illustrative upstream entries in observed/paused/pending/failing states; filtering, pause/resume/rename/remove, offline-catalog subscription and OPML import/export. Try `https://examples.invalid/new.atom`. |
| Activity, evidence, automation, operations and settings | Retained audit/event history and run chooser; locally signed synthetic intake, disabled interval/calendar schedules and intentionally withheld notification reservations; capacity/retention, appearance/density and session clearing. Reload reconnects the example identity. |

<!-- r[impl kiln.web.exploration.scenario.local] -->
CI state, work commands, review policy/provider operations and inspection use
their real local APIs. The workflow provider is an explicit in-process
simulator: seeded running/queued/unknown runs remain available for inspection;
newly dispatched interactive runs complete after bounded observations. Native
capability admission still applies (the pinned workflow contract supports
cancellation, not pause/resume). Local review submit changes only a disposable
target ref, and its CI facts come from exact retained **simulated** runs.

Feeds use the production bounded parsers/renderers with an explicit offline
catalog, not the network-fetch shell. Only named `https://examples.invalid/*.atom`
catalog URLs are admitted; arbitrary URLs fail rather than fetching or silently
substituting data. Notification work may be reserved in the real journal but is
never delivered. Neither source commands nor remote providers are executed.

This is UI rehearsal, not CI/provider verification, hostile-execution isolation,
source trust, review acceptance, release or deployment evidence. Raw history
does not verify signatures or reduce Cairn state. Native Jujutsu/Pijul intake,
external webhook delivery, real worker execution and optional hosted
account/payment services need their separately documented setups; they are not
silently emulated as successful operations. The production HTTPS listener and
its authentication/admission are unchanged.

Kiln web-service/dashboard maintainers own the seeds under
`crates/kiln-web-service/examples/explore/`, the `explore.rs` entrypoint and
future feature examples. Add exact returned-identity links and meaningful
detail data when adding a view. Focused checks are
`cargo test --locked -p kiln-web-service --example explore` and
`cargo clippy --locked -p kiln-web-service --example explore -- -D warnings`,
plus actual browser navigation, edits, downloads and narrow/dark presentation.

#### Stable exploration cases and capture

<!-- r[impl kiln.web.exploration.cases] -->
`/examples.json` carries stable `case_id`, versioned `recipe`, BLAKE3-derived
`recipe_revision`, source identity, presentation/interaction requirements and
resolved runtime subjects for named cases. Restarting creates new runtime IDs
for the same logical case. `GET /examples/cases/<case_id>?recipe=<revision>&workspace=<identity>`
resolves a case in the running workspace; an unknown case is 404, and a moved
recipe or another workspace's identity is 409. The examples index includes a
keyboard-operable case/theme/density/viewport selector with narrow and dark
presentation support.

Capture one case from a running explorer with the workspace identity and recipe
revision from `/examples.json`:

```console
python3 scripts/explore-capture.py --browser ABS_CHROMIUM \
  --origin http://127.0.0.1:PORT --case case.run.failed \
  --workspace WORKSPACE --recipe-revision RECIPE_REVISION --out-dir NEW_DIR \
  --viewport 390x844 --theme dark --density compact
```

The helper drives Chromium over its DevTools pipe with a disposable profile and
an exact emulated viewport. It writes `screenshot.png`, `semantic.json` and
`manifest.json` only after the document is complete, the main landmark is
present, every exact subject identity is in the document, no region is busy and
fonts are loaded; otherwise it refuses and names the unmet conditions. Count,
size and time limits are named constants. Compare two captures with
`node scripts/explore-compare.mjs LEFT/manifest.json RIGHT/manifest.json`. It
reports every case, recipe, theme, density, viewport, browser, readiness, font
and resource incompatibility and lists changed runtime subjects without
normalizing them away. Synthetic execution remains visibly synthetic and is not
CI, provider, review-acceptance or deployment evidence. The Kiln-to-Rats
migration comparison cohort remains blocked on #67/#68 and is not simulated.
The retained [exploration case evidence](../../.cairn/archive/1970-01-01-add-stable-exploration-cases/evidence/verification.json)
captures every named case from two clean starts with identical recipe
revisions, changed runtime subjects and no incompatibility, records the capture
refusals, and includes a Bombadil run that reached its time limit without a
property violation after the narrow-index repair.

#### Property-based browser exploration

<!-- r[impl kiln.web.exploration.browser_property_rail] -->
<!-- r[impl kiln.web.exploration.browser_property_rail.scenario.cohort] -->
Use the existing pinned, separately executed Bombadil tool against the real
console, not the offline report:

```console
nix run path:.#kiln-web-exploration-bombadil -- \
  --time-limit 90 --output-path /tmp/kiln-web-bombadil
```

The default starts its own packaged example on a free loopback port, admits its
native catalog and synthetic session, then runs headless browser exploration.
The runner terminates only its owned example and removes its private stores on
exit. The output directory must be absent or empty; existing traces are never
overwritten. `nix develop path:.#web-testing` supplies the same separately pinned
tool, Chromium, Python and example without enlarging the default Rust shell.

<!-- r[verify kiln.web.exploration.browser_property_rail.scenario.properties] -->
Properties check case/trim/zero-result index search, public page and exact-run/tab
selection after loading, a bounded loading deadline, same-origin navigation,
explicit appearance restoration and horizontal viewport fit, plus upstream
HTTP/JavaScript/console error properties. Generated interactions navigate native
catalog links and run tabs, search, reload, traverse safe history and change
viewport/appearance. They do not submit forms, clear a session, operate runs or
edit issues, reviews, feeds or any other server state. No application-private
function or unrestricted default click/input generator is used.

<!-- r[verify kiln.web.exploration.browser_property_rail.scenario.isolation] -->
Attach to the existing example only when explicitly selected:

```console
nix run path:.#kiln-web-exploration-bombadil -- \
  --url https://britton-desktop.bison-tailor.ts.net:9444 \
  --ca-certificate "$HOME/.config/kiln/web-service/credentials/ca.pem" \
  --time-limit 90 --output-path /tmp/kiln-web-bombadil-attached
```

Attach mode never stops or restarts that service. Admission uses normal CA and
hostname verification, bounded native JSON, no redirects and no ambient proxy
or credential fallback. A non-example CI origin is refused before browser
launch. Chromium must also trust the example certificate: the CA argument
configures admission, not the browser. If a test-only browser needs an exception,
explicitly supply `--browser-arg=--ignore-certificate-errors-spki-list=...` for
the reviewed certificate's base64 SHA-256 SPKI. This option affects only the
owned browser; it is recorded, and does not replace CA-checked admission or
authorize a global trust change. The retained receipt records the exact option
used for the private-tailnet exercise.

Child environments strip `LD_LIBRARY_PATH`, `LD_PRELOAD` and `LD_AUDIT` so a
development shell cannot replace the pinned executables' runtime libraries.
Removed variable names are recorded, not their values. No host environment,
loader configuration or sandbox setting is changed.

<!-- r[verify kiln.web.exploration.browser_property_rail.scenario.evidence] -->
Keep `report.json`, `bombadil.log`, the staged specification/native context and
`trace/` together. Native exit 0 is success, 2 is a property violation, and other
failures remain errors; inspect property snapshots and actual actions rather
than treating tool startup as coverage. Open the retained trace with:

```console
nix run path:.#bombadil -- browser inspect /tmp/kiln-web-bombadil/trace
```

Native `browser test --reproduce` is best-effort and requires the same surviving
origin/context. An owned origin is stopped at campaign end; a fresh example has
new identities, so identical replay across resets is not promised. Random
exploration is bounded, not exhaustive feature coverage or CI/provider,
production or release qualification. The [browser campaign receipt](../../.cairn/changes/add-bombadil-web-exploration/evidence/verification.json)
retains observed results and non-claims. Kiln web-service/dashboard maintainers
own `checks/bombadil/kiln-web-exploration.spec.ts`,
`scripts/web-exploration-bombadil.py` and the flake tool/runner entries.

<!-- r[verify kiln.web.topbar_containment] -->
<!-- r[verify kiln.web.topbar_containment.scenario.location] -->
<!-- r[verify kiln.web.topbar_containment.scenario.controls] -->
<!-- r[verify kiln.web.topbar_containment.scenario.evidence] -->
The long-location header regression is exercised through **Raw collaboration:
Example exact-source patch**, not a copied volatile object ID. The rebuilt
header keeps flexible outer tracks, intrinsic search width and elided labels;
it does not hide document overflow or collapse the location to zero. The
[cross-width UI receipt](../../.cairn/changes/fix-console-topbar-overflow/evidence/ui-verification.json)
records 390/768/1024/1440px geometry, desktop pinned navigation, usable account
menus, command-palette interactions, focus restoration and mobile target sizes.
The [campaign receipt](../../.cairn/changes/fix-console-topbar-overflow/evidence/verification.json)
keeps the unchanged Bombadil property outcomes. Observing the deployed example
does not deploy this local fix. Kiln web-service/dashboard maintainers own the
header and this regression check.

<!-- r[impl kiln.web.topbar_containment.scenario.host_safety] -->
<!-- r[verify kiln.web.topbar_containment.scenario.host_safety] -->
Before building on a constrained shared host, inspect configured GC thresholds
and current capacity. A manual `--max-freed` limit does **not** cap a separate
automatic collection triggered by the host's `min-free`/`max-free` policy.
For bounded verification work, apply `--option min-free 0` to every Nix
invocation, including `nix-store` root registrations. Use one build job and
secure root-owned runtime scratch when appropriate; retain temporary package
roots until the campaigns finish. Do not change host policy, perform shared
cleanup or restart managed services without separate authorization. The
[read-only host diagnosis](../../.cairn/changes/fix-console-topbar-overflow/evidence/host-safety.json)
records the exercised host's source-backed settings and capacity; that policy
remains operator-owned and unchanged.

#### Managed loopback examples

<!-- r[impl kiln.web.exploration.scenario.deploy] -->
The dedicated `website-exploration` flake package and
`kiln-website-exploration` app contain the same standalone example, not the
authenticated service. For an explicitly authorized managed local deployment,
build the selected frozen source revision, retain its output as `new_package`,
then install its separate user unit:

```sh
nix profile add "$new_package" --option min-free 0
systemctl --user link "$new_package/share/systemd/user/kiln-website-exploration.service"
systemctl --user enable --now kiln-website-exploration.service
```

The package's executable is `bin/kiln-website-exploration`; its user unit pins
the immutable output path and listens only at <http://127.0.0.1:18787/examples>.
Stop any manually launched exploration process on that port before activation.
Do not replace or restart `kiln-web-service.service`, modify its profiles or
route this synthetic operator through the existing HTTPS website. Managed
deployment does not make examples persistent: every service start creates fresh
private temporary stores. `systemctl --user restart kiln-website-exploration`
resets them; `systemctl --user disable --now kiln-website-exploration` stops the
workspace and disables automatic start with the user manager.

For an update, retain the previous package/unit, remove only the
`kiln-website-exploration` profile element before adding its replacement, then
relink/re-enable it with `systemctl --user enable --force "$new_package/share/systemd/user/kiln-website-exploration.service"`
and restart that unit. If adding the replacement fails, restore the previous
profile element before changing the
unit. Executable rollback restores the previous package/unit, not discarded
example edits. Never remove the independent `kiln-web-service` profile element.

The [managed example deployment receipt](../../.cairn/changes/add-website-exploration-examples/evidence/deployment.json)
binds implementation source `4418586c` to the installed package, loopback socket,
all 80 desktop scenarios and 29 narrow/dark view/tab checks, local cancellation
and artifact download, managed restart/reset, private-store cleanup and the
preserved authenticated service. Its four packaged release-profile tests passed.
It grants no production, provider-execution, publication or lifecycle acceptance
claim.

#### Optional private-tailnet access

<!-- r[impl kiln.web.exploration.scenario.tailnet] -->
The operated example index is
<https://britton-desktop.bison-tailor.ts.net:9444/examples>. This is a separate
origin from the authenticated CI workspace on port 9443. It uses the existing
Kiln private CA: client devices must trust that CA, just as for the existing
Kiln site. No client trust settings are changed automatically.

After explicit operator approval, render
`deployment/kiln-website-exploration-tailnet.conf.in` with the declared
`dns_name`, `tailnet_ipv4`, `tailnet_ipv6`, `certificate`, `private_key` and
`authorization_include` values. These are paths and public endpoint facts, not
credential bytes. The fragment requires the existing `$kiln_peer_socket` map
and private registered-user `proxy-auth` resolver. Store it as
`~/.config/kiln/web-service/website-exploration-tailnet.conf` and add one include
inside the existing Nginx `http` block. Preserve the original server and its
Host/Origin maps. Validate the full configuration with the installed Nginx
binary, prefix and `-c` arguments from `kiln-tailnet-proxy.service` before
gracefully reloading only its master:

```sh
systemctl --user kill --kill-whom=main --signal=HUP kiln-tailnet-proxy.service
```

The example origin binds only the declared tailnet addresses on port 9444.
Only existing registered untagged Tailscale identities are admitted; client
identity headers are not authority. Every admitted visitor shares the disposable
synthetic operator workspace, never live CI authority. Only this origin is
translated to `http://127.0.0.1:18787`; missing-Origin writes remain rejected.
Browser authorization, cookies and identity/proxy-key headers are cleared before
the example backend. The private proxy key goes only to the resolver. The
original HTTPS backend, application profiles, credentials, Tailscale Serve
routes and Funnel state are unchanged.

The [tailnet deployment receipt](../../.cairn/changes/add-website-exploration-examples/evidence/tailnet.json)
records CA/hostname-verified reads from the owned `aspen3-1` peer, all 21 browser
destinations and eight run tabs, narrow/dark review, a temporary subscription
write/removal, a byte/hash-verified artifact download, and negative Host/Origin
and identity-header checks. The existing CI health endpoint remained available.
The operator owns listener exposure, enrollment, certificate renewal and trust;
Kiln web-service/dashboard maintainers own the reusable template and examples.

Rollback removes only this fragment's include from the current configuration,
validates and gracefully reloads Nginx again. A private pre-change configuration
is retained in the receipt's rollback directory; do not restore it over later
operator edits. No backend restart, global profile rollback, Tailscale Serve
reset, credential rotation or live database rewind is part of this operation.


### Navigate the live workspace
<!-- r[related kiln.work.usability] -->

<!-- r[impl kiln.web.console_layout] -->
Navigation is a compact icon rail. Hovering it, or moving keyboard focus into it,
expands it over the page to show every label and the **Continuous
integration**, **Work**, **More work**, **Inspect** and **Administration**
groups; labels stay available to assistive technology while it is collapsed.
**Keep navigation expanded** at the bottom pins it open beside the content for
this page session (the URL is unchanged). Escape collapses an unpinned expansion
and returns focus. Group headings keep their place while the rail is collapsed
and fade in when it expands, so no destination moves under the pointer. The
active page is an inverted ink row. Rail destinations and run tabs are links, so
they open in a new tab and keep browser history; commands stay buttons.
Secondary groups open automatically for a selected deep link, and when the rail
scrolls the active page is scrolled into its view. Existing page URLs remain
valid; the Kiln brand returns to the Overview, the page a bare console URL
opens. The service boundary note (authenticated live service; actions affect
the configured execution provider; recorded evidence is not acceptance
authority) is in the account menu. Pages sit in a centred column under a large
page title.

<!-- r[impl kiln.web.wayfinding] -->
Every destination has a section number, as in a standards manual. The numbers
come from one register in `ui.js` (`KilnUi.destinations`), which the rail, the
page title block, the breadcrumb, the document title and the command palette
all read:

| Group | Destinations |
| --- | --- |
| 0 Start here | 0.0 Guide |
| 1 Continuous integration | 1.1 Overview, 1.2 Runs, 1.3 CI projects (a project's history shares 1.3), 1.4 Workers, 1.5 Runbooks |
| 2 Work | 2.1 Issues, 2.2 My inbox, 2.3 Planning, 2.4 Code review, 2.7 Cairn changes, 2.8 Fleet (and under **More work**: 2.5 Requests & triage, 2.6 Saved views) |
| 3 Inspect | 3.1 Source, 3.2 Patchgate receipts, 3.3 Activity, 3.4 Feeds, 3.5 Evidence |
| 4 Administration | 4.1 Automation, 4.2 Operations, 4.3 Raw collaboration history, 4.4 Settings |

The title block shows the number and group above the title, and the purpose
below it. The rail shows each number beside its label once the rail expands;
numbers are visual and stay out of each link's accessible name. In the command
palette (**Ctrl K**) every destination is a **Go to** command, so typing `4.2`
or `operations` finds Operations. The console admits exactly the register's
pages, so a new page is added there once.

**Guide** (0.0, in the rail footer and in Settings) is the console's index: a
first path for a new operator, every destination by number with its purpose,
how each run state reads (drawn with the real status indicators), the words
Kiln uses and the keyboard reference, including code review's shortcuts. It
reads the register, the status rule and the review shortcut table, so it cannot
describe a page or key the console lacks. An Overview with no project offers
**Open 1.3 CI projects** and **Read 0.0 Guide**.

<!-- r[impl kiln.web.visual_design] -->
<!-- r[impl kiln.identity.shared] -->
<!-- r[impl kiln.identity.graphics_standard] -->
The live console presents the Kiln graphics standard that it shares with the
offline catalog, the offline report and the hosted bridge page. It follows the
NASA Graphics Standards Manual (NHB 1430.2) and engineering-drawing practice:
paper panels framed in ink on a grey page in light appearance, inverted in
dark; an ink navigation spine in both; square corners; flat surfaces ruled by
hairlines; a Helvetica-class grotesque for reading text and a small, upper-case,
tracked monospace for technical labels (eyebrows, column heads, field labels,
fact terms), identifiers and numbers. Each view's single primary action is a
solid ink block (paper in dark appearance); other buttons are framed paper
cells, and selection is an inverted ink cell. Identity red (NASA red, PMS 179)
appears in three places only: the brand mark, the short index block opening
each page's title rule, and the request line. It never marks a status, an
action or a selection. Success, failure, running, queued, cancelled, unknown
and warning states keep distinct semantic colours, icons and text. Only
overlays (menus, dialogs, the command palette, the expanded rail) are raised,
by an unblurred offset shadow. The brand mark is a square of identity red with
an ink kiln door, drawn in CSS; `/favicon.svg` carries the same shape. Every
page has one title row over the ink title rule, with one quiet **Refresh** icon
button that refreshes and reconnects live observations.

The top bar holds the search/command trigger (**Ctrl K**), the running count, a
live-connection indicator and an account menu with the signed-in identity, an
**Appearance** switch (System, Light, Dark), **Appearance & settings** and
**Clear session** (sessions remain active across browser `pagehide`/sleep
transitions and clear only on explicit **Clear session** or rejected
authorization; bearer tokens stay in memory only). The switch and
**Settings → Appearance** write the same URL
state. **Density** changes spacing without shrinking mobile touch targets.
Menus and dialogs arrive with one short fade; page changes are immediate, and a
reduced-motion preference removes every transition and animation.

<!-- r[impl kiln.web.live_activity] -->
<!-- r[impl kiln.web.live_activity.scenario.count] -->
<!-- r[impl kiln.web.live_activity.scenario.stale] -->
While the last admitted live snapshot reports running runs, the top bar shows
**N running** on every page and the door of the brand mark turns amber. The
count links to the retained-run list filtered to pending runs, where each row
states running, queued or awaiting approval. Queued, awaiting-approval, unknown
and finished runs are not counted. The count and the lit door are hidden before
the first snapshot, while live observation is stale or disconnected, and after
the session is cleared. They read one snapshot; they are not a worker health
check.

<!-- r[impl kiln.web.request_feedback] -->
<!-- r[impl kiln.web.request_feedback.scenario.slow] -->
A read or mutation that stays in flight for more than a moment draws a moving
identity-red line on the lower edge of the top bar and marks the page busy for
assistive technology. The line clears when no such request remains, whether the
request succeeded, failed, timed out or was aborted. It states waiting only,
never progress or success. The live-observation poll and log following do not
show it.

The identity lives in `crates/kiln-dashboard/src/render/identity.css`, served to
the console at `/identity.css` and loaded first. Shared console components live
in `web.css`; `ui.js` (served at `/ui.js` and loaded before the other scripts)
provides `window.KilnUi` presentation helpers. `work.css`,
`work-interaction.css`, `code-review.css`, `project.css` and `source.css` build
on the same tokens. These assets are maintained by Kiln's web-service/dashboard
owners and request no external font or image.

<!-- r[impl kiln.web.design_system] -->
<!-- r[impl kiln.web.design_system.scenario.provenance] -->
<!-- r[impl kiln.web.design_system.scenario.local] -->
The console follows GPUI Kit's design system at the pinned revision
`944803984cf0ba3111861d6e1aa9d6265c109ff9` (NAR
`sha256-/w0H0zmgKPf+8aAFw3BOT8ifKl2xH+wdVKGEbSwXlRM=`). Every icon is a Lucide
icon from that revision, served from Kiln's own origin at `/icons.js` (loaded
before `/ui.js`); no icon, font or image is fetched from elsewhere. The reviewed
names live in `crates/kiln-web-service/src/icons.list`, one per line and sorted.
To add or change an icon, edit that list and run:

```sh
scripts/import-lucide-icons.sh
```

The script resolves the revision with `nix flake metadata`, refuses any revision
or NAR mismatch, an unsorted list, a name missing from the revision or an SVG
with unexpected attributes or elements, all before writing anything. It then
regenerates `icons.js` (data only: `window.KilnIcons` maps each name to element
descriptors, with the revision, NAR, list BLAKE3 and per-icon source BLAKE3 in
its header) and copies `LICENSE-LUCIDE` beside it. Repeated runs are
byte-identical. `KilnUi.icon(name)` builds the decorative SVG (`aria-hidden`,
not focusable, `currentColor` stroke) and throws for a name that is not in the
list; `KilnUi.statusIcon(status)` maps run and review states to icons, with
unrecognised states shown as unknown. Static markup uses
`<span class="icon" data-icon="name" aria-hidden="true"></span>` placeholders,
which `ui.js` replaces on load (`KilnUi.hydrateIcons(root)` does the same for
inserted markup), and `KilnUi.disclosureSummary(label, icon)` builds a
disclosure summary with its turning chevron.

Colour tokens use GPUI Kit's semantic theme names with dots written as hyphens.
The values are Kiln's palette, declared once in `identity.css` for every
surface. Each colour is one `light-dark(light, dark)` declaration, so the two
appearances cannot drift. A region that is dark in both appearances (the
navigation rail, the log panel) sets `color-scheme: dark` instead of restating
tokens. Former names were removed, not aliased:

| Former token | GPUI Kit token | CSS custom property |
| --- | --- | --- |
| `--background` | background | `--background` |
| `--text` | foreground | `--foreground` |
| `--muted` | muted.foreground | `--muted-foreground` |
| `--surface-soft` | muted.background; list.active.background; table.head.background | `--muted`; `--list-active` (current row); `--table-head` |
| `--surface` | secondary.background; popover.background | `--secondary` (controls, raised rows); `--popover` (menus, dialogs) |
| `--panel` | group_box.background | `--group-box` (panels, cards) |
| `--surface-hover` | accent.background; list.hover.background | `--accent` (hover); `--list-hover` (rows, navigation, menu items) |
| `--border` | border | `--border` |
| `--border-input` | input.border | `--input-border` |
| `--border-strong` | list.active.border; Kiln extension | `--list-active-border`; `--border-emphasis` |
| `--sidebar` | sidebar.background | `--sidebar` |
| `--focus` | ring | `--ring` |
| `--signal`, `--signal-strong`, `--signal-text` | primary.background, primary.hover.background, primary.foreground | `--primary` (ink), `--primary-hover`, `--primary-foreground` |
| `--olive` | link; progress.bar.background; syntax string | `--link`; `--progress-bar`; `--syntax-string` |
| `--accent` | syntax number | `--syntax-number` |
| `--status-success`, `--status-failure`, `--status-warning`, `--status-running` (and `-bg`) | success, danger, warning, info (and `.background`) | `--success`, `--danger`, `--warning`, `--info` (and `-background`) |
| `--status-queued`, `--status-cancelled`, `--status-unknown` (and `-bg`) | Kiln extensions | `--queued`, `--cancelled`, `--unknown` (and `-background`) |
| `--log-background`, `--log-text`, `--log-muted` | editor.background, editor.foreground; Kiln extension | `--editor-background`, `--editor-foreground`, `--editor-muted-foreground` |

Text on a state's `-background` uses the state colour itself, so no separate
`-foreground` exists for states. `--hero`, `--olive-soft` and `--accent-soft`
were unused by the live console and were removed. Spacing, type, radius, shadow,
motion and layout sizes keep their names; every radius tier is `0`, and
`--shadow-md` and `--shadow-overlay` are unblurred offsets. Kiln extensions
beyond the table are `--frame` (the ink line around panels, cards, controls and
overlays), `--syntax-<class>` for each server token class, `--code-selection`
for selected source lines and selected text, the label tokens `--label-font`
and `--label-tracking`, and the identity colours `--brand` (identity red),
`--brand-door` (the mark's ink door) and `--brand-live` (the lit door). The
offline catalog's `catalog.css`, which also styles the console shell, uses the
same names; no stylesheet maps one vocabulary onto another.

<!-- r[impl kiln.identity.contrast] -->
`tests/kiln-identity.test.mjs` holds the identity. Every declared text, status,
action, focus, boundary and syntax pair must meet its contrast floor in both
appearances: 4.5:1 for text, 3:1 for the focus ring, the input boundary, the
ink frame, a selected row's boundary, identity red against the rail and the
page, the mark's door against red, and the lit door against the idle door. Ink
action emphasis must stay apart from the success, failure, running and warning
colours, and identity red from the failure colour. No surface stylesheet may
declare a colour literal or use a token that nothing defines. The standard
check fails on identity red outside the mark, the title rule and the request
line, on a rounded corner or radius tier, and on a blurred shadow. After
changing a value in `identity.css` or a surface stylesheet, run:

```sh
node --test tests/kiln-identity.test.mjs
```

`light-dark()` needs Chromium 123, Firefox 120 or Safari 17.5.

<!-- r[impl kiln.web.readable_observations] -->
Outcomes appear as status pills with an icon and a human label (for example a
`DomainFailure` run reads **Failed**, `Success` reads **Passed**). Durations use
human units (`273357 ms` reads **4m 33s**) and timestamps read relative to now
(**2h ago**). The exact canonical outcome, recorded milliseconds and the local and
UTC instant stay available on hover and to assistive technology, and exact
identity disclosures keep full local/UTC timestamps. Timing a run cannot have
yet is named from its recorded phase: a running run's duration reads **In
progress**, a queued or awaiting-approval run's start and duration read **Not
started**, and an unfinished run's completion reads **Not finished**. Other
missing timing reads **Not recorded**, never zero or the current time. Byte
quantities read in binary units (**1.2 GiB**) with the exact byte count beside
them. Formatting never changes stored data.

<!-- r[impl kiln.web.page_consistency] -->
Every CI page leads with readable labels and keeps exact identities secondary:
run choices show the full project name under the status and time, worker targets
lead with their assigned projects (provider profile and a shortened target ID
below; the full ID is in the inspector), and activity entries lead with the
method, a shortened target and the decision, with the exact target beneath and
every recorded field under **Recorded fields**. Long boundary and non-claim
statements sit in labelled **About this view** (or **About …**) disclosures near
the page or section heading; short decision-relevant warnings stay inline. Empty
and unconfigured sections show an explicit state with guidance instead of a
blank area. Row actions such as **Logs**, **Browse runs** and run links share one
bordered action style and are at least 44px tall on touch layouts.

<!-- r[impl kiln.web.visual_design.scenario.lists] -->
The Overview project list, a project's run history and the code-review change
list are tables while their own region is wider than 48rem (768px). In a
narrower region, on a phone or portrait tablet or beside a second column on a
small desktop, each row stacks into lines, so a project name or a change title
is not squeezed into a narrow column. A status label is shown whole, and
**Logs** and **Pipeline** never wrap.

The operator-authorized [visual-design deployment receipt](../../.cairn/archive/1970-01-01-align-blacksmith-visual-design/evidence/deployment.json)
binds the published source, committed executable, unchanged profiles/state and
actual deployed desktop/mobile checks. It does not clear the separately retained
repository-wide traceability or release-readiness holds.

<!-- r[impl kiln.web.minimal_console] -->
Overview is a dashboard built only from the bounded overview, workers and
activity reads. **Project health** states how many projects are failing, a pass
rate over the latest run of each project with its denominator (passed, failed
and in-progress runs) and a legend; unknown, cancelled and run-less projects are
excluded from the rate and listed. Its retained-run count chips filter runs, and
**Active** opens busy execution targets. **Needs attention** shows one card per
failing project with a status pill, duration, relative finish time, short source
revision and direct **Logs** and **Pipeline** links. Logs opens the exact latest
execution, defaults to stderr for a failing project and clears stale run filters;
another stream remains selectable. **Recently passed** and **In progress &
unresolved** list the other projects with the same links. The side column shows
worker slots by status and the latest five activity entries. Each section loads,
empties and fails independently: an unavailable read is stated in its section and
never replaced by sample data. **Recorded duration trend** opens the detailed
observations. In Runs, **More filters** holds exact-source and sort order and
opens automatically when a URL restores non-default values. These views remain
bounded retained observations, not complete history or release authority.

The [minimal-console deployment receipt](../../.cairn/archive/1970-01-01-simplify-live-console/evidence/deployment.json)
binds the frozen source, installed executable, served assets and actual desktop/320px
workflows. All 19 retained run projections and 546 original configuration, work
and retained files were preserved; no source publication or lifecycle acceptance
was performed. Kiln's web-service/dashboard maintainers own these interactions.

Start with **New issue…**. Title, description, status and priority are visible;
ownership/labels, relationships, dates and CI/source links open in labelled
disclosures. Creation and editing still confirm exact fields before signed writes:
the confirmation names the record and the verb (for example **Create issue “…”?**
with **Create issue** and **Keep editing**) and lists the principal, object, expected
head and each changed field.
Each work destination offers its own primary action: requests in Triage, a saved
view in Saved views, and the selected planning type in Planning. **Create something
else** retains access to all supported types, including private attention
preferences. Planning categories distinguish projects, milestones, cycles,
initiatives and teams from CI project enrollment.

Search stays visible. Open **Filters & display** for type/status/assignee/project,
list/board layout, grouping, sorting, reset and saving the current view. Its summary
shows active filter count and board mode. Named views keep their existing URL
override precedence. An empty list gives contextual guidance; unavailable storage
remains a failure, never substituted demo or empty-success data. Read-only sessions
retain browsing without gaining work-write authority.

Active search and filters appear as chips below the search. Filters inherited from
a named view are shown but only URL overrides can be removed, so view precedence is
unchanged; the view chip clears the named view. Rows show work status, priority
and a relative update time; hovering or focusing a time reveals the exact local and
UTC instant. Linked CI outcomes and durations in work detail use the same readable
labels while keeping the canonical value inspectable. The page **Refresh** re-reads
the bounded work snapshot.

<!-- r[impl kiln.web.console_layout] -->
Work pages use the shared page header: **New issue…** (or the destination's own
type) and **Create something else** sit in its action area. Issues, My inbox and
Requests & triage show the list in a card beside a detail card on wide screens
and stacked on narrow ones; with nothing selected the detail card reads
**Select an issue** (or the destination's item). **Hide list** collapses the list
while a record is open. The **Active / Backlog / Done / All** quick status sets
only the exact `work_status` URL override (started, backlog, completed or none),
so it never adds a new status rule; under a saved view with its own status, the
last segment reads **View default** and clears only the override. Planning and
board layouts keep full width with card columns; the selected record opens below.
Each row's **Actions** menu (also right-click or Shift+F10) offers Open detail, Quick
peek, selection and Watch/Favorite/Mark read, with the same labels as the detail.


Overview count chips link to retained run filters; the active count opens busy
workers. Failing projects appear before healthy projects. Projects leads with
the inventory cards, which show each project's latest retained run (started,
requested or finished, by state); the start form and enrollment follow it.
**Select for new run** only selects the project in the start form and never
dispatches it.

Runs lists retained runs as a table grouped under local start-day headings,
with status, project (linking to its project page), short source revision
(exact value in the tooltip), duration and relative start time. Column headers
sort only the loaded page and say so; service filters, cursor paging and the
URL-restored selection are unchanged. Rows are keyboard focusable: arrow keys
move between rows and Enter opens one. On screens at least 90rem wide the
selected run opens beside the table (rows then stack compactly); narrower
screens open it below. The table also stacks its rows when its region is 48rem
or narrower, with the sort controls above them; on a phone the status and the
start time each take a line. **Hide run list** collapses the table beside an
open run.
The run header shows the outcome pill and phase above a facts grid. Open **Exact
source & execution identities** for complete values, including exact local/UTC
timestamps. **Rerun…** (exact revision) and **Cancel run…** appear in the header
only when the principal may operate and the run advertises them; they open the
same exact-identity confirmation as the **Execution actions** disclosure, which
still holds pause, resume and the other rerun modes. The confirmation names the
run and the verb, for example `Cancel run "Kiln — Rust workspace tests"?` with a
**Cancel run** commit and a **Keep running** dismissal (**Keep queued** for queued
work, **Keep paused** when resuming, **Cancel** for reruns); its exact facts are
unchanged. Active runs show elapsed
time since the recorded start (or Not recorded) and `n of m nodes complete`,
updated from the live stream; the ticker stops at a terminal state, on leaving
the run and on session clearing. Opening Logs reads the selected stream
immediately; empty output is not success or complete-log evidence. The pipeline
and a terminal-styled log panel are shown together on wide screens (side by
side when the inspector has room); the Pipeline and Logs tabs move focus to their
panel and the other tabs open below them. Narrower screens keep one tabbed
column, except that a failed run always shows its logs before the pipeline.

A failed run opened without `node` or `stream` URL parameters starts on its
first failed node and stderr, falling back once to stdout when no stderr bytes
were retained; parameters named in the URL always win. Lines matching a bounded,
conservative error pattern (error, panicked, FAILED, failed, fatal, Traceback,
non-zero exit code) are highlighted over the unchanged retained text, the count
is shown, and **Jump to first error** scrolls to and focuses the first one.
Highlighting is a presentation aid and can miss or over-match lines.

`?page=project&project=<id>` is a project history page rendered by
`project.js`/`project.css` from bounded `/api/runs` pages and `/api/projects`.
It shows health and the current outcome streak, a timeline of recent runs
linking to exact executions, a duration trend over runs with recorded timing
only (stating how many were excluded), and the last success and last failure
with Logs links. **Run latest…** opens the existing start form preselected and
never dispatches by itself. Project names on the overview, project cards and
the runs table link here; the overview lists running projects first under
**In progress & unresolved** with a pulsing (static under reduced motion)
Running indicator.

<!-- r[impl kiln.monorepo.console] -->
Projects that share one repository (the same `repository_key`) are grouped on
**Projects** under a heading naming that repository and how many projects share
it; a project alone on its repository stays an ordinary card. Each card, and the
project history page, shows the project's path scope root (or **Whole
repository** when unscoped) and the build graphs it consults (Cargo, Nix). A run
recorded with an affected decision shows **Why this ran** under its facts: each
recorded reason in plain words (changed files in scope with a sample and count,
a matched declared dependency, the graph edge from the changed package to the
project's package, or the conservative fallback such as no earlier revision or
an unreadable build graph) and the changed-file count or why it is unknown; the
affected-set BLAKE3 is under the exact identities. **Automation** trigger cards
show the path scope, and signed event intake records list each project of the
event as affected (with reasons) or skipped (with its reason). In change review the CI requirement
lists every affected project with its CI state and a link to its run, and shows
skipped projects as **Not required** with their reason. Runs, triggers and
projects recorded before scopes existed show none of these additions.

Evidence and comparison inputs offer bounded retained-run suggestions while
still accepting exact IDs. Comparison suggestions exclude the current run and
other projects; missing suggestions do not establish that no older run exists.
Source requires an explicit repository selection. Changing or clearing that
selection clears stale files and diffs; independent history/tree/file failures
remain explicit. COB and receipt inspection preserves exact selected identities
even when inventory reads fail. Expand complete retained fields when needed;
producer decisions and missing authority values never grant acceptance authority.

On narrow screens, the navigation drawer traps keyboard focus. Escape or the
backdrop closes it; closed navigation is removed from keyboard focus. Page and
run changes move focus to the selected heading. Appearance and density remain
URL-restored, and clearing a session removes private views and suggestions.

Kiln application/dashboard maintainers own these frontend behaviors. Browser
asset overrides against authenticated read-only APIs exercise source candidates
without deploying them; they do not prove the installed service uses those bytes.

### Extend an existing project registry

<!-- r[related kiln.web.projects.onixresearch] -->

Ordinary startup remains bound to one exact service profile. Adding a project is
an explicit offline authority extension, not an implicit migration or a database
reset. Kiln application maintainers own admission and recovery behavior; the
operator owns repository selection, command review, installed routes and activation.

<!-- r[verify kiln.web.projects.onixresearch] -->
The [retained installed-project observation](../../.cairn/archive/1970-01-01-extend-live-project-registry/evidence/verification.json)
records fourteen additional reviewed committed-ref jobs through authenticated
enrollment and native routes: eight successes and six explicit prerequisite
failures. Original worker/routes, five original runs and retained bytes were
preserved; restart recovered all nineteen run views. These are the recorded
cohort's observations, not execution of the procedure below or qualification of
today's revisions. Existing Kiln self-CI retained its independently pinned cohort.

1. Preserve the original admitted service JSON. Export the next reviewed service
   profile with a nonempty appended project suffix. Preserve every existing
   project in order, including its label, source bounds, workflow and operation
   identities. Preserve all credentials, TLS/service/state roots and resource
   bounds. Only `bounds.max_projects` may increase; it may remain unchanged when
   existing capacity suffices.
2. Export and retain original new-worker JSON bytes and BLAKE3 identities. Prepare
   the reviewed saved workflows and appended exact `route_exact` / `shell_environment`
   graph/node mappings for the native handler. Preserve existing workers, saved
   workflow revisions and routes; changing them is not project extension authority.
   The typed [OnixResearch command catalog](../../config/onixresearch-projects.ncl)
   supplies reviewed committed-ref CI commands, not paths, credentials, source
   identities, installed routes or automatic enrollment. Review those bindings
   separately using the existing service and v2 worker builders.
   Jobs select the declared Git ref, not a Jujutsu bookmark or dirty worktree;
   retain each acquired commit even when those local names disagree.
3. Wait until every existing execution is terminal. Stop the service and native
   handler before importing workflows or extending the binding, and back up the
   SQLite database, retained logs/artifacts, original profiles and Lattice store.
   Import the reviewed saved workflows while quiescent. Keep the next profile
   separate until its binding transition succeeds.

```console
kiln-web-service extend-projects \
  --previous-profile "$KILN_PREVIOUS_SERVICE_PROFILE" \
  --profile "$KILN_NEXT_SERVICE_PROFILE"
```

The command requires an existing versioned store, takes the existing owned-state
lock, validates canonical replay and retained evidence, and transactionally
compare-and-swaps only the exact profile binding. It does not rewrite canonical
state bytes, call a provider, enroll projects or create missing service state.
An active owner/run, changed existing authority, stale/replayed transition,
invalid suffix, corrupt retained state or failed transaction rejects the change.
The old profile cannot reopen the successfully extended store.

Activate the matching next service profile and appended handler, then restart
the handler, service and any dependent proxy stopped with the service. Enroll
the new allowlisted projects through authenticated **Projects** intake. Exercise
their real jobs and inspect **Projects**, **Workers**, exact source revisions,
retained output and original histories. CI failures remain failures; enrollment
and a passing bounded job are not whole-repository release qualification.

The [recorded local registry exercise](../../.cairn/archive/1970-01-01-extend-live-project-registry/evidence/verification.json)
added fourteen projects while preserving the original five Kiln run views and
ten retained files. Eight initial checks passed. Five others could not fetch
SSH dependencies because the host rejected its SSH configuration permissions;
the remaining check received HTTP 404 for its pinned Octet archive. Those six
real prerequisite failures remain visible and do not authorize permission,
credential or dependency-pin changes. The receipt binds exact revisions,
operations, retained logs, installed executable and observed UI surfaces.

### Monorepo projects

Several service projects may share one repository. Each uses the identical
`repository_path`; another spelling of the same directory (for example a
symlink alias) is rejected, and one repository holds at most 64 projects. The
project API reports a shared `repository_key` (BLAKE3 of the canonical path). A project
without `scope` keeps repository-wide behaviour and is always affected. A scoped
project adds:

```json
"scope": {"root": "crates/api", "include": ["**"], "exclude": [], "depends_on": ["Cargo.lock"], "graphs": ["cargo"]}
```

| Field | Meaning | Default |
| --- | --- | --- |
| `root` | Repository-relative directory; `""` is the repository root. | Required |
| `include` | Globs relative to `root`. | `["**"]` |
| `exclude` | Globs relative to `root`. | `[]` |
| `depends_on` | Repository-relative globs that also affect the project. | `[]` |
| `graphs` | Distinct build-graph kinds to consult: `cargo`, `nix`. | `[]` |

Globs share code-review ownership semantics: `*` and `?` match within one `/`
segment and `**` across segments. A file is in scope when it lies under `root`,
matches an `include` and matches no `exclude`. Admission names the project and
field and serves no project of the profile when `root` or any pattern is
absolute, contains an empty, `.` or `..` segment, a backslash or NUL, or exceeds
256 bytes; when a list holds more than 32 patterns; or when `graphs` repeats or
names an unknown kind. `config/web-service.ncl` mirrors these rules; Rust
admission is authoritative.

**Affected decision.** For one exact base and candidate, Kiln decides every
project of the repository deterministically and records one affected-set BLAKE3.
A project is affected with one or more reasons:

- `unscoped`;
- `path` — a changed file is in scope (sample of at most 5 paths and a count);
- `declared` — a changed file matches a `depends_on` pattern;
- `graph` (`cargo` or `nix`) — a changed package is reachable, through the
  reverse dependency closure, from a package under `root`;
- `fallback` — `no_base`, `truncated`, `unsupported_provider`, `read_error`, or
  `graph_unavailable` with its reason. Uncertain facts never skip: a changed-file
  fallback affects every scoped project; an unavailable graph affects every
  project consulting that kind.

Otherwise the project is skipped with `no_changes_in_scope`. Changed files are a
Git tree diff between the exact base and candidate commits, never the working
tree; a rename contributes both paths. More than 10,000 paths or 1,048,576 total
path bytes yields `truncated`. A push uses the previous recorded head of the
reference as base (a new reference yields `no_base`); a pull request or review
uses the revision's base.

Graphs see only Cargo workspace members and `path` dependencies (including
`workspace = true` through `[workspace.dependencies]`) and Nix `flake.lock`
inputs of type `path`. Registry dependencies and Nix expressions reading sibling
directories without a `path` input are invisible; declare them in `depends_on`.

**Dispatch.** One webhook delivery to any trigger evaluates every enabled push
or pull-request trigger whose project shares the canonical repository and
reference, and queues runs only for affected projects; at most one trigger per
project takes part. Each project gets one automation source record carrying
`affected: {identity, decisions}`; skipped projects get status `skipped` with
`no_changes_in_scope` and no run. Repositories without any scoped project keep
today's dispatch exactly. The first push of a reference has no recorded base,
so every project runs with `fallback` `no_base`. Each queued run records `affected: {set_identity,
reasons, changed}`, where `changed` is `{exact_count}` or `{fallback}`; older
records load without it. Manual and scheduled runs are unchanged and record
`fallback` `no_base` only for scoped projects.

Jobs of a scoped project start in `<checkout>/<root>` inside the exact checkout.
A missing root fails the job with `project_root_missing`; one escaping the
checkout fails with `project_root_escapes_checkout`.

**Identity and review.** The scope identity, BLAKE3 over the scope and project
id, is bound into run and gate candidate identities, so changing a scope
invalidates older results. When a project on the review repository is scoped, code
review computes the affected decision for the revision's base and commit, and
the merge queue tests its candidate once per affected project. It requires, for
every affected project on the
review repository, a current clean full run for the exact commit and current
scope identity. Skipped projects do not block. The `ci` checklist item lists
`ci_projects` with a state of `passed`, `failed`, `pending`, `missing` or
`stale_scope`; CI from another project or an older scope never satisfies it.

### Source projection definitions

<!-- r[related kiln.source.projection.definition] r[related kiln.source.repository_identity] -->
`kiln_core::projection` defines how several source views over one exact
snapshot are described and resolved. It is a pure contract; its only consumers
so far are the worker materializer, the build-input planner and projected
writeback below, which no dispatch, service or console page calls yet, and the
project API still reports the path-derived `repository_key`.

- **Repository identity.** BLAKE3 over the source provider profile and the
  provider-native repository reference. Local cache paths and remote URLs are
  admitted locators and never enter it, so moving a cache keeps the identity
  and two checkouts both named `kiln` stay distinct. One handle with two
  provider bindings is refused unless a reconciliation names both; the
  retired identity is then recorded as superseded.
- **Definitions** are authored in Nickel against
  `config/source-projection-contracts.ncl` (`Definition`). The profile is
  `navigation`, `runnable` or `writable`. The selection uses the monorepo
  `root`/`include`/`exclude` semantics above. Mounts map source prefixes to
  destination prefixes; every matching mount emits, so one source can appear
  twice as an alias. Creation destinations are writable-only, runnable and
  writable profiles require `exact_tree`, admitted generated inputs are
  runnable-only, and the target profile must represent every required
  capability. Rust admission is authoritative and computes the definition
  identity from the canonical form, so reordering declarations changes
  nothing.
- **Manifests** bind the repository identity, exact revision, existing
  `canonical_tree`, definition, evaluator and adapter versions, target,
  coverage and dependency bindings. Every destination records a native origin
  (source path, provider-native object and an origin identity) or a generated
  origin (generator and input identity). `content_identity` covers what a
  consumer would materialize; `identity` covers everything. Faults (unsafe
  paths, unmapped selected paths, collisions including case-fold and
  file/directory conflicts, unsupported capabilities, bounds) are reported as
  sorted diagnostics before any effect.
- **Completeness** is a type. A navigation, truncated, missing- or
  denied-dependency, or generated selection cannot become a complete build or
  write workspace, and a stored manifest is only checked for consistency and
  staleness. A complete projection still grants no read, write, delegation or
  publication authority.

`ProjectionDefinition::from_project_scope` converts an admitted monorepo scope
into an equivalent definition labeled `monorepo_project_scope.v1`; its
`depends_on` and graph kinds stay affected-project facts.

<!-- r[related kiln.source.projection.disclosure.restricted] -->
**Capability-scoped views.** `kiln_core::projection::{authority, disclosure,
sharing}` decide what one holder may see of a projection. A holder never
receives the trusted manifest, and a restricted view, context or scope cannot
be deserialized; each comes only from current authority.

- **Vocabulary and scope.** The source-view vocabulary,
  `kiln.source-authority-coverage.v1`, has eight operations (discover, read
  content, read history, prepare workspace, modify source, create source,
  delegate, retrieve result), each mapped to one `kiln/source/...` ability.
  None implies another, and submission and publication stay protected
  implementation operations. Only an authority adapter builds an
  `EffectiveSourceScope`, from a verified session bound to one exact
  revision. It holds only the grants the port admitted request by request: a
  session's raw capability set confirms those grants, only when observed for
  the same audience, repository, snapshot, view binding and generations, but
  never adds one. Path scope is compared per component, so a grant for
  `crates/app` never covers `crates/apple`. The local-operator profile,
  `LocalSourceAuthority`, is an explicit full-source grant without
  delegation.
- **Scoped selection and enumeration.** `ScopedSelection::for_holder` is the
  view's frozen `ViewCeiling` intersected with the paths the holder's current
  grants let it discover or read; it covers the repository root only for a
  full-source holder. The shell asks the source adapter to enumerate over
  that selection and records the result with `ScopedEnumeration::new`, which
  refuses a native origin outside the selection and a generated entry when
  the selection does not cover the root. Whether an enumeration is truncated
  therefore never depends on entries the holder may not see. Kiln checks the
  claimed selection and containment only; it cannot see entries an adapter
  silently dropped.
- **Truncated facts are enumerated again, never filtered.**
  `ScopedSelection::filter_facts` narrows complete facts to the selection
  (generated entries only for a root selection) and refuses truncated facts
  with `ScopingError::TruncatedInput`: where a wider enumeration stopped
  depends on hidden entries, so carrying its truncation into a holder's view
  would let them decide its coverage and its unknown answers. Enumerate the
  case again over the holder's selection. `ScopingError` has no public form
  and does not serialize, so it can never be shown to a holder as
  availability.
- **Restriction.** `disclosure::restrict` refuses an expired scope, another
  repository or revision, a scope that names neither discover nor read
  content, and, as stale, an enumeration recorded for any selection other
  than the holder's current one. It then keeps an entry only when its native
  origin lies in the ceiling and a current grant lets the holder discover or
  read it. Aliases are judged by their origin; generated entries and
  dependency identities reach only a full-source holder, while dependency
  names are shown. Only kept entries are counted, paged (at most 256 per
  page) or turned into coverage, and a view lists at most 4,096 entries.
  Coverage is complete only when every authorized entry is listed, and
  otherwise truncated after the entries listed. For a path the holder may
  discover, `AudienceView::lookup_origin` tells present, unrequested by the
  definition, absent, unknown (the holder-scoped enumeration was truncated)
  and truncated (the view's own bound cut it) apart.
- **Handles and bindings.** Handles, cursors and the view reference are
  BLAKE3 hashes keyed by a shell-held `HandleKey` over holder-visible data
  only (audience, scope, ceiling, definition and visible content), so a
  known digest, an equal-byte file or an alias forges nothing. The
  `ViewBinding` identity also covers the trusted manifest and the authority
  and revocation generations; it keys caches and stale checks only. Every
  page, lookup and handle resolution first rechecks the binding against the
  holder's current scope, and resolution rechecks the operation over the
  handle's origin.
- **Uniform public refusals.** A missing, forbidden or stale object (an
  `Unavailable` from a lookup, resolution or page) and every
  `DisclosureRefusal` from `restrict` present one public shape,
  `{"error":"unavailable"}`. The reason (`Missing`, `Forbidden`,
  `Truncated`, `Unknown`, `Stale`, or the refusal itself) stays server-side
  for the owner's audit and never reaches the holder.
- **Owner-side attribution.** `AudienceView::attribute` attributes a refused
  probe for the owner only, and only against a complete audit enumeration
  made over `ScopedSelection::whole` of the view's frozen ceiling. For a
  holder whose grants cover the whole ceiling, such as a full-source holder,
  the two selections are equal and its own enumeration is a valid audit. An
  audit over any other selection, such as another holder's narrower cached
  view, and a truncated audit get no attribution, since either could report
  a hidden file as missing; a displayed destination gets none either. Under
  a valid audit, a destination the audit lacks is `Missing`, one the holder
  may not discover through the ceiling under a current grant is
  `Forbidden`, and a discoverable one the listing stopped before is
  `Truncated` or `Unknown`. A scope that is no longer current, or an audit
  of another repository, revision or definition or lacking a destination the
  view lists, is `Stale`.
- **Cairn context.** `restrict_context` discloses Cairn items through
  `.cairn/changes/<id>/**` path authority, from an observation of the view's
  repository at its exact revision; text needs read content. A hidden
  prerequisite of a visible item is an opaque handle whose status reads
  `unavailable` unless the holder may discover its `.cairn/changes/<id>/`
  change, and a `RestrictedContext` can never stand in for a complete
  observation.
- **Sharing** is its own operation. `sharing::admit_share` derives a
  `SharePlan` no broader than the sharer's scope and the frozen ceiling in
  path, operation, snapshot and lifetime; it needs `delegate` over every
  shared path, gives the recipient a depth below the sharer's, and binds the
  ceiling, so expanding a saved view later never expands an existing share.
  A view reference or handle alone confers nothing.
  `kiln_adapter_basalt::issue_delegated_source_grant` issues attenuated
  `kiln://source/v1/` UCAN delegations from an admitted `SharePlan`.

Known owner-side gaps, tracked in
[#110](https://github.com/OnixResearch/kiln/issues/110), affect only how a
probe is attributed in the owner's audit; what a holder sees is unchanged:

- The only accepted audit is over the frozen ceiling. When a holder views a
  newer definition than the one its ceiling came from, a probe of an
  existing file that the newer definition adds but the ceiling hides is
  attributed `Missing`, not `Forbidden`.
- `ScopedEnumeration::new` checks containment only, so a shell that labels a
  holder's own scoped view as a whole-ceiling audit gets hidden files
  attributed `Missing`. Only pass an enumeration actually made over
  `ScopedSelection::whole`.
- Owner-side audit mismatches (snapshot, definition, or a listed destination
  missing from the audit) return `Stale`, the same reason as holder
  staleness, while a wrong selection or a truncated audit returns no
  attribution.

Until #110 is resolved, a `Missing` attribution does not prove that the
path does not exist.

The restricted corpus (maintainer, project-reader and agent views, a hidden
dependency, equal-byte files, an alias and a hidden Cairn prerequisite) is
authored in `config/fixtures/source-projection-restricted-positive.ncl`
against the source-authority contracts in
`config/source-projection-contracts.ncl`. The Rust tests read its committed
export, `fixtures/source-projection/restricted/corpus.json`, and
`scripts/check-nickel-fixtures.sh` refuses a stale export.

`BasaltSourceAuthority` (`kiln-adapter-basalt`) bridges `SourceAuthority` to
vendored `basalt::kiln::{Verifier::verify_source, revalidate_source,
source_enforcement}` using `kiln://source/v1/` length-hex-framed UCAN grants,
enforcing single-use replay, delegation depth bounds (`<= MAX_DELEGATION_DEPTH`),
and `RevocationView` / `SnapshotRule` checks before consuming replay nonces.
When `ServiceProfile::restricted_projection` is enabled, `kiln-web-service`
constructs `sources::Browser::restricted` and routes `/api/sources*` and
`/api/implementation/cairn*` through `restricted_response`, filtering visible
repositories, paths, handles, Cairn changes, text, and prerequisite status
(`PublicUnavailable` `{"error":"unavailable"}` on any missing/denied/stale
scope or absent observer context).

<!-- r[related kiln.source.projection.materialization] -->
**Materialization.** `kiln_core::projection::materialization::plan` turns a
complete projection and a `kiln.materializer-profile.v1` profile into the
entries to write, a reusable input identity (manifest, content and profile)
and, per attempt, an overlay identity (input, attempt and a non-zero fence).
The `eager_local_directory` profile refuses symbolic links, generated entries,
executable entries on a platform without executable bits, a platform whose
case rules differ from the projection target and over-long destinations.
`kiln_web_service::worker::materialize` writes a plan under a private store:
it verifies each Git blob against its object identity, writes only the planned
entries into a fresh preparing directory, seals them read-only, publishes the
input with a no-replace rename and then records its binding. A reused input is
verified again; an attempt overlay is a private writable copy with its own
binding. A directory or binding without its counterpart, or bound to another
plan, is refused and left as it is, and an interrupted preparing directory is
never adopted.

<!-- r[related kiln.source.projection.materialization.scenario.pinned] -->
**Workspace pins.** `MaterializationPlan::pin` derives a
`kiln.materialization.workspace-pin.v1` record for one attempt and fence: the
admitted repository identity, exact source revision, definition and manifest
identities, materializer profile identity, input identity and overlay
generation. An implementation dispatch plan may carry it as
`workspace.projection`. Admission then refuses a pin for another attempt, fence
or base revision, an overlay that is not the identity of its input, attempt and
fence, and a workspace identity other than that overlay, and the plan identity
includes the pin. A plan without one keeps its earlier identity.
`prepare_overlay` takes the dispatch pin, refuses one that differs from the
plan it is given, and resumes an overlay only when its
`kiln.materialization.binding.v2` binding records exactly that pin; v1
bindings are refused, not adopted.

<!-- r[related kiln.source.projection.route] -->
**Projected workspace routes.** `materialization::route` derives a
`kiln.projected-workspace-route.v1` route from the admitted definition, the
completeness witness, the plan and the pin: the workspace root is the overlay
under the store, read paths are the top-level components of the planned (and,
for a write, creation) destinations, and write paths are the overlay
destinations of origins within the dispatch plan's permitted paths plus each
creation destination whose every mounted source path, every origin planned at
or below it and every mount placed below it are permitted. A creation
directory that holds an alias of an unpermitted origin (mounts `"" → ""` and
`private → src/private` with creation destination `src`) is not granted
whole. Paths are relative to the overlay, as Lattice request v3 names them;
the input root is recorded but never mounted, and the checkout, Git database
and store stay outside. Place the store under a private state root such as
`/var/lib/kiln`: a store at or below `/nix/store`, `/usr`, `/bin`, `/lib`,
`/lib64` or `/run/current-system/sw/bin`, which Lattice's bubblewrap
provider binds into every sandbox, is refused, and `kiln-lattice-dispatch`
also refuses a store at or below the resolved target of any of these roots
that is a link (with `/usr -> /srv`, a store under `/srv`) or a store whose
path is not canonical; a root present but unresolvable refuses every
dispatch. A destination with a `.git` or
`.jj` component is refused, as Lattice would refuse it only after dispatch.
The route carries the authored definition and resolved manifest it was
derived from. `worker::materialize::prepare_route` creates missing creation
destinations. `kiln-lattice-dispatch --projected-workspace-route FILE`
refuses, after the dispatch authority pre-check and before the credential read
or any durable step, a projected dispatch under any authority
profile that does not enable projection (`projection-requires-operator`). Only
a selected `--authority-config` verified UCAN profile with
`projection_enabled` enables projection; `--local-operator`, a verified profile
without `projection_enabled` and the absent profile never do. It also refuses
a pinned plan without its route,
a route for another pin or an unpinned plan, a route that derived again for
the plan's own permitted paths has other paths, a linked overlay root, and a
request whose `workspace_root`, `read_paths`, `write_paths` or network
differ; `kiln-implementation-fixture bind-execution
--projected-workspace-route FILE` binds the same paths and emits the route. A
file write path admits in-place edits only; removing an origin in the runtime
needs a writable creation destination. `SourceAuthority` admission,
revalidation, and pure `check_request` enforcement are wired at retrieval and
workspace effects (S7): `worker::materialize::{prepare_input, prepare_overlay,
prepare_route}`, `kiln-adapter-lattice`'s `check_route` / `worker.rs` (checked
before reading credentials via `open_verified_source_authority`),
`worker::writeback::observe` (`RetrieveResult`), per-`SourceOperation`
`worker::writeback::seal` (`ModifySource`, `CreateSource`, `ReadContent`),
`worker::writeback::retrieve_result`, and the `projected-runtime` example.

<!-- r[related kiln.source.projection.materialization.scenario.baseline] -->
**Materializer baseline.** `config/workspace-corpus-contracts.ncl` types a
shared corpus (`kiln.workspace-corpus.v1`; export
`fixtures/workspace-corpus/corpus.json`): two projects that share a protocol
directory and its declared dependency, an executable script, equal-byte
license files at two origins, an excluded environment file and a file neither
project selects.

```console
cargo run -p kiln-web-service --example workspace-baseline -- \
    --corpus fixtures/workspace-corpus/corpus.json --out receipt.json
```

The off-by-default example commits the corpus to a temporary bare Git
repository at a fixed time. It then materializes every projection through the
eager local materializer for two attempts and writes a
`kiln.workspace-baseline.receipt.v2` receipt with each projection's manifest,
input, entries and overlays, these checks and the timings:

| Check | Evidence |
| --- | --- |
| `reproducible_input` | Two stores hold byte- and mode-identical inputs |
| `independent_attempts` | An edit in one attempt's overlay leaves the other overlay and the input unchanged |
| `verified_input_reuse` | A second preparation re-verifies and reuses the published input |
| `executable_modes` | Sealed 0555/0444 input modes and writable 0755/0644 overlay modes |
| `distinct_origins` | Equal bytes at two paths keep two origin identities |
| `excluded_entry_absent` | An excluded file is absent; each input holds exactly its manifest entries |
| `shared_dependency` | Shared entries and the declared dependency are bound in both projects with identical bytes |
| `damaged_input_refused` | A file changed after publication is refused on reuse |
| `exact_pin_reopen` | An overlay reopens with its edits only under its exact dispatch pin |

The `workspace_baseline` test compares the receipt, without timings, to
`fixtures/workspace-corpus/expected.json`; timings describe one host and are
never asserted.

<!-- r[related kiln.source.projection.build_inputs] -->
**Build inputs.** A complete projection is not yet a complete build input.
`kiln_core::projection::build_inputs` plans one build of a view from a
`kiln.build-input.request.v1` request: profile, targets, command, toolchain and
the whole declared environment. Layout facts come from
`kiln_build_graph::cargo_layout` (or the Nix graph) over the full candidate.
Every required input must be in the view at its own source path, and every
fault is refused before anything runs.

| Profile | Required inputs | Refused |
| --- | --- | --- |
| `cargo_compatibility.v1` | Every file of each target package, of every workspace member Cargo loads (every literal member and each glob member the view holds part of) and of their path-dependency closure; each workspace manifest those packages inherit from; the build root's `Cargo.lock`; `.cargo/config(.toml)` and `rust-toolchain(.toml)` at the build root and its ancestors | Relocated or generated entries; unknown or ambiguous targets; targets in several workspaces; unknown members; a path dependency outside the repository; any absent required input; a candidate without a lockfile |
| `nix_compatibility.v1` | The files of the target flakes and their path-input closure | Any view that omits a source file, because Nix expressions can read sibling paths no input declares |

Registry, Git and external flake inputs are recorded as `pinned_by_lockfile`,
never as part of the source closure. A `kiln.build-input.plan.v1` plan's BLAKE3
identity covers the candidate, the projection manifest and content, the
profile, every required input's path, role and object, the command, the
toolchain and the environment. `BuildInputPlan::accepts_evidence` accepts only
evidence recorded for that identity, so a changed layout, input, lockfile,
toolchain, command or environment needs new evidence. A plan names hidden
paths, so it stays with the controller. It grants no authority and does not
say that the build passes.

The off-by-default `build-input-qualification` example qualifies the typed
corpus in `config/build-input-contracts.ncl`
(`fixtures/build-inputs/corpus.json`): a two-project Cargo workspace with glob
members, a shared path dependency, inherited workspace fields, a root
lockfile, Cargo configuration and a toolchain file.

```console
cargo run -p kiln-web-service --example build-input-qualification -- \
    --corpus fixtures/build-inputs/corpus.json --out receipt.json --execute
```

It commits the corpus with a working checkout, plans each case and
materializes planned views into attempt overlays. With `--execute` (and the
`build-input-fixtures` test feature, run by the `build-input-fixtures` Nix
check) it first confirms that `cargo --version` and `rustc --version` report
the declared toolchain. It then runs each planned command at its overlay's
build root. The environment holds only the declared variables, `PATH`, and
private `HOME`, `CARGO_HOME` and `CARGO_TARGET_DIR` directories. The
`kiln.build-input.qualification.receipt.v1` receipt records the cases below.

| Case | Plan | Execution |
| --- | --- | --- |
| `alpha-tests`: alpha, the shared dependency and root files | Planned | Passes; entries unchanged |
| `beta-tests`: beta, the shared dependency and root files | Planned | Fails: beta reads a file of alpha's package that no manifest declares, and the overlay does not hold it, although the working checkout does |
| `alpha-without-proto` | Refused: the shared dependency is absent | Not run |
| `alpha-without-configuration` | Refused: `.cargo/config.toml` is absent | Not run |
| `alpha-relocated` | Refused: relocation is not a qualified transformation | Not run |

An edit to the shared dependency written through the `alpha-tests` overlay maps
back to `shared/proto/src/lib.rs`. `monorepo::affected` over the full
candidate's Cargo graph then selects both alpha and beta, although beta is
outside the editing view. The receipt is compared, without timings and the
observed toolchain, to `fixtures/build-inputs/expected.json`. Cargo
configuration above the overlay, absolute paths and the host toolchain stay
outside the plan. The off-by-default `projected-runtime` example dispatches the
`alpha-tests` and `beta-tests` cases and the two-project Nix corpus
(`fixtures/projected-runtime/nix-corpus.json`) through `kiln-lattice-dispatch`
to a real Lattice binary under bubblewrap, under the
`--authority-config` it is given, which must select the verified profile with
`projection_enabled`. It runs each planned command inside the route, records
the confinement probes (host checkout, `.git`, store backing paths, UNIX,
abstract and loopback sockets, writes outside the write paths, a symlink
escape) and the local-operator refusal, and captures the overlay into
`kiln.projected-candidate-facts.v1` facts; a case qualifies only on the
complete expected probe set and, for `beta-tests`, an observed build failure
whose recorded output names the missing `alpha/assets/banner.txt`.
The run was repeated on the integration branch with the cohort Lattice
`5a8e1d51` under a verified UCAN profile with `projection_enabled` whose keys
and grants are Basalt's synthetic test-support fixtures: every case's probe set
was complete and the local-operator request was refused. That qualifies the
runtime on that host and synthetic profile only, not under a deployment
authority profile; the earlier local-operator receipt stays superseded.
Relocation, generated-input providers and joining plan identities to the build
cache are later work.

<!-- r[related kiln.source.projection.writeback] -->
**Writeback.** A writable view's overlay is not a repository diff: it omits
and rearranges source files. `kiln_core::projection::writeback::capture` takes
a completeness witness for a write, its admitted definition, the overlay's
workspace pin, the shell's observation of the overlay and the attempt's
current fence and source facts. It maps every overlay file back through the
manifest and the mounts:

| Overlay | Captured as |
| --- | --- |
| Every destination of one origin unchanged | Nothing |
| Every destination of one origin changed to the same bytes and kind | One `edit` naming the expected base object |
| Every destination of one origin removed | One `delete` naming the expected base object |
| Destinations of one origin that disagree | Refused: `contradictory_alias` |
| A new file under a creation destination that one mount maps to an absent, selected source path | One `create` |
| A new file outside every creation destination | Refused: `undeclared_new_file` |
| A new file that no mount or several mounts map to a source path | Refused: `ambiguous_new_file` |
| A new file over a source file the view does not hold | Refused: `create_over_hidden_source` |
| A new file whose source path is outside the selection | Refused: `new_file_outside_selection` |
| A symbolic link | Refused: `unsupported_entry` |

Declarations (`Rename`, `Fork`, `AliasChange`, `MembershipChange`) name what
the overlay cannot show by itself: a rename or fork needs the origin's exact
bytes at a new file under a creation destination; an alias or membership
change is recorded as a view-definition operation and never changes the
source. Without a declaration a moved file stays a delete and a create. A
source that moved since the view was resolved yields a
`kiln.projection.writeback-conflict.v1` outcome naming the overlapping
canonical origins (a changed object or entry kind, a mode-only change
included), never a sealed candidate; a rebase is a new resolution and
overlay generation. A superseded fence (`superseded_attempt`), a pin for
another view or a build witness is refused first. The sealed candidate
(`kiln.projection.writeback-candidate.v2`) has a BLAKE3 identity over its base,
definition, manifest, overlay pin, operations and view operations;
`verify_recorded` still verifies recorded v1 candidates, read-only.
`SealedCandidate::reconstruct` applies the operations to the base entries, and
`candidate_facts` binds a sealed candidate to its build plan and the projects
`monorepo::affected` selects over the full candidate.

`kiln_web_service::worker::writeback::observe` hashes every overlay file as a
Git blob without following links. `seal` re-reads each captured file, refuses
bytes that changed after capture, writes only those blobs over the base tree,
and checks that the tree differs from the base at exactly the operation paths
and lists the reconstruction. It then commits with the base as parent and moves
no reference. Another attempt's pinned overlay keeps the old bytes; a view
resolved at the sealed commit shows the new ones. Handing candidates to
independent verification, publication and current-authority checks are
separate, later steps.

### Remote Lattice workers

The live service uses the existing [Iroh workflow bridge](ci-and-providers.md#iroh-remote-workflow-execution).
Set the project's workflow socket to the controller-local proxy socket. On the
worker, point the enrolled Iroh worker at its native Lattice handler socket and
register the exact reviewed graph, saved workflow revision and node mappings.
The worker must reach the service's registered HTTPS origin and trust its CA;
Iroh transports workflow exchanges, not source objects or worker callbacks.

The v2 worker profile separates two identities:

- `source_repository`: the controller's canonical Git repository native identity,
  exactly 64 lowercase hexadecimal characters, without a `b3:` prefix. For this
  local Git source provider it is BLAKE3 of the UTF-8 canonical controller
  repository path bytes, without a trailing newline. It must match the run's
  `repository.native_id` and Lattice's `LATTICE_SOURCE_REPOSITORY`.
- `repository_path`: an explicitly provisioned worker-local Git checkout containing
  the requested commit and bounded source tree. It may have a different absolute
  path and local repository identity. Provision/fetch source separately; the job
  does not fetch or substitute a moving branch. Dirty worktree bytes are not inputs.

Worker admission checks the declared repository against the producer context
before materialization. Callback admission still requires the exact acquired
commit and canonical tree, project, attempt, execution and selected nodes. The
local Git acquisition keeps its own identity; it is not relabeled as the controller
repository. This is an operator-reviewed binding, not repository ownership or
source-trust verification.

**Migration:** v1 profiles and missing bindings are rejected. Re-export reviewed
Nickel inputs with `source_repository`, retain the new original JSON bytes and
BLAKE3, and regenerate/review worker operation identities, graphs and exact native
routes before use. Historical receipts and deployed profiles are not migrated or
requalified implicitly.

Repeat the disposable native exercise with the selected current executables and
the maintained native Lattice source cohort:

```console
python3 scripts/web-service-smoke.py \
  --service "$KILN_WEB_SERVICE" --lattice "$LATTICE" \
  --lattice-source "$LATTICE_SOURCE" --iroh "$KILN_ADAPTER_IROH" \
  --root "$NEW_PRIVATE_SCRATCH_OUTSIDE_TMP" --evidence "$EVIDENCE_DIRECTORY"
```

Omit `--iroh` for direct local execution. The remote mode uses enrolled ephemeral
loopback peers and an independent worker clone; it exercises actual source
commands, dependencies, logs, artifact bytes, failures, planned restart recovery
and cancellation. For restart, it stops the controller before restarting both
bridges, then reconciles the original execution without redispatch. A live lost
exchange retains canonical `Unknown`; this check does not add an automatic
retry or restore a pending control surface after such an outage. It does not
establish WAN/relay availability or distinct-host deployment. `local`/`remote`
adapter labels bind supported capabilities and exact routes; they do not select
per-node placement or split a DAG across hosts.

### Optional signed COB workspace

The live **Work**, **Planning**, **Triage**, **My work & inbox** and **Saved views**
pages use an independently selected `--work` policy. Kiln's web-service maintainers
own this application surface; the immediate consumer is the developer coordinating
source-bound CI work. The existing CI project registry remains execution policy,
not an editable planning-project database.

Apply [`config/web-work.ncl`](../../config/web-work.ncl) to an operator-reviewed
record containing exactly `git_directory`, `signer_seed_file` and
`write_principals`. The builder emits `kiln.web-work.v1`. Select an existing,
operator-owned local Git directory, preferably a dedicated store, and provision
a private file containing exactly **32 raw bytes** of service-signing seed.
Hex text, implicit keys and discovered Radicle/operator keystores are not accepted.
The Git directory must be owned by the service UID and not group/world-writable.
Keep the exact signing key and Git history together when backing up or restoring.

```console
nickel export --format json --output "$KILN_WORK_PROFILE" "$KILN_WORK_NCL"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE" --work "$KILN_WORK_PROFILE"
kiln-web-service serve --profile "$KILN_SERVICE_PROFILE" --work "$KILN_WORK_PROFILE"
```

Retain other independently admitted runtime options. Omitting `--work` leaves an
explicit unconfigured surface and never discovers a default store. `check`
admits policy and private seed without opening or modifying the Git repository;
actual requests additionally verify store ownership, native framing, signatures
and replay. No installed store, service configuration or operator key is changed
by the disposable verification procedure below.

An authenticated Viewer or Operator may collaborate only when its **exact
application principal ID** is listed in `write_principals`. This is independent
of CI Operator permission. Unlisted users remain read-only; Workers cannot use
the work API. Same-origin JSON write admission is still required. Removing a
writer denies its new commands while preserving its previously service-signed
history. The signature attests the service-recorded authenticated principal,
not an end-user Radicle signature or user-held key.

Nine version-1 native types under `xyz.onixresearch.work.` retain issues,
projects, milestones, cycles, initiatives, teams, requests, views and personal
attention records. The pure model validates typed relationships, parent/blocker/
duplicate cycles, team-defined statuses, scheduling, authorship and privacy.
Requests preserve original customer/source intake. Comments, mentions, reactions,
descriptions and team/project updates are durable operations, not browser-local
canonical state. Personal subscriptions, read-head markers, favorites and
reminders are private COB fields. The inbox derives attention from the admitted
snapshot and browser time; this does not provide background push/email delivery.

Private views and personal records are owner-controlled at the application API;
they are **not encrypted from Git/store administrators**. View ownership and
visibility are immutable: create a separate shared view rather than exposing a
formerly private history. All work types deliberately stay out of the separate
raw `/api/cobs` browser, which has no principal-aware work-history privacy.

Writes confirm exact values and native heads. Stale heads, changed request
replays and competing/foreign branches fail explicitly; exact duplicate requests
return the original historical result. A signed global sequence and predecessor
operation bind cross-object replay independently of timestamps and object-ID
ordering. The local store uses a cross-process lock and native ref CAS; it is a
single application history, **not automatic CRDT merging or remote synchronization**.
Bounds include 1,024 objects, 128 operations per object, 8,192 total operations,
16 MiB of verified history, 256 KiB per operation/reduced record and 4 MiB per
HTTP response. Exceeding a bound reports unavailable rather than truncated success.

Use Ctrl/Cmd+K for cross-entity search and commands; results are grouped under
headings (Search, Go to, Create, Work, CI projects, CI runs, Retained reviews) and
arrow keys follow that visible order. Work rows support j/k or
arrow navigation, Enter to open, Space to preview, x or checkboxes for selection,
and Shift+F10 for actions. Bulk changes confirm per-object heads, run sequentially
and report partial outcomes; they are not atomic and are never blindly retried.
Named views persist filters, sorting, grouping and list/board layout as COBs.
Explicit URL controls override a named view only when present; opening it from
favorites or the palette restores its saved configuration, including board layout.
The Kiln web-service shell owns this optional collaboration surface and its
native smoke exercise; its consumers are authenticated workspace principals.
Clearing the session erases private snapshots, previews, results and pending reads.

<!-- r[related kiln.work.kanban.config] -->
<!-- r[related kiln.work.kanban.boundary] -->
**Kanban board.** On the Work page, the board layout is a Kanban board over issue COBs. A saved
view is the board: its object ID, filters, sort and private/shared visibility
apply to both its list and board layouts, and switching layout never writes the
view. A view may store `board.columns`, an ordered list of 1 to 64 distinct
statuses (edit it in the view form). Views without it, including every view
written before boards existed, read and replay unchanged and use the default
columns: Triage, Backlog, Started, then team-defined statuses (only the view's
`team` filter, otherwise every team ordered by title, each in workflow order),
then Completed, Cancelled, Snoozed, Duplicate. `group_by` applies to the list.

Every column is shown even when empty. Team-defined columns name their teams.
A new column must be built-in or defined by a team workflow; a kept column whose
status a team later removes is marked retired and is not a move target. Matching
issues whose status has no column appear under **Other statuses**; none is hidden.
A status outside the built-in set (a team status or a retired one) is a known
value: its indicator is a neutral cell marked custom, never an unknown state.
`GET /api/work/board?view=<id>` (plus the Work page's `kind`, `status`,
`assignee`, `project`, `q` and `sort` overrides) is the read-only projection. It
returns at most 100 cards per column with totals, matched, shown and list-only
counts, the 1,024-object snapshot bound and `complete`; the board states when it
is filtered or partial. Requests and planning records stay in the list layout.
Another principal's private view is reported as missing.

Moving a card (its **Move…** button or the `M` key, or dragging it to a column,
which opens the same confirmation) sends one `set_status` command for that
issue only, naming its exact head. The status must be built-in or in the issue's
team workflow; `duplicate` needs the issue form, and `snoozed` needs a future due
date. Writer admission is unchanged. A timeout or unavailable response leaves
the outcome unknown: **Retry same request** resends the identical request ID, so
an applied move returns its original result instead of moving twice. A head
conflict is shown, not overwritten: reload, then apply the move to the current
head with a new request, or keep the current status. WIP limits and manual card
ordering are not provided.

**Monorepo views.** A saved view may span a monorepo without losing its
components. `ci_project` and `ci_repository` accept comma-separated exact CI
project ids or BLAKE3 repository keys (at most 64 of either). A work record
matches only when one of its explicit `run` links names a retained run that
resolves to an admitted project in that set; an unlinked record, a removed
project, a lost run or an unknown key matches nothing and is shown as unlinked.
The planning `project` filter also accepts a bounded set (`project-a,project-b`),
and every member must be an admitted planning project when the view is written.
`group_by=ci_project` places each row once, under its first linked project in
repository-then-id order or **No linked CI project**. A board view may set
`board.lane_by=ci_project`; its lanes are the bounded, deterministically ordered
CI projects matching the view's CI filters (every admitted project when the view
names no CI filter), including empty lanes, and each card
appears once (status stays on the card). Issues whose links do not resolve appear
under **No linked CI project**. Status columns are rejected in lane mode; Move
still targets the canonical status columns (`move_columns`) and drag-to-lane is
available only on status boards. The console exposes the derived project and
repository filters, multi-project selection, CI grouping and lane labels, and
restores them from `work_ci_project`, `work_ci_repository` and `work_project` URL
overrides. Derived facts are a read-only `kiln.work.ci.v1` listing section; they
are never written into signed work records and do not prove CI success, execution
or release authority.

Board status is **planning state** only. Moving a card, including to Completed,
does not acquire an agent or worker execution lease, dispatch or cancel runs, or
grant any CI operator authority. It neither proves that CI passed nor that a
release completed; CI outcomes and Cairn/release acceptance remain separate,
exactly identified evidence.

Linked change detail joins only explicit source/revision, patch/review, run,
artifact and Cairn identities. Missing or inaccessible observations remain
unavailable; work completion and recorded provider verdicts grant no merge,
lifecycle or release acceptance. No work action writes native Radicle
issue/patch objects, publishes refs, fetches peers or changes provider authority.

Repeat native persistence, restart, stale/replay, privacy and actual CI/source
joins using the maintained disposable exercise:

```console
cargo test --locked -p kiln-web-service
python3 scripts/web-service-smoke.py \
  --service "$KILN_WEB_SERVICE" --lattice "$LATTICE" \
  --lattice-source "$LATTICE_SOURCE" --work \
  --root "$NEW_PRIVATE_SCRATCH_OUTSIDE_TMP" --evidence "$EVIDENCE_DIRECTORY"
```

Add `--browser-hold` only when driving the browser and writing its evidence marker
before the exercise's bounded wait expires. The exercise creates caller-owned
disposable stores and credentials, uses real native signatures and source-bound
commands, and retires its resources. This is repeatability evidence for the
selected local cohort, not installed-service activation or release qualification.

### Optional read-only COB browser

<!-- r[related kiln.dashboard.cobs.operated] -->
<!-- r[related kiln.dashboard.cobs.operated.scenario.empty] -->

The live workspace's **COBs** view reads explicitly selected local Git
stores, independently of CI project enrollment. It does not search the current
directory, `$RAD_HOME`, a Radicle node, sibling checkouts or remote peers. Omitting
`--cobs` leaves the browser unconfigured; an admitted empty repository list is
configured but empty. Neither case discovers a default store.

Apply [`config/web-cobs.ncl`](../../config/web-cobs.ncl) to an operator-reviewed
input record with a `repositories` array. The builder exports schema
`kiln.web-cobs.v1`; each repository has exactly these fields:

| Field | Meaning |
| --- | --- |
| `id` | Stable public local key used by the `repository` query parameter; the Nickel component contract permits at most 100 lowercase ASCII letters, digits, `_`, `.` or `-`, excluding `.` and `..`. |
| `label` | Public display label, 1–128 printable ASCII characters in the authoring contract. |
| `repository_id` | Operator-declared native repository identity, 1–256 printable ASCII characters; this is not the local key or a derived path. |
| `git_directory` | Absolute server-owned Git directory, not a worktree or browser-supplied path; no `..` components. Select the actual bare store or worktree's Git directory explicitly. |

Runtime admission also rejects duplicate local keys, native repository IDs or
Git directory paths. The selected directory must exist as a directory, not a
final symlink, for a read to succeed. This is bounded local observation, not
hostile-filesystem confinement. Public keys, labels and native IDs are disclosed
to admitted readers: do not put secrets or host paths in those fields. Server
`git_directory` paths and filesystem diagnostics are not returned in API JSON.

Supply the reviewed expression and exported JSON paths separately from the
original service profile:

```console
nickel typecheck config/web-cobs.ncl
nickel export --format json --output "$KILN_COBS_PROFILE" "$KILN_COBS_NCL"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE" --cobs "$KILN_COBS_PROFILE"
kiln-web-service serve --profile "$KILN_SERVICE_PROFILE" --cobs "$KILN_COBS_PROFILE"
```

Retain any independently admitted `--access` and `--automation` options on both
commands. `check` admits the policy, not store availability, signatures or
history completeness; inspect actual readback after startup. This runtime policy
does not change the canonical CI `ServiceProfile` identity, worker JSON identity,
CI journal, source enrollment or workflow authority. The packaged deployment
does not implicitly enable it; a separately reviewed service invocation must
pass `--cobs` and provision read access to the selected directories.

The registry exposes three groups of types, with no producer reducer. The six
Cairn decision types are produced by Cairn:

- `xyz.onixresearch.cairn-change`
- `xyz.onixresearch.rfd`
- `xyz.onixresearch.adr`
- `xyz.onixresearch.verification-obligation`
- `xyz.onixresearch.release-decision`
- `xyz.onixresearch.policy-exception`

The three Radicle collaboration types are observed as they exist in local
stores, without translation, lifecycle reduction or Kiln meaning:

- `xyz.radicle.issue`
- `xyz.radicle.patch`
- `xyz.radworks.job`

Refs of any other type are skipped without failing the inventory; an explicit
`type` filter outside this registry is rejected with `cob_type_invalid`.

Authenticated viewers and operators may make bodyless GET requests:

- `/api/cobs` accepts only optional `repository` (local key), `type` (exact
  registered name), `q`, `limit` and `cursor`. Search is case-insensitive substring
  matching over bounded recorded root titles, the root-commit summary, object ID,
  local key, label, native repository ID, type and recorded author. It does not
  search later edits, document bodies or arbitrary action content.
  Ordering is by local key,
  type and native object ID. The default page has 25 objects; `limit` is 1–100.
  Keep filters and limit unchanged when following the returned `next_cursor`.
  Cursor snapshots bind those selections and observed object/ref-tip identities;
  moved refs or changed selections reject stale paging instead of silently
  skipping to another result.
- `/api/cobs/object?repository=KEY&type=TYPE&object=OID` requires all three exact
  selections. `OID` is the native 40-character lowercase hexadecimal root commit
  ID, not the generated Kiln display identity. Missing objects return an explicit
  missing result; malformed identities, unknown/duplicate query fields, bodies,
  browser paths, writes, anonymous readers and worker credentials are rejected.

Inventory returns all admitted public repository keys/labels/native IDs in
`repositories`, selected-store availability and counts in `sources`, all
registered `types`, paged `objects`, filtered `total`, `next_cursor`, `snapshot`,
`complete` and `non_claims`. Each object row carries its exact type, bounded
recorded root title (or root-summary fallback), native object ID and ref tips plus
the root commit's recorded author and time; the browser renders that time in local
time alongside UTC. Titles may come from later numbered actions in the root, not
just action zero. Unavailable or oversized metadata does not make up a title:
the root summary remains the fallback. These are not reduced current titles.
An unavailable
selected store makes the inventory incomplete,
not falsely empty; detail reads fail explicitly on unavailable/malformed stores
or exhausted bounds. Distinguish these from an available store containing no
supported objects. The page restores repository/type/search/paging and the exact
object selection in its URL. A stale selection does not auto-select a replacement;
clearing a session clears private COB views and pending reads.

Native refs are `refs/cobs/<type>/<root-oid>` or
`refs/namespaces/<key>/refs/cobs/<type>/<root-oid>`. The inspector preserves every
observed ref tip, native operation ID, Git parents, recorded author/time/message
and numbered JSON action blob identity/content. Each operation is checked against
its type manifest. History follows operation parents back to the selected root;
parents named by `Rad-Resource` or `Rad-Related` trailers are resource links, not
history edges. Concurrent tips remain observations, not a chosen semantic winner.

The inspector presents content by native type: Cairn change artifacts, RFD/ADR
documents, verification-obligation fields and claims, release candidates and
evidence, policy-exception requests, issue comments/edits, patch revisions/reviews
and native job Request/Run/Finished events. Named artifacts and fields have
separate disclosures; body text preserves its recorded whitespace without JSON
escape sequences. Markdown/HTML and source-supplied URLs remain inert text, not
executed markup, navigable links or fetched resources. Admitted patch-to-source
lookup links retain their separate exact-commit checks.

Exact object identities/ref tips, operation framing and raw JSON remain available
in labelled disclosures. Unknown or malformed action shapes open raw content
instead of guessing meaning. Every recorded operation remains visible in identity
order, including concurrent and superseded claims. A lifecycle, vote or job
outcome is a recorded claim, not current lifecycle state, CI success or acceptance
authority. This presentation path is maintained by Kiln dashboard maintainers.

Current implementation bounds are fixed, not policy knobs:

| Read boundary | Maximum |
| --- | --- |
| Policy JSON / repositories | 262,144 bytes / 64 |
| Each decoded query value / response JSON | 256 bytes / 4,194,304 bytes |
| Scanned refs per store / inventory objects across selected stores | 16,384 / 4,096 |
| Ref tips per object / history operations per detail | 128 / 128 |
| Git parents per operation / tree entries per operation | 128 / 256 |
| Commit / tree / manifest bytes | 65,536 / 65,536 / 4,096 |
| Numbered actions per operation / bytes per action | 128 / 262,144 |
| Root metadata preview bytes across numbered actions / excerpt characters | 8,192 / 180 |
| Retained action bytes across one history / displayed root-title or summary characters | 1,048,576 / 256 |

Raw action JSON and commit metadata are untrusted recorded content, rendered as
inert text. `signature_status = "not_verified"`, `signatures_verified = false`
and `reduced_state = null` are deliberate boundaries: this is not Cairn-reduced
lifecycle state, actor authentication, signed-ref verification, a semantic
conflict verdict, remote completeness, synchronization or publication proof.
Browsing performs no Radicle signing, mutation, fetch, sync or publication and
grants no CI, gate, acceptance or release authority. No repository credentials
or transport authority are admitted by this policy.

Kiln application/dashboard maintainers own this view and its repeatability
rail; operators own store selection, local filesystem authority, policy review,
reader enrollment and deployment. Repeat the persistent authoring checks from
the development shell:

```sh
nickel typecheck config/web-cobs.ncl
nickel export config/fixtures/web-cobs-positive.ncl >/dev/null
for fixture in config/fixtures/negative-web-cobs-*.ncl; do
  if nickel export "$fixture" >/dev/null 2>&1; then
    echo "negative COB fixture unexpectedly passed: $fixture" >&2
    exit 1
  fi
done
cargo test --locked -p kiln-web-service --test cobs
nix build path:$PWD#checks.x86_64-linux.nickel --no-link -L
```

The positive fixture and unknown-field/relative-path rejection fixtures are
authoring checks, not installed-store or successful runtime evidence. For actual
readback, open **COBs** as an admitted viewer/operator, inspect inventory and
one exact native object, follow a returned cursor, restore its URL and clear the
session. Repeat with a deliberately unavailable selected store and verify an
explicit incomplete/error state rather than an empty-success claim. Local
fixtures and browser observations do not qualify deployment or release readiness.

<!-- r[related kiln.dashboard.cobs.population] -->
<!-- r[related kiln.dashboard.cobs.population.scenario.import] -->

The browser shows nothing until the selected store holds `refs/cobs/<type>/<oid>`
objects. To populate it from the repository's own reviewed lifecycle artifacts,
import each active change once, as an explicit offline operator action:

```console
cairn cob import <change> --root . \
  --git-repository "$KILN_COB_STORE" \
  --keystore "$RAD_HOME/keys" \
  --workspace "$KILN_RID" \
  --policy "$PWD/cairn-policy/generated/cairn-policy.json"
```

Preview returns the actor, artifact identities and plan without mutation; add
`--execute` only after reviewing it. Creation signs with that keystore, so the
commit is bound to that Radicle identity. It writes only
`refs/namespaces/<actor>/refs/cobs/xyz.onixresearch.cairn-change/<oid>`, refuses a
change that already has a COB, and performs no publish, sync, announce, delegate
or peer action. The read-only COB browser cannot create these objects. Imported objects
are raw recorded history: signatures, authority and lifecycle reduction stay
unverified by the browser.

<!-- r[verify kiln.dashboard.cobs.population] -->
<!-- r[verify kiln.dashboard.cobs.population.scenario.import] -->
The [retained signed-population observation](../../.cairn/archive/1970-01-01-show-local-run-time-check-cobs/evidence/verification.json)
under `cob_population` records fourteen imports with the existing operator
identity, one new namespaced ref per object, and duplicate-import refusal.
Pre-existing refs, HEAD, packed refs and store configuration were unchanged.
The live browser listed all fourteen native IDs/tips and opened retained
artifact text; signatures remained unverified. No publication, synchronization,
announcement, delegate change or peer exchange occurred. This is scoped
historical evidence, not authority to import again or proof about newer changes.


Inspect the candidate-bound local outcomes in
`.cairn/archive/1970-01-01-add-onix-cob-dashboard/evidence/verification.json`; the commands
and procedure above are repeatability instructions, not a claim that a new
candidate has passed them.

### Read-only source browser

The live workspace's **Source** view reads explicitly admitted local
repositories. It does not discover repositories beside the service, search the
current directory, resolve a remote or check out a working tree.

Apply [`config/web-sources.ncl`](../../config/web-sources.ncl) to an
operator-reviewed input record. The builder exports schema
`kiln.web-sources.v1`; each entry has these fields:

| Field | Meaning |
| --- | --- |
| `id` | Stable public key used by `/api/sources/<id>/…`; lowercase ASCII letters, digits, `_`, `.` or `-`, excluding `.` and `..`. |
| `label` | Public display label, 1–128 printable ASCII characters. |
| `kind` | Exactly `git`, `jj` or `pijul`. |
| `path` | Absolute server-owned repository path. For Git this is a worktree or bare repository; for Jujutsu a colocated worktree or a store with `.jj/repo/store/git`; for Pijul a checkout containing `.pijul`. |
| `channel` | Pijul only: the channel whose current state is read. |
| `executable` | Pijul only: absolute path to the `pijul` program. Omitting it makes reads report unavailability rather than silently failing. |

Admission rejects unknown kinds, relative paths or executables, duplicate ids or
paths, `channel`/`executable` on a Git or Jujutsu entry, and a Pijul entry
without a channel. Paths, executables and object-store locations never appear in
browser requests or responses.

```console
nickel typecheck config/web-sources.ncl
nickel export --format json --output "$KILN_SOURCES_PROFILE" "$KILN_SOURCES_NCL"
kiln-web-service check --profile "$KILN_SERVICE_PROFILE" --sources "$KILN_SOURCES_PROFILE"
kiln-web-service serve --profile "$KILN_SERVICE_PROFILE" --sources "$KILN_SOURCES_PROFILE"
```

Retain any independently admitted `--access`, `--automation` and `--cobs`
options on both commands. Omitting `--sources` leaves the view unconfigured; an
admitted empty list is configured but empty.

Authenticated viewers and operators may make bodyless GET requests:

- `/api/sources` returns each configured key, label, kind, channel, availability,
  observed head and revision scope.
- `/api/sources/KEY/log?limit=&cursor=` returns at most 100 revisions per page,
  newest first. `cursor` is a bounded offset, not a revision selector.
- `/api/sources/KEY/tree?revision=&path=` lists one directory (`path` is a
  relative directory path; traversal, backslashes and absolute forms are
  rejected).
- `/api/sources/KEY/file?revision=&path=` returns one file's text when it is
  valid UTF-8 without NUL bytes, otherwise `binary: true` and no content.
- `/api/sources/KEY/diff?revision=` returns the exact commit's unified diff
  against its first parent (an empty parent for a root commit) with file,
  insertion and deletion counts. Truncation is reported, never silent. Pijul
  rejects this route: its CLI exposes no historical diff.

Git and Jujutsu reads resolve an exact 40-character commit id, or the current
head when `revision` is empty, from the repository's Git object store; a
symbolic revision is rejected. Pijul reads run the configured program under
Bounded Exec with fixed argv (`log`, `list`), a cleared environment and a 15 s
deadline. The Pijul CLI exposes no historical blob read, so Pijul trees describe
the current channel state and Pijul files describe the tracked working copy;
`revision` is rejected for a Pijul source and both scopes are labelled.

- `/api/sources/lookup?revision=` reports which admitted Git/Jujutsu sources
  actually contain one exact commit, so a recorded Radicle patch can be opened
  as a real diff. Sources that lack the object are omitted, never guessed.

Syntax highlighting is an opt-in addition to three reads. Add `highlight=1` to
`/api/sources/KEY/file`, `/api/sources/KEY/diff` or the code-review diff
`/api/code-review/changes/ID/diff`, and the response gains one `highlight`
field. Without the parameter, the response bytes do not change. Any other value
is a malformed query: `400 query_invalid` for Source reads and
`400 review_query_invalid` for code review. Highlighting runs after the bounded
read and never alters the text, `truncated` or any other field.

```json
{"schema":"kiln.highlight.v1","language":"rust","status":"highlighted","reason":null,
 "lines":[[[0,2,"keyword"],[3,7,"function"]],[]]}
```

- Each token is `[start, end, class]`. Offsets are UTF-16 code units within one
  line. Tokens are sorted, non-overlapping and inside the line.
- `class` is one of `keyword`, `string`, `comment`, `number`, `constant`,
  `type`, `function`, `macro`, `attribute`, `property`, `variable`, `operator`,
  `punctuation`, `tag`, `escape`, `heading` or `link`.
- **File:** `lines[i]` covers line `i` of `text` split on `\n`; a trailing
  `\n` starts no extra line. `language` is detected from the path.
- **Source diff:** `lines[i]` covers line `i` of `diff`, split the same way.
  Content offsets include the one-character `+`, `-` or space prefix. Header,
  hunk and `\ No newline` lines carry `[]`. `language` is `null`.
- **Code-review diff:** `lines` has one entry per row of
  `files[].hunks[].lines[]`, in order, after any `file=` filter. Offsets are
  relative to the row's `text`, and never cover its trailing `\n`. `language`
  is `null`.
- In a diff, each file takes its language from its old and new paths
  (`/dev/null` is an absent side). The old and new sides of each hunk are
  highlighted separately as context-free fragments; context rows use the new
  side. Rows of binary or unsupported files carry `[]`. A truncated diff still
  highlights its incomplete last file.
- `status` is `skipped` with `lines: []` and a `reason` when nothing is
  highlighted: `unsupported_language`, `binary` (NUL or non-UTF-8 bytes),
  `too_large`, `parse_error` or `truncated_input`. A diff is skipped when none of
  its files can be highlighted (the first file's reason is reported). Input
  over 512 KiB or 20,000 lines, more than 1,024 tokens on one line or 131,072
  in one result, or a response that would pass its 4 MiB bound is `too_large`.
  A diff of more than 256 files is `truncated_input`. A failure is reported as
  a skip, never as an error response.

The grammars are pinned Tree-sitter crates for Rust, JavaScript, TypeScript and
TSX, Python, Nix, Nickel, TOML, JSON, YAML, CSS, HTML, Bash, Markdown, C and Go
(see [LICENSES.md](../../LICENSES.md)). Embedded languages are highlighted inside
Markdown (inline markup and fenced code), HTML (`<script>`, `<style>`) and Nix
(shell strings). Tree-sitter's WebAssembly feature is not enabled.

The page (`source.js`/`source.css`) has a repository picker, a revision bar and
three views selected by `sview`:

- **Files** (default) lists one directory beside the selected file on wide
  screens and above it on narrow ones: directories first, then files, each with
  an icon and a humanized size. The filter narrows the loaded listing only; it
  is not a repository-wide search. The file viewer shows the path, size, line
  count and object id, Copy path, Copy link, Download (the retained text, byte
  for byte) and Wrap lines. Line numbers are links; Shift extends a range and
  Go to line selects one by keyboard or touch. Syntax highlighting comes from
  the service's pinned Tree-sitter grammars (the console requests
  `highlight=1`) and is presentation only. The file line names the language,
  or says why nothing is highlighted (unsupported language, binary, over the
  highlighting bound, parse failure); malformed token data renders the file
  plain with "Highlighting unavailable".
- **History** lists bounded revision pages (More revisions) with author name,
  relative time and short id; the head and the pinned revision are marked.
  Choosing a revision opens its Changes; Browse files pins it for Files.
- **Changes** shows the pinned revision against its first parent: totals, a
  changed-files list with status and +/- counts, and collapsible per-file hunks
  with old and new line numbers and +/- markers, highlighted by the same
  service grammars (a note says when they are not). Truncation is stated at the
  top and on the incomplete file. Without a pinned revision the view says how
  to pin one; for Pijul it states that historical changes are not readable.

The URL keeps every selection: `src`, `rev`, `spath`, `sfile`, `sq`, `sview`
and `sline` (a line `12` or a range `12-20`). A reload restores exactly that
selection; an unknown source, missing file, invalid revision or a line outside
the file is reported, never substituted, and nothing is selected automatically.
Newer/older steps, Use head and the up-one-level link change only the URL
selection, never the repository. A recorded patch commit opened from the COB
view lands on that revision's Changes.

Fixed implementation bounds: 64 sources per policy; 100 log entries per page;
512 tree entries; 262,144 file bytes; 262,144 diff bytes; 1 MiB retained program
output; 4 MiB response. Reads never fetch, check out, write a ref, sign, publish or alter CI
state, and they grant no CI, gate, acceptance or release authority. A Jujutsu
store without a Git-visible head, an unavailable Pijul program and an unreadable
repository are reported as unavailable, never as an empty repository.

Repeat the checks from the development shell:

```sh
nickel typecheck config/web-sources.ncl
nickel export config/fixtures/web-sources-positive.ncl >/dev/null
for fixture in config/fixtures/negative-web-sources-*.ncl; do
  if nickel export "$fixture" >/dev/null 2>&1; then
    echo "negative source fixture unexpectedly passed: $fixture" >&2
    exit 1
  fi
done
cargo test --locked -p kiln-web-service --test sources
```

### Supervised deployment

Kiln self-CI is the consumer; Kiln maintains the application and these assets.
The operator owns host selection, account/ACL reservation, TLS identity,
credentials, workflow authority and activation. Assembling a package or system
is not permission to activate it. No asset selects a real host or generates TLS.

#### Product and Onix integration

The dedicated product is `packages.${system}.web-service`. Its unit is installed
at `lib/systemd/system/kiln-web-service.service`; the package installer substitutes
`@out@` in `deployment/kiln-web-service.service.in` with the immutable output path.
The package must contain `bin/kiln-web-service`.

The external module export is exactly:

```nix
onixModules.kiln-web-service = { file = ./deployment/onixos-module.ncl; };
```

Use the reviewed Kiln flake as an existing consumer input. Pass its module set
through Onix's existing provenance route, and resolve its requested package with
an explicit overlay (inside the consumer's existing `onix.mkFlake` arguments):

```nix
moduleInputs.kiln = kiln.onixModules;
overlays = [
  (final: prev: {
    kiln-web-service = kiln.packages.${final.stdenv.hostPlatform.system}.web-service;
  })
];
```

In the existing inventory, bind an instance's `service` to `kiln-web-service` and
`module = { input = "kiln", name = "kiln-web-service" }`. Select exactly one
existing machine in `roles.server.machines`. Required `roles.server.settings`:

- `uid`, `gid`: operator-reserved non-root account IDs (not example host values).
- `profile_path`: absolute, reviewed runtime JSON path; no shell syntax or unit
  specifiers. The module does not create or replace this profile.
- `credential_sources`: four absolute runtime source paths keyed by
  `tls-certificate`, `tls-key`, `operator-token`, `worker-token`. These may name
  operator-provisioned files or files materialized by the consumer's existing
  artifact system. Do not pass Nix paths, secret strings, or store locations.

The module is explicitly NixOS/systemd-specific, not an implementation of a new
Onix backend. It emits private preserved state intent and native systemd leaves
because the portable service provider has no notify-readiness contract. It has
one fixed user/group/unit/state identity and rejects multiple instances or
multiple server machines. No supplementary groups or firewall openings are
created; grant the selected UID only the necessary repository read/traverse and
Lattice Unix-socket access outside this module.

The selected NixOS machine must enable `preservation` with an operator-chosen
`persistentStoragePath`, and the consumer must pass its reviewed
`inputs.preservation` into `mkFlake`. Provision that backing filesystem in the
consumer's existing hardware/storage definition. Onix rejects this module's
preserved state without the adapter; declaring a persistent root alone is not
sufficient. Do not downgrade the state to volatile to bypass admission.

#### Profile and credentials

`config/web-systemd.ncl` accepts the same application input as
`config/web-service.ncl`, **omitting** `tls`, `operator_token_file`,
`worker_token_file`, and `state_directory`. It delegates all application checks
to that builder and binds:

- state: `/var/lib/kiln-web-service`;
- certificate: `/run/credentials/kiln-web-service.service/tls-certificate`;
- private key: `/run/credentials/kiln-web-service.service/tls-key`;
- tokens: `/run/credentials/kiln-web-service.service/{operator-token,worker-token}`.

Export the reviewed application through this helper as JSON at `profile_path`.
The JSON contains references, not credential bytes. Supply real HTTPS origin,
listen address (unprivileged port), repository/source/workflow identities and
bounds through the original builder. The process has no bind capability. Keep
repositories under an accessible read-only deployment path such as `/srv`, and
proxy sockets under `/run`; `/home` is hidden and `/tmp` is private. Provision a
traversable proxy socket parent even if the external provider is unavailable.
Startup admission never performs a live provider exchange.

For the standalone packaged unit, use `/etc/kiln-web-service/profile.json` and
root-owned credential sources below `/var/lib/kiln-web-service-credentials/`:
`tls-certificate.pem`, `tls-private-key.pem`, `operator-token`, `worker-token`.
Pre-provision the dedicated `kiln-web-service` account with the reserved UID/GID
and no login access. A systemd drop-in can replace references for another layout;
clear `LoadCredential=` before replacing the four bindings. Do not change the
credential destination names or bound state without an explicit profile review.

Provision credential sources as root-owned, single-link regular files, mode
0400 or 0600, in a root-owned 0700 directory outside application-writable state.
The artifact owner must materialize these runtime files before start, never
place decrypted bytes in the Nix store or profile. systemd loads the sources into
root-owned **0440 files on a read-only credential mount**, with an ACL granting
read only to root and the service UID. Kiln checks that exact ACL and mount flag
before reading directly; no root helper, permission relaxation or private runtime
copy is needed. Ordinary group/world-readable files remain denied. No-follow,
single-link and private-key byte bounds still apply. Source provisioning remains
a trusted operator/artifact boundary, not a claim of race safety against root.

#### Supervision, rotation and recovery

The actual unit runs `check --profile` before `serve --profile`; preflight does
not mutate state, open listeners or dispatch work. `Type=notify` with
`NotifyAccess=main` waits for initialized TLS/listener/canonical state. HTTPS
`GET /healthz` is minimal and unauthenticated, with configured Host/origin
admission. Readiness is not proof the external provider is reachable.

The separately operator-managed desktop user service has its own [source-bound publication and redeployment receipt](../../.cairn/archive/1970-01-01-publish-deploy-gate-repair/evidence/deployment.json). That cutover builds immutable source `946e2d29`, checks all unchanged installed profile arguments, preserves consistent private SQLite backups, and replaces only the web-service Nix-profile element. Its existing `Requires=`-linked proxy/auth units restart with the web service; Lattice remains running. Rollback restores the prior package without rewinding live SQLite or collaboration refs. This local user-service observation is not deployment of system Aspen units or hostile-workload qualification.

State is owned by the dedicated identity, mode 0700, and is the only declared
persistent writable path. Source access is read-only. The unit uses no-new-
privileges, no capabilities, private devices/tmp, protected system/home/kernel,
restricted syscalls/namespaces/address families, 1 GiB memory, 128 tasks, 1024
open files and 200% CPU ceilings. Review those bounds against the chosen profile;
use an explicit reviewed drop-in rather than silently widening them. Restart is
on failure with a five-second delay and at most three attempts per minute;
startup has 60 seconds and SIGTERM shutdown 30 seconds before group kill.
External workflows belong to the provider and must not be child processes of
this web-service unit.

Canonical-state ownership is explicitly unlocked after the shell's other fields
close. A descriptor inherited during another thread's fork-to-exec window cannot
retain ownership after that shell is dropped. A second live owner is still
rejected; this does not bypass canonical replay, profile binding or recovery.

Rotate by provisioning replacement source files securely and restarting under
operator authority. Do not delete state to force a restart: durable state binds
the exact profile and recovers original operations without blind redispatch.
Restore the same profile, UID and state after a crash. Changed profiles require
explicit operator review; this helper provides no automatic state migration.

#### Maintained desktop user-service updates

The operated desktop workspace uses the **user** `kiln-web-service.service`,
not the system unit above. Its executable comes from the `kiln-web-service`
element in the operator's Nix profile. Updating it requires explicit operator
authority; do not change profiles, credentials, worker routes or durable state
to make a package start.

Freeze the source revision and build its dedicated `web-service` output. Retain
the current package path, a consistent SQLite backup, private configuration and
retained-file backups, and the exact API run snapshot before activation. Run the
new executable's `check` with every argument from the installed unit's
`ExecStartPre`. Set `new_package` to the built output and `previous_package` to
the recorded installed output. For the current Nix profile CLI:

```sh
nix profile remove kiln-web-service
nix profile add "$new_package"
systemctl --user restart kiln-web-service.service
```

`nix profile add` derives the element name from this package; it has no `--name`
option. The old process remains running during profile replacement. If adding
the new output fails, add `"$previous_package"` before restarting. If activation
fails after replacement, remove the new element, add the previous output and
restart the same user unit. Do not roll an old database snapshot over live state
as part of an executable rollback.

Existing `Requires=` dependencies can restart the identity resolver and tailnet
proxy with the web service. Verify all three units, the unchanged Lattice PID,
CA-verified HTTPS health, exact served asset identities, unchanged unrelated
profile elements, and preservation of the original runs and retained files.
Exercise the served browser without asset overrides. The
[2026-10-28 deployment receipt](../../.cairn/archive/1970-01-01-improve-lattice-rendering/evidence/deployment.json)
binds the published code, package, source acquisitions, rollback and live checks;
it does not waive lifecycle or release holds.

The later [signed-work deployment receipt](../../.cairn/archive/1970-01-01-add-cob-workspace/evidence/deployment.json)
records separately approved work-store/key provisioning and the `work.conf`
user-unit drop-in. When rolling back to an executable without `--work`, first
move that drop-in into the private rollback directory and reload the user daemon;
then restore the previous profile package and restart. Keep the new Git store
and signing seed together; do not delete work history or rewind CI state.
Original run fields remain preserved across this upgrade; the new response adds
an empty `step_telemetry` array for histories without recorded step observations.

The [workspace navigation deployment receipt](../../.cairn/archive/1970-01-01-repair-workspace-product-navigation/evidence/deployment.json)
binds the subsequent issue-first navigation repair to its published source,
running package and served asset identities. It records desktop and 320px mobile
browser checks without asset overrides, preserved runs and retained files, and
the private executable rollback backup. Existing traceability and release holds
remain unchanged.

The [visual identity deployment receipt](../../.cairn/archive/1970-01-01-establish-kiln-visual-identity/evidence/deployment.json)
binds published source `5a8f97e7` to the running package and all twenty served
asset identities. It records CA-verified local health, anonymous refusal,
authorized reads, preserved runs and retained files, desktop and 320px browser
checks over the local origin, and the rollback package. The tailnet origin was
unavailable before and after that activation, so it was not exercised.
Publication readbacks are in the neighbouring `publication.json`.

The [technical graphics standard deployment receipt](../../.cairn/changes/adopt-technical-graphics-standard/evidence/deployment.json)
binds local source commit `a79c6337`, which is not published, to the running
package and all twenty served asset identities. It records CA-verified local
and tailnet health, anonymous refusal, authorized reads, preserved runs,
retained files and work refs, desktop and 390px browser checks over the local
origin, and the rollback package. Lattice was failed before that activation and
was stopped again after one identical failed restart.

The [Patchgate receipt deployment receipt](../../.cairn/changes/clarify-patchgate-receipts/evidence/deployment.json)
binds local source commit `5ffe67b0` to the running web-service package and all
six changed asset identities on both origins. It records the packaged Rust
tests, the real retained receipt and its verbatim native report, desktop and
390px dark browser checks, and preserved CI state, retained files, work refs,
configuration and credentials. Request auditing changes only the audit history
and durable version. The administrative `--job-mode=ignore-requirements`
restart kept the already-inactive Lattice service stopped and the active
proxies running; this choice requires those dependency-state preconditions.
The source was not published, and lifecycle acceptance remains unchanged.

The [Workers deployment receipt](../../.cairn/changes/clarify-workers-page/evidence/deployment.json)
binds local source commit `79384053` to the running web-service package and all
six changed asset identities on both CA-verified origins. It records the
packaged release-profile tests and desktop/320px compact-dark browser checks
over the installed assets, without asset overrides or a browser TLS exception.
The original runs, retained files, signed work store, configuration and
credentials are preserved; only the bounded request audit and durable version
changed. The same dependency-limited restart kept both proxies running and
Lattice inactive. Source publication and lifecycle acceptance remain unchanged.

#### Non-mutating evaluation and isolated proof

Run these after integration, not while sibling changes are in flight:

```sh
nickel export deployment/fixtures/positive.ncl
nickel export config/fixtures/web-systemd-positive.ncl
nickel export --import-path "$ONIXOS/lib" deployment/fixtures/module-positive.ncl
```

Each `deployment/fixtures/negative-*.ncl` must fail export (module fixtures need
that same reviewed Onix import path); so must
`config/fixtures/negative-web-systemd-*.ncl`. They cover unsafe path expansion,
traversal/injection, missing/store/application-writable credential sources,
root/fractional account IDs, insecure origin and accidental state rebinding.
Symlinks/ownership/credential contents require runtime admission, not textual
path fixtures. The synthetic fixture package handle is only an eval argument;
it is never substituted for the real executable in runtime proof.

Build the dedicated package and run its disposable systemd qualification:

```sh
nix build path:$PWD#web-service --no-link --print-out-paths
nix build path:$PWD#checks.x86_64-linux.web-service-deployment --no-link -L
```

Assemble/build the consumer system without activating it. The disposable VM
exercises the packaged unit with real TLS and LoadCredential sources: HTTPS
health, main-PID readiness, UID/mode/state, SIGTERM, restart/recovery and
invalid/missing/private-file rejection. Run the
native workflow success/failure/log/artifact/cancellation rail separately with
the exact corrected Lattice cohort. Neither evaluation nor that isolated proof
claims production activation, source trust, arbitrary-job confinement,
lifecycle acceptance or release readiness.

### Use the live interface

Open the registered HTTPS origin. The page first makes a bounded, read-only
`GET /api/session` without `Authorization`. An admitted network identity opens
the workspace with its exact principal label and viewer/operator role; a 401
leaves the bearer form available. Direct localhost access still requires an
admitted bearer. Manually entered tokens stay only in memory; clearing the
session erases private views, pending requests and download URLs. Reload and
back/forward restoration recheck network authorization and never restore a
cleared bearer token. There are no hosted accounts, persistent browser sessions
or permissive CORS. A browser clear-session action cannot revoke network access.

Export separate reviewed `config/web-access.ncl` and `config/web-automation.ncl`
inputs and pass their JSON outputs as `--access` and `--automation` to both
`check` and `serve`. They do not replace the original service/worker profiles.
Access policy admits independent viewer credentials and, optionally, exact
Tailscale user ID/login/role bindings with a distinct private proxy credential.
A viewer or tailnet binding may declare `agent = { harness, model? }`; when code
review is configured, its exact `viewer:<id>` or `tailscale:<user-id>` principal
is merged into the review agent list so advisory/binding vote policy and submission attestation apply. The
declaration must name a review write principal or startup fails. Harness/model
text is bounded; it is configuration, not verified execution metadata. The
admitted session principal and attributable audit records carry the declaration,
and the workspace and activity feed show it as declared identity, not as an authority grant. When
present, it is recorded on revisions and submission receipts submitted by that
agent and shown with the attested principal.
Viewer requests can inspect retained facts but cannot enroll, run, control or
configure automation. Worker credentials remain limited to exact worker ingress.
Denied writes are attributable audit observations, not canonical CI events.

For an operator-authorized private-tailnet deployment, preserve the original
backend origin, profiles, tokens and state. Bind Nginx only to the declared
Tailscale addresses; keep Funnel disabled and unrelated Tailscale Serve routes
unchanged. Admit the exact public Host and Origin, translate only that Origin
to the registered backend origin, and validate upstream TLS against its CA.
Missing-Origin writes still reach backend rejection; foreign origins are denied.

Use `kiln-web-service proxy-auth --access "$KILN_ACCESS_PROFILE"` behind Nginx
`auth_request`. This private loopback endpoint resolves the actual TCP peer
through the configured tailscaled Unix LocalAPI socket. Only registered exact
user ID/login pairs on untagged tailnet nodes are admitted. Nginx must overwrite
the peer socket and forwarded principal headers, clear client Authorization,
and supply the distinct proxy credential to the resolver and HTTPS backend.
Never trust browser-supplied identity headers or give every peer a shared
operator bearer. Unregistered/tagged peers fail closed before application effects.

Supply the proxy credential using systemd `LoadCredential` and a startup-generated
private Nginx include in a 0700 `RuntimeDirectory` under `UMask=0077`. Keep it out
of the Nix store, source, URLs, HTML/JavaScript and logs. Restart the proxy after
rotation. The local operator owns peer ACLs, principal enrollment, CA renewal,
credentials and listener exposure; Kiln dashboard/service maintainers own the
reusable identity and browser flow. This is not a hostile-code sandbox.

Enroll an allowlisted project, start its source-bound run and inspect exact
revision/tree, attempt, native execution, node state and canonical outcome. Finite
live snapshots reconnect with visible loss/uncertainty; they are not a complete
event history. Mutations use durable request identities and are never blindly
retried. Controls require current canonical revision, attempt, native execution
and sequence plus the selected capability; stale/unsupported controls fail before
effects and require fresh operator action.

stdout/stderr are actual retained Bounded Exec bytes, continuously uploaded
before command exit with **worker-owned byte cursors**. Several steps sharing a
native node append to its streams. Bounded Exec owns command termination while
private bounded transports retain output; cancellation/deadline stops the owned
process group and descendants. Ingress failure never masks the command outcome
or silently retries ambiguous uploads. Initial worker binding obeys one absolute
lookup deadline, including each HTTPS request and polling sleep.

Empty output, capture truncation, read bounds and retention are explicit; active
work need not produce new bytes yet. Diagnostic logs retain **native provider
cursors**. Log search operates on retained bytes, including chunk boundaries;
full-stream download supplies the exact retained bytes and BLAKE3 metadata.
Neither stream invents progress or completion. Text is inert; byte cursors are
never inferred from displayed UTF-8 character counts. Follow works with an
admitted network session as well as a manual bearer.

The web viewer reads one bounded chunk at a time. Node, stream and search sit in
one toolbar; the cursor field, **Read from cursor** and **Read from start** are
under **Advanced**. **Read from cursor** appends when the requested cursor is the
displayed stream/search's next cursor. Empty reads preserve existing output and
text selection. **Read from start**, or an explicit different cursor, replaces
the displayed range after a successful read. **Follow** is enabled automatically
for an active run unless you uncheck it, and ends once the run is terminal and
no new bytes arrive.
Failed continuations and rejected seeks retain admitted text with an explicit
unavailable/stale status; they do not advance the accepted cursor. Changing a
run, node, stream or search invalidates old responses rather than mixing output.

Changing the literal search loads matching stdout/stderr from the beginning.
Diagnostic streams disable search and never submit `q`; switching back restores
the worker-stream search. Search skips are not source gaps. Follow scrolls only
while already at the tail, so scrolling up permits inspection of older text.
The existing display memory limit can still evict old visible text with a warning;
use an earlier cursor or the exact retained-stream download when needed.

Artifacts are real declared worker files, bound to exact source, attempt,
execution and known node. Uploads verify bounded bytes and BLAKE3 before atomic
owned retention; downloads use opaque IDs rather than caller paths. A receipt or
log is never substituted for a build product. Trusted worker declarations and
matching hashes do not prove product semantics or provider honesty.

The service has one state-directory owner, SQLite WAL/FULL durability and an
exact profile identity. Restart reconciles original admitted work; it does not
redispatch ambiguous work. Retention exhaustion rejects further intake rather
than silently pruning authority. Profile migration, retention rotation,
credential provisioning and worker isolation remain operator responsibilities.

#### Workspace navigation and recorded evidence

- **Overview:** retained totals, current project health, last-run completion time
  and recorded run-duration trends. Timestamps show browser-local date/time with
  its timezone alongside the exact ISO UTC instant. The same presentation is
  used in run and worker inspectors; locale/timezone do not change stored facts.
  Historical unknowns are excluded, not treated as zero. Missing/invalid times
  stay unavailable. Node timing is unknown when the first native observation is
  already terminal; no start time or duration is fabricated.
- **Workers:** configured execution targets, assigned projects, occupied/free
  project slots, queued work and exact latest execution observations. Metadata
  search, slot-state filters and independent exact selection are URL-restored;
  refresh and live snapshots retain selection. See [worker target inventory](#worker-target-inventory).
- **Runs:** metadata search, project/outcome/exact-source filters, oldest/newest
  ordering and stable bounded pagination. Cursors bind the filter/order and
  exclude later arrivals. A missing or filtered selection is not silently
  replaced. Page, run, node, inspection tab, comparison, appearance and density
  are URL-restored, including a full reload.
- **Pipeline:** admitted dependencies grouped by longest dependency depth, with
  recorded state badges/counts, measured node cards and highlighted immediate
  prerequisites/dependents. Stages do not imply execution order, readiness or
  placement. **Fit**, **1:1** (actual size), and bounded 35–150% zoom affect only the scrollable
  diagram; long names wrap and the bounded full-node list remains available.
  Select a card or list entry for exact state/timing facts, relationship links
  and **View node logs**. Selection is URL-restored; same-run refresh preserves
  diagram zoom/pan, list position and keyboard focus. Missing, ambiguous or stale
  node selections are never replaced. Historical missing edges and invalid
  topology show an explicit warning instead of an invented or partial diagram.
  Unknown timing stays unknown; a recorded zero duration remains zero. Neither
  Completed nor a green badge grants CI, gate, review or release authority.
  Clearing the session clears all private workflow views. Kiln dashboard
  maintainers own this browser presentation; Lattice retains execution semantics.
  See [bounded rendering evidence](../../.cairn/archive/1970-01-01-improve-lattice-rendering/evidence/verification.json).
- **Rerun exact revision…** creates a new source-bound attempt at the selected
  immutable revision. **Run latest admitted reference…** resolves the current
  reviewed reference into a new candidate. **Retry known failed nodes…** selects
  the failed/downstream subgraph and requires an exact registered provider route;
  missing subset support is rejection, never a full-graph fallback or success.
  Actions require explicit confirmation and fresh selected-run facts.
- **Automatic failed-node retry:** a project may set `retry_failed` (0–3, default
  0). When a run ends `DomainFailure`, `Denied`, `TimedOut` or `ProviderError`,
  the service issues the same `failed` rerun on the exact same source, with the
  deterministic request identity `auto-retry-<ordinal>-<parent run>`. Each retry
  child stays untrusted when the parent was untrusted, keeps the parent link,
  and never rewrites the parent's failed outcome. Unknown node states, unknown
  or superseded untrusted gates, unresolved dirty snapshots and request/run
  retention limits refuse creation without changing the parent. The chain stops
  after `retry_failed` automatic attempts. Because the child is a partial-node
  run, it is never a full clean run and cannot satisfy required CI for the
  revision; the Tests & flaky view counts its per-test pass/fail observations.
  A passing child is labelled "failed, then passed on retry" and never shown as
  a green parent.
- **Tests & coverage:** reviewed worker steps may declare relative `reports`
  with `kind` `junit`, `cobertura`, `lcov`, `gate`, `review` or `provenance` and a
  `path`. Original profile bytes remain pinned; adding declarations requires a
  newly reviewed profile and exact provider route. Raw retained reports bind
  source, attempt, execution and node; malformed/stale/oversized uploads fail.
  Missing reports are unavailable, not passing tests or full coverage.
- **Compare & flaky:** compares retained outcome, duration, test status and
  coverage facts. Repeated-test instability requires actual pass/fail history
  for the same exact source revision, pipeline, node, report and test identity;
  a changed source alone never becomes a flaky-test claim.
- **Activity** distinguishes canonical events from attributable admission audit.
  **Evidence** exposes exact source/pipeline/execution/artifact linkage and
  recorded gate/review/provenance documents. Expand recorded-facts groups to
  inspect native values. A document's verdict is not accepted gate authority,
  review acceptance, source trust or release readiness.
- **Operations:** provider observations, durable queue order, admitted execution
  capacity and retained usage. A near-limit warning precedes intake exhaustion;
  it does not prune state, promise placement or convert unknown provider state
  into healthy capacity.

#### Agent, worker, compute and serving observations

The optional `--inventory INPUT` service argument enables authenticated
`GET /api/inventory`. The operator supplies bounded normalized observations and
an explicit principal/source/generation policy; this endpoint neither discovers
providers nor authenticates raw remote producer messages. Unconfigured input is
503, not an empty or healthy fleet. Worker-scoped credentials cannot enumerate it.

The Workers page keeps four distinct families: agent definitions, actual worker
instances, compute devices/shared resource pools, and model-serving services.
Configured, healthy, idle, reserved and executing are independent facts. A shared
pool is counted once; references to it do not add capacity. Missing, stale,
partial, rejected and unsupported producer facts retain their reasons. Existing
`/api/workers` remains the configured CI admission-slot projection described below.

`GET /api/inventory?work=WORK_BLAKE3` joins only the canonical current work's
immutable requirements attachment. Configure `--implementation-work DATABASE`
for that read-only lookup; arbitrary query-supplied needs or filesystem paths
are not accepted. Missing requirements stay unknown, and stale work/attachments
are refused. Explanations are not dispatch grants: the actual consumer rechecks
work, attempt, fence, policy and exact owner grants before transport.

For an offline projection of the explicitly synthetic example:

```console
cargo run --locked -p kiln-web-service --example inventory -- fixtures/inventory/synthetic-v2.json operator 2000 json
```

Use `html` for escaped script-free output. Equal admitted inputs, requirements,
principal and time share the same projection identity across CLI, HTML and API.
The example performs no provider discovery and does not qualify a real fleet.
Kiln core/provider/web maintainers own the contracts and consumers; qualified
Lattice, Aspen/Molten and Animus observations remain independent prerequisites.

To build the input from producer observations instead, write a
`kiln.inventory.collection-set.v1` manifest naming the retained Lattice, compute
allocation and Animus serving documents, and run
`kiln-web-service inventory-collect MANIFEST PROFILE`. It reads only the named
files, and its output is the input file above. See
[inventory producer collection](../inventory-producers.md) for the mapping, the
facts each producer leaves unknown and the producer consumer contract.

#### Exact artifact browser previews

Review can project an explicitly configured `--artifact-previews CATALOG` only
when its exact source, currently selected successful run and retained canonical
product record all agree with the descriptor. A check digest is not a product
path or launch URL. The preview module verifies the full runtime closure, native
NAR hashes and required trusted signatures before export. Missing, moved, stale
or incomplete evidence is a preview-only failure.

The loopback launch path binds both an independent secret capability token and
the exact descriptor identity. Knowing the public descriptor identity does not
grant launch access, and a capability for another descriptor is not admitted.

The opt-in `run-build` action creates a new exact-source canonical Engine run
whose sole node is the native Nix BuildPort. Its profile and node identity come
only from the trusted service profile's `canonical_nix_build` configuration;
the Engine constructs the operation-bound `BuildEffectPlan`, records the run and
plan before dispatch, then retains the provider's actual observation and maps
that result through normal workflow admission and journal transitions. It never
adds a build to a completed workflow run. The configured Nix derivation and
outputs must be the reviewed source-bound build for the selected source run.

The separate `produce` action is read-only over that retained canonical Build
run. It accepts no build profile, plan or observation JSON; it admits only the
profile-bound plan and actual retained product/store observation. The selected
root must be an exact required product with a complete verified runtime closure.
Caller-authored observation/current-binding JSON and check digests never grant
build or launch authority.

Both commands use the existing canonical Engine store lock; stop the service first.

The producer releases the canonical store lock after validating and preparing the
retained descriptor, before its disposable preview server begins serving; the
review service can then reopen the same canonical store while that preview is
live.

Example invocation:

```console
cargo run --locked -p kiln-web-service --example artifact-preview -- run-build \
  SERVICE_PROFILE SUCCESSFUL_SOURCE_RUN_ID UNIQUE_REQUEST_ID
cargo run --locked -p kiln-web-service --example artifact-preview -- produce \
  SERVICE_PROFILE CANONICAL_BUILD_RUN_ID BUILD_NODE SELECTION.json \
  PREBUILT_TRYNIX_TREE ENGINE_MANIFEST.json ABS_NIX ABS_NIX_STORE \
  OWNED_SCRATCH NEW_CATALOG.json
```

The producer uses an existing content-pinned TryNix Linux engine on a separate
disposable loopback capability origin, never the authenticated console origin.
Payload, output, starts, requests and expiry are finite; normal shutdown removes
owned exports/catalogs. Its `cleanup` command reconciles expired owned leases
after crashes. The selected deployment must schedule that cleanup before
claiming finite crash retention. The native Build run remains a separate
selected-node run and is excluded from full-CI selection. Review derives its
displayed CI run from the validated parent source run while retaining the
descriptor's exact Build run and attempt. Only a matching retained build record
for the exact currently selected successful source CI run appears on review. The browser
receives no repository secrets or build-only inputs. Preview failures remain
separate observations and never revise CI, review or merge evidence.
For engine-only rehearsal, `fixtures/artifact-preview/engine-cohort.ncl` selects
one already-built signed static CLI and requires an explicit finite Unix expiry.
`artifact-preview engine-cohort` verifies and executes that exact selection;
it publishes no review catalog and invents no CI run, candidate or build receipt.
Discovery, hydration, guest boot and program readiness remain separate in
`window.kilnPreviewObservation`. Clean/warm engine evidence cannot qualify an
absent canonical build binding. Unsupported browsers, engines, network guests
and Redox remain explicit refusals; previews never alter CI/review/merge evidence.

The retained [clean/warm engine-only observations](../../.cairn/archive/1970-01-01-add-exact-artifact-previews/evidence/browser-verification.json)
record the exact signed CLI and all four phases in an initially empty isolated
browser context. Each launch resets its own observations; cancellation and
expiry remain explicit preview-only results.

The retained [canonical CI-bound browser cohort](../../.cairn/archive/1970-01-01-add-exact-artifact-previews/evidence/canonical-browser-verification.json)
launches the exact Nix build product of the review's selected successful CI run
from the real review action. It records the producer and review executables,
the admitted descriptor and launch capability shape, clean and warm launches in
an isolated browser context, refused capability/descriptor substitutions,
unchanged review state and removed catalog/scratch after shutdown. It is a
single-host local cohort, not production or hostile-isolation qualification.

<!-- r[impl kiln.preview.comparison] -->
To compare preview evidence, append an owner-controlled directory (owned by the
producer's user, not group- or world-writable) as the producer's optional
`OWNED_OBSERVATIONS_DIR` argument. After each launch the preview page posts
only its phases and bounded result to its own loopback origin. The server
refuses foreign origins, non-JSON or undeclared-length bodies, reports over
64 KiB, unknown, repeated or out-of-order phases, unknown outcomes and output
beyond the descriptor limit. It composes a `kiln.preview.observation.v1` record
from the admitted descriptor, so the browser never supplies the binding, engine,
platform or invocation, and names the record by a domain-tagged BLAKE3 identity.
Records expire after 30 days. A directory holding 256 records refuses new ones
instead of evicting; `artifact-preview cleanup-observations DIR` removes expired
records. Engine-only rehearsal retains nothing.

Start the service with `--preview-observations DIR` to read that directory.
Review re-verifies each record's owner, mode, size, schema and identity; one
tampered record withholds every observation for the change, and the error stays
preview-only. Review projects up to 32 unexpired records whose project,
repository and revision commit match the change. A record is current only when
it binds the current revision commit and a retained canonical build record
derived from the selected successful CI run; every other record is historical.

`GET /api/code-review/changes/<id>/preview-comparison?baseline=<identity>&candidate=<identity>`
returns a `kiln.preview.comparison.v1` body. Its identity is derived from both
record identities and the `kiln.preview.comparison-rules.v1` rule version. The
body lists every engine, platform, argument or executable difference, reports
`same_observation` for a record compared with itself and states whether exit
status and output match. A malformed identity returns
`400 preview_comparison_subject_invalid`. An unknown, expired or unprojected
subject returns `404 preview_baseline_unavailable` or
`404 preview_candidate_unavailable`. A historical candidate returns
`409 preview_candidate_not_current`. No other record is substituted.

The review page restores a pair from
`review_preview_compare=<baseline>~<candidate>` and shows both exact identities,
phases and bounded outputs side by side; narrow review panes stack the lanes.
Responses for an earlier pair or a moved candidate are ignored, and the baseline
lane never offers a launch. Observations are untrusted browser reports. They
never change CI, review, merge or implementation-completion state.

The retained [comparison browser cohort](../../.cairn/archive/1970-01-01-compare-preview-evidence/evidence/browser-comparison.json)
launched the exact canonical CI-bound artifact twice, retained both reports and
compared them through the review page by form and by deep link at narrow and
wide widths. Forged report submissions and forged retained records were
refused, and review state was unchanged. Both observations bind the current
revision, so a historical baseline and a moved candidate are unit-tested only.

<!-- r[impl kiln.preview.structural] -->
Structural source evidence is off by default. Add `structural=1` to the
comparison read, or tick **Structural source evidence** in the comparison form
(`review_preview_structural=1`), to add a `kiln.preview.structural.v1` record.
Any other value returns `400 preview_comparison_query_invalid`, and without the
parameter the response is unchanged. The service compares the two observations'
exact revision commits with exact whitespace and the default diff limits, and
passes the result to the same per-file function as the structural review mode.
For the same commits, each file therefore carries the rows or the named
fallback that `base=<baseline revision>&mode=structural` shows. No grammar runs
in the browser.

##### Related work: `pithings/unkernel` (preview-engine comparison only)

[unkernel](https://github.com/pithings/unkernel) (pinned at
`83c65c3cb496737390ed002eceac18de0db1f6a8`) compiles Linux 7.3.4 itself to
WebAssembly and runs unmodified AArch64 user space on a Zig user-mode CPU
(`kwemu`). The pinned TryNix QEMU-WASM engine (`trynix-startvm-v1`) remains the
only supported browser preview engine in Kiln.

- **Not a job-isolation backend:** unkernel is not a Kiln `VmmBackend` or
  hosted runner and must not be used for Firecracker, Cloud Hypervisor, QEMU,
  Unikraft, untrusted pull-request, fleet/Herdr worktree, or verifier
  execution. Hosted no-KVM fallback stays on `kind = "qemu"` with
  `acceleration = "tcg"`, and untrusted pull-request runs require
  `kind = "firecracker"` with `untrusted_capable = true` and host confinement
  (seccomp/Landlock/no-new-privs).
- **ISA, network, and license boundaries:** Kiln's preview products today are
  `x86_64-linux` whereas `kwemu` targets AArch64 user space only; Kiln previews
  enforce `network_access: false` (unkernel's optional Wisp network is not
  admitted); and unkernel splits licenses between GPL-2.0-or-later /
  GPL-2.0-only (kernel, patches, `kwemu`) and MIT (JavaScript SDK).
- **Precondition for any future spike:** No preview, CI, review, or merge
  behaviour changes here. Any later engine spike must be a separate default-off
  issue with its own protocol, started only after the ISA precondition is met
  (either Kiln admits `aarch64-linux` preview products or an `x86_64` user-mode
  WASM CPU exists) and measured against TryNix clean/warm boot times.

The record binds the comparison identity, both commits, its settings and every
file's outcome, plus fixed display-only provenance, under a domain-tagged BLAKE3
identity. The settings are the preview and engine rule versions, the bounds,
the deadline, the diff limits and a grammar identity. That identity covers the
package, version and `Cargo.lock` checksum of the Tree-sitter runtime, the
highlight engine and every grammar; a `kiln-highlight` test fails when the pins
drift from the lock file. An incompatible comparison yields `incompatible` with
its reasons and no files, because unlike runtime cohorts are never compared as
if only the source changed. A provider that cannot read the revisions yields
`unavailable` with `unsupported_provider` or `source_unavailable`. A record over
2 MiB yields `unavailable` with `evidence_exhausted`. Records are computed per
request and not retained. They describe source structure only: they change no
CI, review approval, merge or completion state and measure no review time.

The frozen fixture in `crates/kiln-web-service/tests/fixtures/structural-preview/`
holds a pure reformat, a moved unchanged function, one edit, a parse error, a
file without a grammar and a binary file. It retains its settings and expected
outcomes, and a route-level test requires the preview record to equal structural
mode for the same commits.

#### Worker target inventory

Authenticated viewers and operators can read `GET /api/workers`; worker-scoped
credentials cannot enumerate the control plane. The endpoint accepts no query
or body. Successful reads are immutable projections bounded by the existing
project/run limits; they do not contact providers or add persistence.

Targets group the exact provider-profile, workflow endpoint and platform binding.
Kiln-owned versioned BLAKE3 IDs remain stable across project ordering and public
label changes. Endpoint paths, repository paths, credentials, UID policy and
executable arguments never enter the response. The pinned Lattice adapter still
admits only `linux-x86_64`; this view grants no additional provider capability.

Each enrolled project supplies one admission slot, not a physical worker or CPU.
A nonterminal run outside the queue occupies its slot, including ambiguous
executions. A shared target with a free enrolled slot reports **Slots available**;
without free slots it reports **Unknown execution** for ambiguous work or **Busy**
otherwise. Unenrolled targets report **Not enrolled**. Free slots use a neutral
badge rather than a health-success indication; busy is blue, and uncertain
execution or an expired heartbeat is amber. The page and Overview share this
vocabulary. Failed durable storage returns unavailable (503), never usable
capacity. A queued or cancelled-before-dispatch request does not replace the
latest execution observation; missing recorded sequence or timing stays unknown.

<!-- r[impl kiln.dashboard.workers.view] -->
Workers opens with a three-fact readout of occupied project slots, free slots
and queued runs, followed by a full-width **Execution targets** register. Targets
sort by readable project names, with exact binding/ID tie-breakers. The rows keep
provider/platform metadata secondary and show slot state, heartbeat evidence,
queue and the latest retained execution together. They stack by their own
region width rather than overflowing on a narrow screen.

**Search targets** filters project names/IDs, provider profiles, platforms and
target IDs as you type. **Slot state** includes **No current heartbeat** for
offline targets, whether no admitted report has arrived or the retained report
is no longer current. These
are local filters over the already bounded inventory: typing does not issue an
inventory read per keystroke. `worker_q` and `worker_status` are committed to the
URL after a short pause; reload and browser history restore them. Clear filters
does not clear the exact target selection. Arrow keys move between visible
target links.

Selecting a target opens its readable detail: project name and state, admission
capacity and latest execution, heartbeat facts, then assigned projects and
occupied slots. Project links open CI project histories; execution links keep
the exact retained run ID. **All targets** returns to the register and restores
focus to the selected row, or to search if the filters hide it. **Browse targets**
keeps the register and filters available while the detail is open. A filtered
selection remains selected, and a missing target is never replaced. **Exact
target binding and snapshot** retains the full copyable target ID, provider,
platform, counts and snapshot version. Refresh preserves selection and open
disclosures. Clearing the session erases inventory and invalidates delayed reads.

Heartbeat evidence is separate from admission slots. Monitoring configured before
any admitted report still reads **Not observed**, not expired. A retained
observation shows its admitted time and API-projected snapshot counts; those
counts are not a copy of the last producer heartbeat. Only an online observation
supplies **Current observed free**. A noncurrent report instead says **No current
heartbeat**, and any reported free count is labelled **Snapshot free (not
current)** rather than presented as availability. The raw liveness value remains
in the exact-binding disclosure. Neither free admission slots nor a previous
passing run establishes worker health, placement or guaranteed dispatch.

Kiln web-service/dashboard maintainers own this view. Repeat its behavioral rail
with `cargo test --locked -p kiln-web-service --test engine workers::` in the
development shell. For runtime evidence, enroll an isolated project, observe a
real active and queued run, then terminal release; inspect Workers on desktop
and mobile, restore its URL, follow the exact run and clear the session.
Candidate-local observations are retained under
`.cairn/archive/1970-01-01-add-dashboard-workers-view/evidence/`; they grant no deployment,
release or lifecycle acceptance authority.

The presentation helpers also run in the Node rail:
`node --test tests/live-console-ui.test.mjs tests/kiln-identity.test.mjs tests/work-views.test.mjs`.
The [Workers UX verification](../../.cairn/changes/clarify-workers-page/evidence/verification.json)
records real assets over read-only live data plus explicitly synthetic edge
snapshots. It is not a deployment, a worker execution or lifecycle acceptance.

#### Registered automation

Only server-reviewed registrations appear in **Automation**. Operators may
enable/disable declared sources/destinations, choose allowed push/PR categories,
adjust schedule intervals within reviewed bounds and select admitted notification
outcomes. Browser requests cannot add repository paths, credentials or URLs.

Signed push/PR intake verifies the exact repository/reference/current revision,
signature and replay identity before run reservation. Schedules reserve durably,
skip missed intervals rather than burst unbounded catch-up work, and preserve
their next due time across restart. A persisted round-robin selection gives due
schedules and pending notifications fair turns within `max_tick_effects`.

HTTPS completion/failure delivery reserves before sending and retains delivered,
rejected or ambiguous observations without blind resend. Bounded serial delivery
runs outside the engine mutex, keeping API/worker ingress available while a
receiver waits; the single poll executor resumes workflow/schedule polling after
the delivery batch. Configure notification timeouts and effect budgets with that
poll-delay bound in mind. Credentials/destinations never enter the page. No
declared trigger or notification means no invented subscription or destination;
the current operator owns enrollment and delivery policy.

##### Trigger filters

A trigger binds an exact `refs/heads/*` or `refs/tags/*` reference. Tag-bound
pushes require the tag in the project's admitted references; pull-request
triggers remain branch-bound, and schedules remain branch-bound. For example,
`reference = "refs/tags/v1.0.0"` with `push = true`, `pull_request = false` and
`filter = { tags = ["v*"] }` is a reviewed release-tag trigger.

A trigger may add an optional `filter`:

- `branches`: globs that match the pushed branch, or a pull request's base branch.
- `tags`: globs that match a pushed tag. Tags are push-only and cannot combine with pull-request triggers; a tag-bound trigger must admit its exact reference.
- `types`: pull request actions (`opened`, `reopened`, `synchronize`, `closed` or `merged`). An empty list keeps today's `opened`/`reopened`/`synchronize`.
- `paths`: globs that match any changed path, including a rename's old path.

Globs use the shared review and scope matcher: `*` and `?` stay within one `/` segment, and a whole `**` segment spans directories. Negation, character classes and alternation are rejected. A branch-bound trigger rejects a `tags` filter and a tag-bound trigger rejects a `branches` filter, because each registration binds one exact reference.

With `branches`, a pull-request trigger may omit `pull_request_base` and admit every base that the globs match. Listing `closed` or `merged` admits closed pull requests for that repository; it is the only way they ever run.

Changed files come from one bounded diff between the exact base and candidate. Projects scopes reuse the same diff. When changed files are unknown (no base, a truncated diff or a read error), the trigger runs, with reason `paths_unknown`.

Monorepo project scopes still decide which projects are affected; trigger `paths` is a second, per-trigger filter. A source record shows what happened:

- `filtered`: the trigger excluded the event, recorded with `filter.excluded_by`.
- `skipped`: the project scope excluded the event.
- A queued run records `filter.reasons` (matched patterns).

A filtered run never satisfies required CI. A review policy may declare `optional_ci = [{ project, paths }]`; a missing required check is then waived only when every exact changed file of the change matches those paths. A failed run still blocks.

##### Trigger supersession

A trigger may set `supersede_queued = true`. After a new dispatch succeeds,
older runs from the same trigger with a different exact revision are
superseded: a push matches its project and requested reference, while a pull
request matches its project and pull-request number, so another pull request on
the same base is never cancelled. Manual and scheduled runs are never matched. Queued runs complete as `Cancelled` through
durable queue control, and active runs receive one exact provider cancellation
through the admitted control path. Awaiting-approval and terminal runs are untouched.
Provider refusals are best-effort and never fail the new dispatch. Superseded
runs expose `error_code = "superseded"`; an unknown queued-cancel reason is
rejected without mutation. Absence or `false` keeps the previous behavior.

##### Calendar schedules

A schedule has either `interval_ms` (unchanged) or `cron = { expression, timezone = "UTC" }`, with `next_due_ms = null`.

- **Expression:** five fields with numbers, ranges, lists, steps, and month or day names. `H`, `H(a-b)` and `H/n` resolve from a BLAKE3 hash of the project and schedule ids. They are stable across restarts and change only with those ids. The automation view shows the resolved expression.
- **Time zones:** come from the pinned bundled IANA database (tzdb `2026c`), never from host zoneinfo.
- **DST:** a nonexistent local time fires once, shifted by the gap. A repeated local time fires once, at its first instant.
- **`min_interval_ms`:** must be at least 60000. Policy load rejects entries whose occurrences come closer than it within 400 days, and every reservation checks it again (status `schedule_min_interval`).
- **Missed occurrences:** an occurrence that passed by more than its minimum interval, for example during downtime, is `missed`. It never runs; the schedule moves to the next occurrence after now. Keep the service poll interval below the minimum interval.
- **Fired occurrences:** reserve durably, resolve the schedule reference to its exact revision at that moment, and record it on the source.

Operators may only enable or disable a cron schedule; enabling resumes at the next occurrence.

##### Untrusted pull requests

<!-- r[impl kiln.untrusted.policy] r[impl kiln.untrusted.console] -->
A trigger's `repository.untrusted_pull_requests` is `deny` (the default, also when omitted) or `approve`. `deny` refuses fork pull requests exactly as before (`github_fork_denied`) and trusts same-repository pull requests; existing profiles, fixtures and journals replay byte-identically. The retired `allow_fork_pull_requests` stays readable and must be `false`.

With `approve`, a pull request is untrusted when its head repository is a fork (`fork`), or when its signed `author_association` is not `OWNER`, `MEMBER` or `COLLABORATOR` (`author_without_push_access`). An absent association is untrusted. `MEMBER` means organization membership, which usually but not always implies push access; keep `deny` where that matters.

- **Blocked run:** after trigger filters and project scopes select it, an untrusted event creates a durable run in phase `awaiting_approval`, bound to its exact commit. Nothing dispatches. A new commit on the same pull request creates a new blocked run and supersedes older unapproved ones (`Cancelled` with `untrusted_superseded`). Cancel removes a blocked run before dispatch.
- **Approval:** use **Approve run…** in the run header, which opens the shared confirmation dialog, or `POST /api/runs/{id}/approve` with `{request_id, source_commit}`. Only an interactive operator approves: the operator bearer token, or a network identity bound to the operator role. Viewers (`viewer_read_only`) and workers (`worker_scope`) are refused by the role rules; webhook and automation principals, which hold operator capabilities internally, get `untrusted_approval_unauthorized`. A commit other than the run's exact commit gets `untrusted_approval_stale_commit`; superseded and already approved runs are rejected.
- **Execution:** approved runs dispatch only for projects declared `untrusted_capable = true` in the service profile (`untrusted_runner_unavailable` otherwise). The declaration states that the project's execution target runs each job in a microVM without credentials or cache; Kiln cannot verify that isolation. Workers receive the binding with `untrusted` and refuse it unless their profile also declares `untrusted_capable = true`. Untrusted runs never restore or publish cache: the cache endpoints answer `untrusted_run_cache_denied`, and workers skip cache steps.
- **Retries:** exact and failed-node retries keep the gate and its approval, because the commit is unchanged; latest-source retries are refused (`untrusted_retry_requires_exact_source`). A retry never supersedes another commit's blocked run. There is no per-run override.
- **Review:** approving a run is not a code-review approval. It never satisfies approvals, ownership, veto clearing or queue admission. An untrusted result satisfies required CI only for its exact approved commit, and only when the trigger sets `untrusted_results_satisfy_required_ci = true`, which is valid only with `approve`.

Run detail and the change-review `ci` checklist item show the state (`awaiting_approval`, `approved`, `superseded`), reason, approver, approved commit and whether the result counts for required CI. The standalone [GitHub CI adapter](ci-and-providers.md#github-ci-adapter) and hosted GitHub connections admit only `deny`.


### Internal developer loop

The service/worker maintainers own these optional contracts. Operators own their
reviewed profiles, credentials and host activation. Existing profiles omit the
new fields when unused; installing a new worker command still requires its exact
reviewed workflow operation, not an edit to retained identities.

The [corrected native developer-loop receipt](../../.cairn/archive/1970-01-01-complete-internal-developer-loop/evidence/native-07/verification.json)
records an actual four-node branching workflow, five measured steps, partial
failure, isolated cancellation/recovery, expiring worker observations, verified
cross-worker cache reuse, and committed/dirty CLI execution. The preceding
[viewer-browser receipt](../../.cairn/archive/1970-01-01-complete-internal-developer-loop/evidence/native-06/verification.json)
retains desktop (1440px) and mobile (390px) observations and screenshots. Both
trials retired all disposable processes, credentials and scratch. They used two
worker processes/work roots on one host, not two physical hosts. The corrected
trial's cold/warm build steps measured 744ms/870ms: reuse passed, but the warm
step was slower, so there is no performance-improvement claim.

The ownership-lifetime regression and full Cargo workspace pass after explicit
state-lock release. The Nix process fixture now uses non-core-dumping SIGKILL
and requires exactly that signal under its unchanged deadline; its focused
three-test rail passes. Earlier failures remain historical evidence, not current
passing receipts. The original notification failure did not recur under syscall
tracing; retained-description lock behavior was reproduced deterministically,
not inferred from a successful rerun.

For declared development coverage, explicitly select the optional
[`kiln-active-development` profile](../../cairn-policy/development-coverage.ncl).
It includes the selected pending deltas and owned browser sources; it does not
change accepted-only release defaults or accept any change. Unknown anchors
still fail its negative control. Signed-population and installed-project links
point to the scoped retained observations above, not procedures marked as
implementation. See the [qualification results](../../.cairn/archive/1970-01-01-complete-internal-developer-loop/evidence/qualification.json).
No lifecycle acceptance, production activation or release-readiness claim follows.

#### Pipeline measurements and worker capacity

The run inspector shows admitted per-node step measurements and retained reports.
Elapsed duration uses the worker's monotonic clock. CPU usage measures the actual
command and its waited descendants; Linux peak RSS is the maximum child-process
peak, not aggregate concurrent memory or a cgroup reservation. Isolated commands
are accounted inside the namespace, outside the command's resource limits.
Cancellation or timeout can interrupt accounting; unavailable measurements remain
unknown. Historical runs do not acquire invented measurements.

Export `config/web-workers.ncl` and pass its JSON as `--workers` to both `check`
and `serve`. Each admitted worker has an exact principal, project, target and
maximum slot count. Derive the target before installing heartbeat authority:

```console
kiln-web-service worker-target --provider-profile "$PROVIDER_PROFILE_ID" \
  --socket "$LATTICE_SOCKET" --platform linux-x86_64
kiln-web-service heartbeat --profile "$WORKER_PROFILE" --identity "$WORKER_PROFILE_BLAKE3"
```

The worker profile's optional `heartbeat` config controls its stable worker ID,
target, interval, TTL and slots. Run one daemon per worker ID/private work root.
Independent workers may share one admitted project/target. Kernel-held job locks
and available CPU parallelism bound observed slots; process exit releases locks.
Heartbeats have authenticated sessions, strictly increasing observation sequences
and explicit expiry. Transport loss can skip observations without replacing the
session; policy rejection fails closed. Duplicate observations never renew TTL.
Offline workers contribute no observed available capacity. Configured project
slots remain separate from observed worker liveness and are not placement proof.

Optional worker `execution` selects the exact reviewed bubblewrap executable,
read-only toolchain mounts, optional read-only data mounts under reviewed
roots, network policy and per-command resource limits.
Preflight failure never falls back to account-authority execution. The command
gets its private writable checkout/temp and explicit environment, not service
credentials or the worker's repository backing store. This is bounded namespace
isolation, not a claim of hostile-code confinement or host-wide resource control.

#### Shared cache

Export `config/web-cache.ncl`; pass `--cache` to both `check` and `serve`. Its
private cache directory must not overlap canonical state. Policy bounds entries,
bytes, files, age and admitted project/producer/toolchain/platform/output paths.
Each cacheable worker step explicitly declares its inputs and output paths.
Compatible workers share keys based on those declarations and actual BLAKE3 input
hashes, not their local checkout, token or work-root paths. Producer/toolchain
identities must describe compatible commands and environments; a hit is not a
semantic correctness guarantee.

Restore verifies the entire admitted envelope and safe paths before writing.
Publication follows actual successful execution and admitted output ingress;
conflicting immutable content, corrupt bytes, links and credential paths fail
closed. Retention only prunes verified cache members, never journals, source or
run artifacts. The inspector reports actual miss/hit/publication, transferred
bytes and duration. Compare measured cold/warm runs; do not infer a speedup.

#### Developer CLI and explicitly selected dirty inputs

Export `config/web-developer.ncl` with an HTTPS origin, trusted CA, private token
file and finite request/watch bounds. Pin BLAKE3 of the original exported JSON
bytes, not a reserialized object. Submit an exact committed reference:

```console
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" \
  submit --project "$PROJECT" --request-id "$REQUEST_ID" \
  --reference refs/heads/main --revision "$BASE_REVISION"
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" watch --run "$RUN_ID"
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" logs --run "$RUN_ID" --follow
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" cancel --run "$RUN_ID" --request-id "$CANCEL_REQUEST_ID"
kiln-web-service ci --profile "$DEV_PROFILE" --identity "$DEV_PROFILE_BLAKE3" retry --run "$RUN_ID" --request-id "$RETRY_REQUEST_ID"
```

For dirty input, add `--dirty-repository "$WORKTREE"` plus repeated
`--include relative/file` and/or `--delete relative/file`. Only explicitly selected
changes are uploaded. The token must be outside that worktree. Unsafe, ignored,
linked or stale inputs are rejected. Submission binds a BLAKE3 manifest, exact
base revision and deterministic synthetic Git revision; it neither publishes the
source nor relabels the base commit as the executed tree. Independent backing
repositories reconstruct the same admitted bytes, without modifying the user's
index or worktree. Exact retry preserves snapshot provenance in a new run.

The selected Git worktree's `HEAD` must equal the supplied base revision during
capture. In a colocated Jujutsu fixture, create a worktree child of the admitted
bookmark (`jj new main`) before capture; the bookmark itself stays fixed.

Mutations are not blindly retried. Watch preserves success/failure/cancellation/
unknown outcome exits; logs retain real stream bytes and cursors. These commands
grant no source publication, release acceptance or remote mutation authority.

### Repeat the isolated native exercise

From the repository development shell, supply a compatible, exact measured
Lattice executable and matching source cohort with real shell-success semantics.
Ensure `bwrap`, `bash`, `jj`, `openssl`, `nickel`, `b3sum`, Python and user namespaces
are available. Scratch must be a new owned directory outside the producer's
private `/tmp`:

```console
cargo build --locked -p kiln-web-service
python3 scripts/web-service-smoke.py \
  --service "$KILN_SERVICE_EXECUTABLE" \
  --lattice "$LATTICE_EXECUTABLE" --lattice-source "$LATTICE_SOURCE" \
  --root "$KILN_NEW_EXERCISE_ROOT" --evidence "$KILN_EXERCISE_EVIDENCE"
```

`--browser-hold` additionally waits for actual browser observations in the owned
scratch `browser-done.json` after API checks and before resource retirement. The
native `passed` flag covers the API exercise; inspect `browser.passed` separately.
The rail creates a proper isolated CA and server leaf, disposable exact Git source, real reviewed commands/native
workflows and private credentials. It checks successful and failed execution,
multi-chunk output across repeated steps, empty streams, native diagnostics,
artifact bytes/hash, role/origin/TLS rejection, replay, restart, stale/unsupported
controls and canonical cancellation. Failed checks remain failures. It retains
measured executable/profile/source identities and observations, then removes its
processes, private keys, credentials and scratch tree.

The completion-notification restart check compares exact notification
idempotency identities at the real HTTPS receiver. An unrelated pending delivery
may finish during restart; total receiver traffic alone is not replay evidence.
Long activity identities wrap within record cards on narrow displays.

The complete-dashboard observations, including retained failures and corrected
deep-link/mobile browser checks, are under
`.cairn/archive/1970-01-01-complete-live-dashboard/evidence/`. Their immediate consumer is
Kiln self-CI; dashboard/service maintainers own the reusable rail and operators
own host activation and separately admitted access/automation policies.

The separate [local activation receipt](../../.cairn/archive/1970-01-01-complete-live-dashboard/evidence/deployment.json)
records the supervised tailnet deployment, genuine tokenless operator identity,
log-follow cursor progression, byte/hash-verified retained-log download and
desktop/mobile observations. Original profiles, all four retained runs, eight
retained log members and unrelated tailnet routes were preserved. The listener
continues to use the original private deployment CA; clients must trust it.
The [live-response diagnostic](../../.cairn/archive/1970-01-01-complete-live-dashboard/evidence/live-completion-diagnostic.json)
retains Chromium cancellation events alongside completed finite snapshot and
heartbeat readback; it does not claim an empty browser diagnostics log.

The [verification record](../../.cairn/archive/1970-01-01-complete-live-dashboard/evidence/verification.json)
retains the broader Radicle mutation timeout failure and nine dangling
traceability IDs. Those failures remain release holds, not waived checks.

The isolated exercise proves only its recorded cohort, not host deployment.
Neither that exercise nor the local activation establishes hostile-code
confinement, source trust, arbitrary provider correctness, lifecycle acceptance
or release eligibility. Qualification does not transfer to changed executables
or profiles.

### Optional interactive change review

The **Code review** page owns human change reviews, not retained Patchgate
receipts or generic work COBs. Existing work, source and receipt routes remain
available. The initial concrete collaboration provider is separately admitted
local Git; source-browser configuration remains read-only and grants no writes.
No remote hosting, publication, signing or Radicle mutation is implied.

#### Configuration and startup

Author a Nickel input using `config/web-code-review.ncl`; the paired positive
and negative inputs are under `config/fixtures/*web-code-review-*.ncl`.
The exported `kiln.code-review-profile.v1` declares:

- `database`: an absolute SQLite path in an already existing, service-owned
  private directory. Database, journal and lock files must be owned regular
  mode-0600 files; symbolic links and replaceable ancestor directories fail closed.
- `project` and `provider.repository_path`: one existing service project and its
  same canonical local Git repository. `repository_id` is BLAKE3 over that
  canonical absolute UTF-8 path, with no trailing newline. Git commit identities
  retain Git's native algorithm.
- `provider.target_refs`: explicitly admitted `refs/heads/` targets, also present
  in the service project's admitted references. `capabilities` separately enables
  inspect, compare, register, submit, rebase, revert, cherry-pick,
  `apply_suggestions`, `build_integration` and `speculative_integration`
  (building a candidate on an assumed predecessor candidate that descends from
  the exact target). Unsupported operations fail before provider effects.
- `read_principals` and `write_principals`: exact authenticated IDs, such as
  `local:operator` and `viewer:reviewer`, not wildcard grants or display names.
  Application-review writes do not grant access to CI operator routes; Worker
  credentials never grant review access.
- `agent_principals`: exact write principals whose approvals are agent votes.
  They are bounded, unique, and must also be write principals.
  `policy.agent_votes` is `advisory` by default: agent approvals are displayed
  but never satisfy the ordinary approval count or an ownership item.
  `binding` is an explicit policy opt-in that counts agent approvals exactly
  like human approvals. Agent `NeedsRevision` and `Veto` votes keep their
  existing blocking effect in either mode; no merge authority is granted. The
  review detail marks agent reviewers and votes and reports the effective
  advisory or binding mode. The review snapshot exposes the declared agent
  principals and metadata, and the console policy card labels them as declared. Optional
  `agent_metadata` maps these principals to declared, trimmed
  `{ harness, model? }` metadata; a key must name a declared agent. Access viewer declarations are
  merged into both fields at startup, and conflicting metadata fails startup.
- `session_traces`: optional Boolean, absent/false by default. Enables capture of
  untrusted local `refs/notes/traces` pointers when a new review revision is
  constructed; it adds no provider mutation or remote access authority.
- `policy`: named eligible reviewers and submitters, nonzero required approvals
  and explicit `require_ci`. Owner approval, ineligible votes, stale votes,
  unresolved blocking threads, WIP and unmet dependencies cannot authorize submit.
- `signature_name` and `signature_email`: the service's identity for provider-created
  maintenance commits, not an attestation of the original author's signature.
- `policy.ownership`: `teams` maps names to exact eligible review principals;
  `rules` declares IDs, path patterns, owners, teams and optional target-ref
  scopes. Every matching rule needs a current requested non-author owner vote.
  Optional `required_evidence` adds named exact-revision checks, currently
  `{ kind = "project_ci", project = ... }`: a full clean run for that project at
  the exact submitted commit and its current scope identity must pass in
  addition to the owner vote. Evidence never grants merge authority. Optional
  policy-wide `classes` declare ordered path sets; a change belongs to the
  first class whose patterns match **every** current and old changed path,
  otherwise the reserved `logic` class. A rule may map a class to
  `evidence_only` to make its required evidence replace that rule's owner vote;
  absence means `owner_approval`. `evidence_only` needs at least one evidence
  requirement and is rejected for undeclared classes. Missing or incomplete
  changed-file facts fail ownership evaluation before classification. Ordinary
  approvals, vetoes, blocking threads, CI and merge preflight still apply.
  A rule may name a monorepo `project` and a subset of its `paths` as
  `interface`. Interface patterns are either listed verbatim in `paths` or are
  literal paths matched by one of those patterns. When an interface pattern
  matches a changed path, the checklist
  requires a separate full clean CI item for every other affected project in the
  current required-project set when `require_ci` is false, plus one eligible
  non-author owner approval from each ownership rule linked to those affected
  projects. Operators may declare `ownership.dependents` as an explicit source
  project to dependent-project map; when a source is absent, every other
  affected project is treated as a dependent, while an empty set declares none.
  Every project link — including required evidence projects — must name a
  configured review-service project or startup fails.
  Changed paths come from the provider comparison, including renamed old paths;
  incomplete observations cannot silently satisfy ownership. Registration and
  upload automatically request eligible owners. An empty rules list disables
  this additional requirement, not the ordinary approval policy.
- `intake`: explicitly enable the default `refs/heads/review/` contribution
  namespace with an admitted `default_target_ref`. Optional exact `mappings`
  bind other source refs to a target and authenticated principal. A protected
  target can never also be an intake source. Set `null` to disable intake.
  Optional `intake.jujutsu` enables change-ID intake (see "Jujutsu change-ID
  intake" below): `workspace_path` and `executable` (absolute, both or
  neither), `change_id_header` (default `false`) and `max_stack` (1–32).
  Absent exports nothing, so existing profiles stay byte-identical.
- `delivery`: explicitly register HTTPS destinations and exact principal-to-
  recipient bindings, request/response bounds and retained-record/attempt limits.
  Set `null` for local inboxes only. See the delivery contract below.
- `queue`: `kiln.review-queue.v1` with `max_entries` (1–1000) and
  `max_candidate_builds` (1–128). Requires `require_ci=true` and both
  `build_integration` and `submit` capabilities. Set `null` to disable automation.
  Optional `mode` is `serial` (default when omitted) or `speculative`.
  `speculative` requires `window` `{floor, ceiling, increase, decrease}`. The
  bounds are `1 ≤ floor ≤ ceiling ≤ 16` and `ceiling ≤ max_entries`; `increase`
  and `decrease` are each between 1 and `ceiling`. It also requires the
  `speculative_integration` capability. `serial` rejects `window`. Omitted
  fields are not exported, so existing profiles stay byte-identical.
- `review.noise` (optional): display-only `{ id, pattern, reason }` path globs
  for generated, vendored and lock files, for example
  `{ id = "lockfiles", pattern = "**/Cargo.lock", reason = "Lockfile churn" }`.
  Patterns use the ownership glob grammar (`kiln_core::paths`: `*` and `?` within
  a segment, whole-segment `**`; no absolute, `.`/`..`, empty segments,
  backslashes or character classes). Limits: at most 64 rules, patterns of at
  most 256 bytes, unique ids of at most 128 bytes without whitespace, and
  non-blank reasons of at most 256 bytes. `kiln-web-service check` and the
  Nickel contract reject anything else. The rules' BLAKE3 display identity is
  published with the review summary, change detail and diff, so a rule change is
  visible and attributable; it deliberately does not change the review store
  binding or the effective policy identity. Absent exports nothing, so existing
  profiles stay byte-identical. See "Display-only noise files" below.

Export the reviewed input to a private JSON file and supply it explicitly:

```console
kiln-web-service check --profile "$SERVICE_PROFILE" --access "$ACCESS_PROFILE" \
  --code-review "$REVIEW_PROFILE"
kiln-web-service serve --profile "$SERVICE_PROFILE" --access "$ACCESS_PROFILE" \
  --code-review "$REVIEW_PROFILE"
```

Review-profile preflight does not initialize a repository, create the review
database or mutate Git refs. Without this profile the page reports unconfigured
review access rather than inventing a writable provider. Changing provider/project
binding cannot silently reinterpret an existing store. Keep backups private:
drafts, personal queries and notification state are durable service data.

#### Review and submission

Create a change from an exact locally available commit and admitted target.
Upload later exact commits on that same stable change. Revision and comparison
selectors preserve exact identities; file navigation, line permalinks and unified
or side-by-side diffs use provider observations. Binary/truncated content is
explicit and cannot pretend to supply reviewable line anchors.

Viewed markers are private hunk preferences, never approval evidence: they do
not count toward approvals or ownership, and votes stay bound to the exact
current revision. The service identifies each hunk of the revision's exact base
comparison by BLAKE3 over its path and removed and added lines (no line numbers
or context); a file without textual hunks is one whole-file hunk, and binary or
truncated files cannot be marked. Each hunk header has a "Viewed" toggle and each
file a "Viewed" mark-all toggle that writes one marker per hunk; files show
"N of M hunks viewed" and how many hunks are new since your last viewed
revision. Uploading a new revision carries a principal's markers forward only
for hunks whose digest is unchanged; edited, moved-with-edit and new hunks start
unviewed. File markers recorded before hunk markers keep their meaning on their
original revision only and replay unchanged.

"Changes since revision N" in the comparison selector is a rebase-aware
interdiff: the provider rebases revision N onto the selected revision's base in
memory (nothing is written to the repository) and diffs the result against the
selected revision, so target movement disappears. A conflicted rebase, a missing
commit or a provider without the capability reports the interdiff as
unavailable with its reason, and the exact commit comparison is shown instead
with a notice. The rebased side is not an exact revision coordinate, so only
revision-side lines can be commented on there. The view is URL-addressable as
`review_interdiff=<from revision id>..<to revision id>`, optionally narrowed with
`review_interdiff_change=<jj change id>` to the one commit carrying that `change-id`
header in each revision; a stack whose commits lack change IDs, or where the
change is missing or duplicated, reports unavailable. For a revision with several
commits, the commit-range selects compare any exact `review_commits=<from>..<to>`
inside the revision; a range ending before the revision head cannot be
commented on. Browser back and forward restore each view.

<!-- r[impl kiln.review.noise.authority] -->
**Display-only noise files.** A displayed file whose new or old path matches a
`review.noise` rule (first rule in profile order) is listed in one collapsed
group headed "N noise files hidden" with the matching rule ids; one click on the
group expands it, and the first non-noise file is selected by default. Noise
files are shown as viewed for every principal. That state is derived from the
diff's `noise` annotation at display time and is never stored as a viewed
marker. Selecting, expanding and commenting on a noise file work exactly as for
any other file. The boundary is strict: noise rules never satisfy, skip or
weaken ownership rules, required approvals, blocking threads, CI or queue
admission. A file matching both a noise rule and an ownership rule still needs
its owner's vote. Only declared globs are used; Kiln never infers "generated"
content, and binary and truncated handling is unchanged.

<!-- r[impl kiln.review.advisory.authority] -->
**Advisory approvals.** Any read principal other than the change owner and
revision authors may record one non-binding advisory approval of the exact
current revision (`{"type":"advisory_approve"}`) and withdraw it
(`{"type":"advisory_withdraw"}`). These two actions are the only commands a
read-only principal may send; every other command still fails with
`403 review_write_denied`. Owners and authors get `409 advisory_forbidden`, and
withdrawing a missing record gets `409 advisory_missing`. At most 128 advisory
approvals are kept per revision. They are stored apart from votes, so they never
satisfy required approvals, ownership, veto clearing, CI or queue admission, and
they never clear, create or transfer to a candidate. They add no attention, inbox
event, delivery or recipient, and grant no write capability. A new revision makes
them historical. The change detail lists them under "Advisory approvals
(non-binding)", separately from votes. The change list shows the current count,
and the search form's **Advisory approvals** filter (`advisory=with|without`) can
be saved in personal views. The separate **Agent submission** filter
(`agent=with|without`) matches whether the current revision carries attested
agent submission identity. **Agent harness** (`agent_harness=<declared harness>`)
matches that exact declared harness, and **Agent model**
(`agent_model=<declared model>`) matches that exact declared model. A revision
with no declared model never matches a model filter. All three are saved in
personal views. Older journals
and saved views replay unchanged.

Line selection seeds a private draft with the revision, comparison-base commit,
side, file and range. Save drafts before selecting them for atomic publication
with an optional current-revision vote. Historical drafts retain their original
anchors after upload; they are never silently retargeted. Replies, resolution,
reviewer/CC requests and attention create attributable local inbox events.
Published mentions and subscriptions additionally use explicitly configured
external delivery; private drafts never do. Search offers awaiting-review,
owned-needs-revision and unresolved views; saved queries and drafts are private
to the authenticated principal.

<!-- r[impl kiln.web.console_layout] -->
The page header holds **Create change…** and, when branches are admitted,
**Contribute branch…**; both open a form whose commit asks for a confirmation that
names the change or branch, for example `Create change "<title>"?`. Search, personal views and the change table share one
**Changes** card that states the principal's review access. A selected change
opens with an identity card and revision selector; on wide screens the side
column, led by the submit requirements, stays in view while the diff scrolls.
The identity card also shows the observed submit readiness, linked to the
requirements card. The access boundary, the service's non-claims and the target
write boundary sit in an **About this view** disclosure; each panel keeps its own
boundary text in a labelled "About …" disclosure.

The checklist exposes each unmet submit requirement with an icon and label:
**Met**, **Unmet** (outstanding review work such as approvals, threads, WIP,
dependencies, source/target or CI) or **Blocked** (policy, a non-open change,
a current veto or needs-revision vote, or an unreadable ownership observation);
the requirement name and service detail stay visible. Canonical exact-source CI
facts link to the run, preserving full/partial and clean/dirty distinctions.
Only the server's current exact winner evaluation can satisfy required CI;
browser assertions and retained Patchgate verdicts never grant merge authority.
Prepare a candidate, then confirm the exact principal, change, source and target
before submit. Ordered-stack submission evaluates and confirms each dependency
separately and rechecks each next target; it is not unchecked batch mutation.

Abandon/restore changes review state. Rebase registers a new revision; revert and
cherry-pick register separate WIP reviews. These operations do not bypass review
by directly advancing a target. Submission alone uses the provider's exact target
compare-and-swap after current policy, source and CI admission.

#### Contribution intake and review rendering

The branch picker observes exact local commits and binds its contribution to
the authenticated principal. The same branch updates one stable change; replay
does not append a duplicate patch set. Moved expected commits, reassigned
branches, protected targets and symbolic or non-commit refs fail closed. The
HTTPS developer CLI uses the same admission, without Git ref write authority:

```console
kiln-web-service ci --profile "$DEVELOPER_PROFILE" --identity "$DEVELOPER_IDENTITY" \
  review-submit --request-id "$REQUEST_ID" --source-ref refs/heads/review/my-change \
  --title 'Review this contribution' --description 'Exact committed branch'
```

Create, upload and branch-contribution commands accept an optional
self-declared `authorship`: `{"kind":"human"}`, `{"kind":"mixed"}` or
`{"kind":"agent","harness":"…","model":"…"}`, where `model` is optional.

- **Text rules:** harness and model must be trimmed, non-empty text of at most
  128 bytes with no control characters.
- **Rejections:** invalid text fails with `400 review_authorship_invalid`, and a
  malformed shape fails with `400 review_command_invalid`. Both happen before
  any provider effect.
- **Console:** the Create change, Contribute branch and Upload revision forms
  offer a labelled Authorship select that defaults to Undeclared.
- **Developer CLI:** `review-submit` accepts
  `--authorship undeclared|human|agent|mixed [--agent-harness TEXT [--agent-model TEXT]]`.
- **Undeclared:** omitting authorship records the patch set as undeclared, and
  older journals and requests replay unchanged as undeclared. Service-created
  patch sets are always undeclared: rebase, revert, cherry-pick and applied
  suggestions.
- **Change detail:** shows the change's class, folded from its declared patch
  sets, and each patch set's declaration.

Authorship is self-declared and unverified. Kiln never infers or checks it, and
it grants no review or submit authority.

**Optional session-trace pointers (off by default).** Set `session_traces = true`
in the review profile's Nickel input to capture admitted local notes on new
exact revisions. See the [enabled fixture](../../config/fixtures/web-code-review-session-traces-positive.ncl)
and [explicit opt-out](../../config/fixtures/web-code-review-session-traces-disabled-positive.ncl).
With capture disabled, no note is read and no field or trace panel is added to
new revisions. Existing pre-feature journals replay byte-identically. Changing
the flag never backfills or rewrites a retained revision; previously captured
observations remain historical even if capture is subsequently disabled.

The first supported source kind is `git_notes_traces`, using the documented
[`refs/notes/traces` pointer format](https://traces.com/docs/sharing/git-hooks):
one `traces:<externalId>` per line, optionally followed by an absolute HTTPS
shared URL without userinfo. IDs are ASCII letters/digits, `-`, `_` or `.`;
unsupported formats are explained as incompatible, not silently omitted.
Capture is bounded to 16 KiB of note content, eight distinct IDs and 256-byte
ID/host fields. Repeated IDs must agree on their retained host. Missing notes
are explicitly absent; malformed, oversized, too-many, unsupported-provider and
provider-unavailable observations are distinct bounded results. The Git adapter
checks the blob header before loading note bytes and reads only the exact
commit's admitted local note. It never fetches notes, invokes a hook or contacts
traces.com or another session host.

The selected revision's **Session trace pointers** card separates external IDs
and optional normalized URL hosts from Kiln's BLAKE3 pointer identity and raw
pointer-note digest. Each identity binds the exact commit, note digest, source
kind, ID and host; copied commit/digest/host bindings are rejected. These hashes
identify admitted pointers, not unseen session bodies. Only declared ID/host
metadata is retained; URL paths, queries, fragments and session bodies are
omitted, and credential-bearing URL authorities are rejected. IDs and hosts are
retained literally after validation: operators must not encode private data in
them. No prompts, tool results, session-body rendering, remote fetches or
provider links are supported.
Pointers are inert **untrusted historical context**, separate from self-declared
authorship and attested submission. They do not prove that a model ran, satisfy
CI, ownership evidence or intent approval, change approval counts/checklists, or
grant submit or merge authority.

Read principals can export the canonical review window with
`GET /api/code-review/review-window?start_ms=<ms>&end_ms=<ms>`. The range is
half-open, the span is at most 30 days, and no other query keys are allowed.
The document feeds `kiln-dashboard --review` and the HTTPS and iroh review
views. It is a read-only projection with no authority. Each change carries
`advisory_approvals` (advisory approvals of its latest admitted patch set, at
most 128, omitted when zero). The review-friction view reports
`advisory_approvals` and `advised_changes` per authorship class, rendered
identically by both shells and the dashboard as non-binding counts. Windows and
view identities without advisory approvals are byte-identical to earlier ones.
Each exact CI fact may also carry typed checks (`name`, `kind`, `outcome`,
optional artifact digest) derived from the run's nodes and retained reports.
Policies may require a known check kind or name. These requirements belong to
the review project and must pass on its ordinary policy-selected, successful
terminal CI run for the exact reviewed/integration commit and current scope.
Historical, dirty, partial, non-counting untrusted and other-project runs cannot
lend a pass. In monorepos, other affected projects retain their independent CI
requirements; a missing selected review-project run leaves typed checks unmet.
The checklist names the actual selected typed run or explains its absence.
Checks are evidence, not gate or release authority. Agent-opened threads are annotated `open`, `addressed`, `dismissed`,
or `unknown`; addressed requires a later same-base revision whose exact hunk
overlaps the anchor before resolution. A read-only window view is available at
`GET /api/code-review/agent-thread-outcomes?start_ms=<ms>&end_ms=<ms>`.

Markdown uses DOM text nodes, safe inline formatting and fenced code (shown
plain); supplied HTML does not execute. The diff is highlighted by the
service's pinned Tree-sitter grammars (`highlight=1` on the diff read); no
grammar runs in the browser and no CDN is used. Unsupported, oversized or
malformed highlighting renders plain with a note. The whitespace selector
changes the provider comparison while retaining exact original line
coordinates and URL-restored selection.

The "Comparison mode" select offers an opt-in structural (syntax-aware) view
(`review_mode=structural`, `mode=structural` on the diff read). The service parses
both blobs with the same pinned grammars, pairs unchanged and moved top-level
items and diffs the remaining tokens, so pure reformatting shows no change and a
moved, unchanged function is reported as moved. Rows keep exact blob line
numbers, so comments created there equal line-mode anchors; moved regions are
shown as a note without comment controls. Bounds are 256 KiB per side, 40,000
tokens per side, a deterministic work budget and a two-second deadline per
response. A missing grammar, binary or truncated content, a parse error, a hit
bound, an interdiff's rebased side or a whitespace filter falls back to the
line diff for that file with the named reason. The mode is display only.

A published comment may contain one complete `suggestion` fence, with optional
surrounding prose. Its exact inclusive revision-side path/range supplies the
replacement. Select suggestions and confirm their exact identities as the change
owner. The provider rechecks blob bytes, modes, ranges and non-overlap; a stale,
ambiguous, binary or unsafe-path replacement fails closed. Applying creates a
new immutable revision, leaves the target untouched and invalidates old approval
authority. Empty fences intentionally delete the anchored range.

##### Keyboard commands

<!-- r[impl kiln.review.keyboard] r[impl kiln.review.keyboard.navigation] r[impl kiln.review.keyboard.actions] -->
<!-- r[verify kiln.review.keyboard.navigation.scenario.targets] r[verify kiln.review.keyboard.actions.scenario.cancel] -->
The review page offers these shortcuts while it is open and no dialog is
visible. Press `?` in the page for the same list.

| Keys | Action |
|---|---|
| `j` / `↓`, `k` / `↑` | Next or previous changed file (the same route as its link) |
| `n`, `p` | Next or previous hunk of the displayed file |
| `t`, `T` | Next or previous unresolved thread (routes to its exact line) |
| `u`, `U` | Next or previous unviewed hunk, following the focused hunk and wrapping |
| `m` | Toggle unified and side-by-side layout |
| `v`, `V` | Toggle viewed for the focused hunk, or for every hunk of the displayed file |
| `d` | Open the anchored draft editor at the selected line and focus its comment |
| `P` | Open the publish confirmation |
| `s` | Jump to the submit requirements card |
| `?` | Show the shortcut help |

Shortcuts are inert while focus is in an input, textarea, select or editable
region, while a dialog is open, during IME composition, and with Control, Meta
or Alt held. They add no command or authority: each one invokes the same
control as the pointer, so a write keeps its usual confirmation, request
identity and disabled reason, and a cancelled confirmation sends nothing.

#### Jujutsu change-ID intake

<!-- r[impl kiln.review.jj_intake.surface] -->
With `intake.jujutsu` configured, a write principal can contribute by exact jj
change ID instead of a `review/` branch. One change ID maps to one Kiln change
whose identity is BLAKE3 over the repository identity and the change ID. Each
newly observed commit for that ID becomes the next revision. A commit that is
already a revision appends nothing, so duplicate and replayed submissions are
inert.

```console
# One change.
kiln-web-service ci --profile "$DEVELOPER_PROFILE" --identity "$DEVELOPER_IDENTITY" \
  review-submit --request-id "$REQUEST_ID" --jj-change "$(jj log --no-graph -r @- -T change_id)"
# An ordered stack, bottom to top.
kiln-web-service ci … review-submit --request-id "$REQUEST_ID" --stack \
  --jj-change "$BOTTOM" --jj-change "$MIDDLE" --jj-change "$TOP"
# A pushed branch whose commits carry jj's git `change-id` header (no jj needed).
kiln-web-service ci … review-submit --request-id "$REQUEST_ID" --stack \
  --source-ref refs/heads/review/my-stack --change-id-header
```

The console offers the same as **Contribute jj change…**. The command is
`{"type":"jj_contribute","change_ids":[…],"stack":bool,"wip":bool}` or
`{"type":"jj_contribute","source_ref":"refs/heads/…","stack":bool}`, with the
optional `authorship` declaration. Titles and descriptions come from the
commit message. A commit without a summary line is refused with
`review_jj_title_missing`.

**Boundary.** Kiln stays read-only toward jj and is not a jj forge:

- It reads the single operation head from `.jj/repo/op_heads` on disk.
- It runs only the configured jj executable, with `--ignore-working-copy`,
  `--at-op <that head>`, a cleared environment (`JJ_CONFIG=/dev/null`),
  fixed JSON templates, and a 15 s timeout and 1 MiB output bound.
- It never snapshots, rebases, squashes, describes, pushes, merges operations
  or moves bookmarks. jj may still populate its derived index cache.
- Only full 32-character change IDs are keys. Prefixes, revsets and the root
  change are refused.
- The jj Git store must be the review repository. Every resolved commit must
  exist there; when it carries a `change-id` header, the header must match.
- A change ID grants no write authority. Target protection, compare-and-swap
  submission, CI and approval rules are unchanged. Stack intake grants no
  batch-merge authority: every change keeps its own approvals, CI and submit.

**Fail-closed admission.** Every requested change goes through
`JujutsuFacts` admission before any provider effect, reservation or state
change. One refused member refuses the whole request. Stable codes:

| Code | Meaning |
| --- | --- |
| `review_jj_operation_missing` | No operation head was found. |
| `review_jj_operation_divergent` | More than one operation head exists. |
| `review_jj_divergent_change` | The change ID resolves to more than one visible commit, or appears twice on a header branch. |
| `review_jj_conflicted_bookmark` | A bookmark on the resolved commit is conflicted. |
| `review_jj_conflicted_commit` | The commit has jj conflicts (jj `conflict`, or a `jj:trees` header). |
| `review_jj_change_missing` | The change ID is not visible at the observed operation. |
| `review_jj_commit_unreachable` | The resolved commit is absent from the review repository. |
| `review_jj_header_mismatch` | The commit's `change-id` header disagrees with jj. |
| `review_jj_store_mismatch` | The jj Git store is not the review repository. |
| `review_jj_unsupported_backend` | The jj backend is not Git. |
| `review_jj_stack_order` | Stack members are not listed bottom to top in ancestor order. |
| `review_jj_header_missing`, `review_jj_header_merge` | A header branch has a commit without a valid header, or a merge commit. |
| `review_jj_change_id_invalid`, `review_jj_change_duplicate`, `review_jj_stack_required`, `review_jj_stack_bound` (400) | Malformed request. |
| `review_jj_execution_failed` (503) | jj could not be read within its bounds. |

**Stacks.** `stack: true` creates or updates one change per change ID. Stack
intake owns the dependency edges of its members: the bottom member has none,
and each other member depends on the member below at that member's current
revision. Resubmission maps by change ID:

- A revision is appended only where the exact commit changed. jj rebases the
  descendants of an edited change, and a rebased descendant has a new commit,
  so it also gains a revision; revisions stay bound to exact commits for
  compare-and-swap submission.
- Unchanged members and already submitted members are left untouched.
- Edges are rebound when the order or a bound revision changes.
- An abandoned member is refused (`review_jj_change_abandoned`).
- A change ID that disappears is listed in the response's `removed` and shown
  as removed on its change. It is never abandoned; abandon it explicitly.

Single-change requests never touch stack membership or dependencies.

**Provider observations.** Each admitted revision records:

- the source (`jj` or `git_change_id_header`);
- the change ID and commit;
- the observed jj operation, and the operation that created the commit;
- the jj predecessor commits (from `evolog`);
- the commit author.

These are provider facts, never Kiln identities, and grant nothing. Change
detail renders them as an **Evolog**: every revision with its predecessor,
operation, author, declared authorship and a link to the interdiff from the
previous revision (`review_interdiff=<from>..<to>`). It also shows stack
position, or the removed flag. Header intake records no operation or
predecessors. Branch intake is unchanged. Without jj intake, state and
journals serialize exactly as before.

#### Published attention and external delivery

`@principal` and `@{principal}` in newly published comments route attention only
to admitted identities. Subscribe/unsubscribe and read state are principal-owned;
private drafts are neither external payloads nor another principal's inbox.

Delivery reuses the bounded HTTPS adapter: exact destination authority, resolved
IP restrictions, TLS/CA policy, credential-file checks, timeouts and byte bounds.
`webhook` sends `kiln.review-notification.v1`; `email_adapter` sends the same
registered envelope to an operator-provided email gateway, not directly to SMTP.
Recipients never come from comment text or browser-entered URLs. Payloads carry
notification metadata, not comment or draft bodies. Durable reservation precedes
transport; success, rejection, safe retry and uncertain outcomes remain distinct.
A reserved delivery interrupted by restart is uncertain and is not blindly
resent. Delivery identity is not an exactly-once recipient-effect guarantee.

#### Required-CI merge automation and target protection

Authorize the exact source revision, target ref and automatic recomputation scope
in the merge-queue confirmation. Per-target FIFO automation constructs an exact
merge commit, runs the ordinary canonical workflow on that candidate, then
rechecks current source approval, path owners, policy, dependencies, full/clean
CI and target before exact-target CAS. It does not transfer source approvals to
the candidate or manufacture a browser CI verdict. Dependencies must land first.
Target movement clears candidate and run identities and requires a new candidate
and fresh CI; revision or policy changes revoke the queued authorization.

The queue and provider operation journals persist through service restart.
An ambiguous provider operation is reconciled, not dispatched again. An owner
or the authorizing submitter can cancel pending automation, not undo a completed
target update. Active worker HTTPS ingress can fail when the service is killed;
such CI failure blocks submission and is not silently retried or relabeled success.

With `mode: speculative`, each target runs a Zuul-style window
(`kiln_core::gating`):

- **Building:** for the admitted positions (`admitted_candidate_indices`),
  the queue builds candidate k as the exact merge of source k onto candidate
  k-1, through the `BuildSpeculativeIntegration` provider operation. That
  operation compares the exact target before and after construction. The
  head's candidate is an ordinary integration onto the target.
- **CI:** canonical CI is admitted for every exact candidate commit without
  waiting for submissions. Engine dispatch remains bounded by the existing
  per-project execution capacity.
- **Submitting:** candidates are submitted strictly in queue order. Each one
  passes the same fresh approval, owner, policy, dependency and full/clean-CI
  admission and exact-target CAS as in serial mode. Source approvals never
  transfer, and advisory approvals never count.
- **Failures:** a failure is conclusive only at the head. The failing entry
  alone is ejected (`blocked`), and the window shrinks through
  `update_window`. Each later entry whose chain included it is superseded:
  undispatched candidate runs are cancelled, results of dispatched runs are
  ignored, and the entry is rebuilt with fresh CI.
- **Window size:** a landed candidate grows the window up to `ceiling`. A
  target without a recorded window starts at `ceiling`.
- **Predecessor chain:** each entry durably records the root target, each
  predecessor's queue entry, generation and candidate, and a BLAKE3 chain
  identity (`chain`). It keeps up to 32 retired identities in
  `superseded_chains`. Builds or CI results for a retired chain never become
  the entry's candidate. A target moved by another writer supersedes every
  chain on it.
- **Restart:** the stored chains, candidates and runs are reused. CI
  admission identities are derived from the entry, generation, chain and
  project, so admission is replayed, not dispatched again.

All Kiln target mutations pass current policy and exact CAS. This is **not OS or
forge branch protection**: unmanaged repository writers can bypass the service.
Deployments must restrict write access to protected refs externally. The browser
cannot install that access control, and local qualification does not prove it.


SQLite reserves requests durably before provider effects. Request replay binds
principal, payload and expected version. Restart reconciles exact observations
without redispatching effects; pending/uncertain operations remain visible and
reserve their affected changes while unrelated history stays usable. After a
timeout or ambiguous result, refresh and inspect the durable operation; do not
blindly repeat the action under another request identity.

Admission is bounded: 1000 changes, 128 revisions per change, 128 private drafts,
64 personal views, 4000 provider operations, 20000 request-journal entries,
16 MiB state and 4 MiB responses. Lists page changes by 25, notifications by 200
and outstanding operations by 100. Bounds reject excess admission; no automatic
pruning or unlimited history is claimed.

Kiln maintainers own the reusable core, provider conformance and service/browser
integration. Repeat the isolated behavioral rail with `cargo test --locked -p
kiln-core -p kiln-adapter-git -p kiln-web-service` from the development shell.
Repeat real required-CI qualification with `scripts/review-workflow-smoke.py`
using explicit service, Lattice binary/source, private scratch and evidence paths.
It drives branch intake, owner requests, private publication, external HTTPS
delivery, suggestions, reapproval, candidate construction, restart and canonical
CI through independent disposable target readback, plus genuine CI failure.
`--browser-hold` keeps that same service available for desktop/mobile interaction.
Evidence under `complete-review-workflow/evidence/` distinguishes native checks,
browser observations and unsuccessful crash experiments; none grants deployment,
publication, lifecycle acceptance or release authority.


