# Verification

Issue [#42](https://github.com/OnixResearch/kiln/issues/42).

`rebuild_check` is an off-by-default reviewed profile field. When enabled, the
adapter runs `nix-store --realise --check -K DERIVATION` once after the exact
realised derivation is rooted, reusing the build's timeout, stdin, stdout,
stderr, poll-interval and teardown limits.

Each expected output receives a typed observation: `matched`, `differed` with
the observed `<path>.check` retained output, or `not_checkable` when the check
was refused, timed out, was cancelled, exceeded its output bound, was ambiguous,
or exited nonzero without a retained output. A check observation cannot change
the build outcome, products, GC roots or receipt identity.

Focused adapter tests cover the bounded argv, matched output, retained
difference, not-checkable refusal, disabled default and admission refusal for
non-success or misnamed observations. `cargo test -p kiln-adapter-nix` passed 20
tests and `cargo test -p kiln-core` passed 194.

Wording stays "rebuilt once on this host, matched or differed". The
reproducibility non-claim remains; diffoscope retention is not part of this
option.
