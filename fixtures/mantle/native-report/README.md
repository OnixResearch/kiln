# Captured Mantle native report

`failure-report-v1.json` is a `crunch-build-report-v1` document captured from a
real local Mantle invocation:

```console
mantle --store <temporary-store> --state-dir <temporary-state> \
  --json build examples/hello.ncl --no-substitute
```

The local producer reached its sandbox failure path because `bwrap` was absent.
Store, state, plan and log paths were replaced with the Kiln test profile paths;
the serialized native shape, collections and failure envelope are otherwise
unchanged.

The binary was a local release-cohort build, not the pinned cohort executable.
This fixture is report-shape compatibility evidence only. It is not an executable
identity, cohort refresh, producer authority or release claim.
