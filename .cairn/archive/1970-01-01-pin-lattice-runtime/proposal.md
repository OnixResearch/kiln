## Why

Pin the CI handler independently of the MIT contract import. GitHub issue #37 records the current mismatch.

## What Changes

Operator and Radicle workflow documentation MUST identify Lattice runtime 85a38b6a4bceeae4bb2057cf157e21752100f1b5 separately from the MIT contract import 70496e67c7fd4a8b05914161a8e09de2759bebc8. Historical Aspen observations MUST NOT qualify the newer handler.

## Impact

Kiln maintainers own the adapter/configuration and operator documentation. Current consumers are GitHub, Gitea, Radicle and the web service. No remote authority or license import changes.
