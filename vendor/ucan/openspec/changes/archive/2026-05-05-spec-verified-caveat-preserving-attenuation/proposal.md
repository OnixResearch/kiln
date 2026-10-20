## Why

Capability resource/ability containment now routes through verified capability semantics, but the shell-local check that child capabilities preserve parent opaque caveats remains ordinary Rust. The audit found no exact vendored helper that pairs the same containing parent capability with opaque caveat preservation, so this needs its own OpenSpec before any implementation attempt.

## What Changes

- Specify the exact attenuation invariant: a child capability can rely on a parent capability only when resource/ability containment and parent caveat preservation both hold for the same parent.
- Define acceptable future implementations: either route through a new exact verified primitive or keep shell-local logic with focused negative coverage and guard documentation.
- Preserve opaque caveat payload semantics and caller-supplied policy evaluation.

## Capabilities

### New Capabilities
- `library.verified-caveat-preserving-attenuation`: Future work has a precise invariant for pairing verified containment with opaque caveat preservation.

## Impact

- **Files**: OpenSpec library spec now; future work may touch `src/token.rs`, `src/verified.rs`, tests, docs, and bridge guards.
- **APIs**: No immediate public API change required unless a data-only helper is exposed.
- **Dependencies**: No new runtime dependencies.
- **Testing**: Focused regressions must cover wrong-parent caveat preservation, missing caveats, extra child caveats, and successful attenuation.
