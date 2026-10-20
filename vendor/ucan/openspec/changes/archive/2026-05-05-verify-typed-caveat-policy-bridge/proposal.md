# Change: Verify typed caveat policy bridge

## Why
UCAN currently exposes caller-supplied caveat policy hooks, while the vendored verified-logic crate already proves a small typed caveat subset (time bound and rate limit) as deterministic conjunctions over caller-supplied runtime facts. A shell policy adapter can route that exact subset through the verified primitive without changing opaque caveat semantics or adding ambient runtime behavior.

## What Changes
- Add a first-party typed caveat policy set for the exact finite caveat subset represented by verified-logic: time bound and rate limit.
- Decode only explicit, documented payload formats for that subset and fail closed on malformed/unknown payloads.
- Evaluate the typed subset through existing verified caveat bridge helpers.
- Preserve caller-supplied caveat policy boundaries for all other caveats.

## Non-Goals
- No arbitrary caveat language, JSON policy VM, network lookup, persistence, replay store, quota accounting backend, ambient clock, or hidden policy defaults.
- No claim that caller-supplied request counts, windows, or evaluation times are fresh or correct.
- No change to unknown-caveat deny-by-default behavior.
