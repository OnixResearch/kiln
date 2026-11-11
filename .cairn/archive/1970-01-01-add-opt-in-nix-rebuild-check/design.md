## Decisions

The check is a separate reviewed option because it doubles build work and can
fail for reasons that are not build failures. It runs only after a successful
realisation and after products are rooted, so a failed or unknown build never
gets a check observation.

`-K` is always passed. When Nix keeps a differing output, the adapter records
the exact `<path>.check` path only after observing that path entry. A nonzero
check without a retained output is `not_checkable`: Kiln cannot distinguish a
substituted or fixed-output refusal from another refusal, and must not call it a
difference.

The check reuses the build's `ExecutionLimits`. Its bounded log is hashed into
the observation; the log bytes are not retained as a report. No GC root, output,
receipt or product identity changes because of the check.

## Non-claims

The observation proves only that this host rebuilt the exact output once and
compared it. It does not prove reproducibility across builders, hosts or time,
does not gate CI, and does not establish release readiness. A matched output is
not a product-correctness claim; a differed output is not a build failure.
