# Bind the Aspen Extension to the Runtime Profile

## Problem

The hosted extension currently admits a compiled `profile_fixture()`. The host admits the deployed runtime profile from an explicit file. A valid deployment can change a bound, such as the callback timeout, and therefore change the exact profile identity. Lifecycle callbacks can still pass, but the first hosted trigger then fails profile admission after the host durably reserves it.

## Proposed Change

Require the extension process to load the same explicit runtime profile used by the host. Bind both the profile path and expected BLAKE3 profile identity into the bounded execution request. The extension must reject missing, malformed, changed, or identity-mismatched profile input before it handles a callback.

## Success Criteria

- The host binds one explicit runtime profile path and exact admitted identity into every extension execution.
- The extension accepts a valid non-default profile whose identity matches the host.
- Missing, duplicate, malformed, changed, or identity-mismatched profile arguments fail closed.
- Any failure after durable reservation remains `Unknown`; recovery does not redispatch it.
- Existing callback, durable recovery, and pacing behavior remains unchanged.
- Positive and negative tests, strict Clippy, Octet, Cairn, and Nix checks pass.
