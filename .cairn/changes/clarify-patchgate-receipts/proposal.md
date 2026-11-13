## Why

The operator asked for the Patchgate receipts page to be redone so that it is usable and clear. Today it is neither:

- The page never says what Patchgate is, what a receipt holds or what to do with one.
- Each receipt in the list is named by its file name, its byte count and its provider profile. The list is sorted by file name, so the newest review is not first, and there is no search or filter.
- Five of the nine decisions that Kiln admits (`waiting`, `pending`, `removed`, `reverted`, `unknown`) read as "Unrecognized decision" on the page, and in the run's Reviews tab they take the unknown tone. A reader cannot tell a change that waits for a prerequisite from a change that Patchgate does not understand.
- No decision says what it means or what happens next.
- A selected receipt leads with BLAKE3 and content identities. The facts a reviewer needs (each change's message, paths, who decided, why and when) are only inside the raw native report.
- The page and the run's Reviews tab draw the same receipt differently, from two separate renderers.

## What Changes

- A page module, `crates/kiln-web-service/src/receipts.js` with `receipts.css`, renders the Patchgate receipts page and the run's Reviews tab from one receipt presentation. The receipt code in `web.js` is removed.
- One decision vocabulary covers all nine normalized decisions. Each has a label, a tone, a meaning and Patchgate's own next step, taken from Patchgate's state model and `explain` guidance. An unrecognized value and a missing one each read explicitly, and neither reads as accepted.
- The list names each receipt by its first change's message as Patchgate recorded it, shows a tally of decisions, the number of changes, the latest decision time and any anomaly (a producer claim of acceptance authority, or isolation not recorded). It lists the newest receipt first. A text filter (file name, message, change hash, candidate, effect or report identity) and a decision filter, including "Needs attention" for Patchgate's stuck states, narrow it at once and are restored from the URL. The arrow keys move between receipts.
- With no receipt selected, the detail panel explains how to read a receipt and lists every decision with its meaning and next step.
- A selected receipt reads from the human facts to the exact ones. First comes a headline with the first change's message, the file name and the latest decision time, then a tally of decisions and a plain statement that a receipt is evidence only. Each change follows with its decision, the decision's meaning, its message, paths, decider, decision time, reason and prerequisites, its full hash with a copy control, and any model, context-coverage or Nix-check evidence. Then come the review context (channels and states, trusted configuration, policy, protected and documentation paths, model requests and review time, verifier versions), the exact identities, the verbatim Patchgate report and the complete receipt. Every fact read from the native report is labeled as recorded by Patchgate. A normalized decision always comes from Kiln's admitted observation. A native report that cannot be read leaves the decisions in place and marks the recorded context unavailable.
- The run's Reviews tab shows the same receipt with its run binding (recording time, node, source revision, report record) and a download of the exact retained bytes. A review report that the run declared but that is unavailable is shown as unavailable, never as a clean review.
- `/api/reviews` adds two bounded facts per receipt, both read from the native report: `first_change_message` (the first line of the first normalized change's recorded message, at most 160 characters) and `latest_decision_at_ms` (the latest RFC 3339 decision time among the native records). Both are null when the native report cannot be read.
- The console's copy-to-clipboard helper moves from `source.js` into `ui.js`, so the Source page and the receipts page share it.
- The destination register's purpose for 3.2 and the Guide's vocabulary describe a receipt in plain words.
- Modify the accepted requirement `kiln.dashboard.reviews.view` and add three scenarios: decisions, recorded context and finding a receipt.

## Impact

- **Current consumer:** the operator and reviewers who read Patchgate receipts on the live console, both from the receipt directory and in a run's Reviews tab.
- **Immediate outcome:** a reader can find a receipt by what it reviewed and when, and can see what Patchgate decided for each change, why and what happens next, without opening raw JSON.
- **Durable capability:** one receipt presentation shared by the page and the run tab, and one tested decision vocabulary that names all nine admitted states.
- **Owner:** Kiln web-service and dashboard maintainers.
- **Files:** `crates/kiln-web-service/src/{receipts.js,receipts.css,web.html,web.js,ui.js,source.js,server.rs,reviews.rs}`, `crates/kiln-web-service/tests/reviews.rs`, `tests/live-console-ui.test.mjs`, `tests/kiln-identity.test.mjs` (surface list), `cairn-policy/development-coverage.ncl` (this change's delta root while it is in flight), `docs/operator.md`, `CHANGELOG.md` and this package.
- **Testing:** node tests for the decision vocabulary, the tally, the native-context reader over well-formed, partial, non-JSON and hostile reports, the list ordering and filtering, and the copy helper's use. Rust tests for the inventory's new facts, including unreadable reports, a message for another change, invalid times and bounds. `cargo test` for `kiln-web-service`. A browser review of the real assets over the real Rust reviews API, with realistic receipts covering every decision, light and dark, desktop and phone widths.
- **Authority:** presentation only. Lifecycle acceptance, synchronisation, deployment and publication stay with the operator.

## Out of Scope

- No change to receipt production, the `kiln.patchgate.review.v1` schema, Patchgate admission or the run report API. The page stays read-only and never dispatches Patchgate.
- No new icon, font or remote asset.
- Receipts are not linked to runs on the page. The directory carries no run binding.
- No deployment, publication, spec synchronisation or archive.
