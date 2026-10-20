## Context

UCAN exposes a `VerifiedTypedCaveatPolicySet` for a finite typed caveat vocabulary. Satisfaction already routes through vendored `verified_logic::ucan_caveat`, but payload parsing still performs shell-local ASCII/single-delimiter admission before converting bytes into `u64` values.

## Goals / Non-Goals

**Goals:** verify deterministic payload-shape admission for the supported typed caveat formats and route the UCAN adapter through it.

**Non-Goals:** prove UTF-8 parser correctness, decimal-to-`u64` parser correctness, clock freshness, rate-limit counter truth, arbitrary caveat semantics, caller policy correctness, or token provenance.

## Decisions

### 1. Byte-shape helpers in `ucan_caveat`

**Choice:** add helpers over `Vec<u8>`/bytes for ASCII decimal payload and rate-limit `max_count:window_seconds` shape.

**Rationale:** this is the exact deterministic fact currently supplied by the UCAN shell. ASCII digits and colon bytes are finite byte predicates and do not require trusting parser or policy semantics.

**Alternative:** verify full numeric parsing. Rejected for this slice because Rust's `str::parse::<u64>` remains a runtime parser boundary and overflow/error mapping should stay explicit in UCAN.

## Risks / Trade-offs

**Overclaiming parser semantics** → Docs and specs explicitly state the bridge covers only byte shape, not UTF-8/u64 parser correctness or numeric provenance.
