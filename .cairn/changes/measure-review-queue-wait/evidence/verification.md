# Review-queue wait measurement verification

Issue: <https://github.com/OnixResearch/kiln/issues/61>, measurement slice from
its 17 November comment.

## Delivery and ownership

The immediate result is an off-by-default synthetic fixture. It runs a frozen,
typed load through the real review queue in serial and speculative modes and
retains a `kiln.review-queue-wait.receipt.v1` receipt. The durable capability is
a repeatable baseline that a later admission, budget or backpressure design for
#61 must cite. It has three parts:

- the `Load` contract and its checked export;
- the pure classifier, summary and receipt validation in
  `code_review::queue_wait`;
- the shared harness used by the `review-queue-wait` example and the
  `review_queue_wait` test.

Kiln review-queue and admission maintainers own it. No service, default policy
or stored state changed.

## Exercised checks

Commands ran from this checkout through `nix develop path:$PWD -c`.

| Check | Observed result |
| --- | --- |
| `cairn gate proposal|design|tasks measure-review-queue-wait ...` | PASS before implementation; task gate PASS again after the tasks were ticked |
| `bash scripts/check-nickel-fixtures.sh` | PASS (92 positive, 617 negative), including the new positive fixture, the refused negative fixture and the export freshness check |
| `cargo test --locked -p kiln-web-service --lib queue_wait` | 5 passed |
| `cargo test --locked -p kiln-web-service --test review_queue_wait` | 1 passed in about 1 s for both modes |
| `cargo clippy --locked -p kiln-web-service --all-targets -- -D warnings` | PASS once a lifetime-bounded type alias replaced the complex edit-list type |
| Scoped `rustfmt --check --edition 2024 --config skip_children=true` | PASS for the 6 touched Rust files |
| `cargo run --locked -p kiln-web-service --example review-queue-wait -- --load fixtures/review-queue-wait/load.json --out .../evidence/receipt.json` | Wrote [`receipt.json`](receipt.json); load identity `65985688ab2eb5c9ca653d01acb2a1535d3d8496002127f5e05a89c1addd1467` |

The unit tests pair positive and negative cases:

- **Load:** the frozen load is admitted, and its identity changes only with the
  typed content. 23 malformed or unbounded loads are refused: schema, clock and
  queue bounds, candidate count, duplicate or invalid ids, late arrivals, CI and
  deadline bounds, unknown CI results, modes, windows and unknown fields.
- **Classifier:** every branch.
- **Summaries:** nearest-rank, with explicit zero counts and no invented waits.
- **Read-back:** 19 altered receipts are refused:
  - relabelled or swapped series, and numbers placed in the merge-queue series;
  - an empty reason;
  - an edited count or wait;
  - a relabelled outcome or code;
  - unordered, out-of-horizon, request-less or out-of-order dispatch times;
  - an edited load;
  - edited non-claims;
  - an unknown field;
  - a wrong schema;
  - reordered candidates;
  - an oversized receipt.

## Frozen load results

The load has eight candidates over 40 one-second ticks, a queue bound of seven
and one CI slot. In both modes, three candidates were submitted, and one each
failed, expired (withdrawn at its deadline), stayed unknown (CI outcome
unknown), kept waiting and was rejected (`review_queue_bound`).

| Wait (enqueue to ...) | Serial min / p50 / max (n) | Speculative min / p50 / max (n) |
| --- | --- | --- |
| Start | 0 / 12 / 21 s (5) | 0 / 3 / 16 s (7) |
| First CI dispatch | 1 / 13 / 22 s (5) | 0 / 6 / 15 s (6) |
| Submit | 4 / 10 / 20 s (3) | 3 / 6 / 16 s (3) |

Observed queue mechanics, recorded without acting on them:

- **Serial target reset:** after every landing, the next serial head first
  re-observes the moved target and resets in one tick, then builds in the next.
- **Speculative rebuilds:** charlie's CI failure at 8 s shrank the window and
  made delta and echo rebuild. Echo built three candidates; only the last one's
  CI was dispatched.
- **Wasted dispatch:** delta was withdrawn at 9 s. Its rebuilt candidate's CI
  run, still queued in the engine, was dispatched at 11 s and held the only CI
  slot until 14 s, which delayed echo's dispatch. `dequeue` does not cancel
  queued engine runs.
- **Unknown CI blocks progress:** foxtrot's unknown CI blocks the serial head,
  and in speculative mode it holds the CI slot, so golf's CI was requested but
  never dispatched.

These describe this declared load: scripted CI durations, one CI slot and tick
resolution. They are not production CI, reviewer or merge latency, and they
change no policy. A backpressure or withdrawal change would need its own design
that cites this receipt.

## Boundaries

- No external merge or landing queue, implementation work admission or inference
  request is in the path. Those three series are `not_measured` with their
  reasons.
- The fixture's scripted workflow provider is not Lattice, and the clock is
  synthetic.
- Chen and Stratton's 56.1 percent finding and the X posts appear only as
  context in `docs/fair-admission.md`. They are not loads or thresholds.
- The full workspace tests, Octet, release-readiness and flake checks were not
  run.

## Lifecycle

Native `cairn sync measure-review-queue-wait ... --execute` added
`kiln.admission.queue_wait` and its three scenarios to
`.cairn/specs/fair-admission/spec.md`; nothing was archived. `cairn validate`
passed. Traceability coverage counts 1,887 requirements, 1,885 of them
referenced. It still exits 1 with the same two missing GitHub anchors and 71
unrelated dangling IDs. Every `kiln.admission.queue_wait` ID is referenced.
