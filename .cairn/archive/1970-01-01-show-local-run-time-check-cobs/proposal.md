## Why

The operator requested browser-local last-run times alongside existing UTC evidence and a check that the live Onix COB browser works. Current timestamps are UTC only; the installed service omits its optional COB policy.

## What Changes

- Render recorded dashboard timestamps in browser-local time with a timezone label while retaining the exact UTC instant; expose last-run completion time in the overview.
- Exercise native COB reads, authenticate live inventory/detail requests and connect the existing public Kiln Git store through the separate read-only runtime policy. Populate that store with the workspace's active Cairn changes under the operator's explicit authorization, signed with the operator's existing Radicle identity, without publishing or synchronizing them.

## Impact

- **Files**: live web JavaScript, operator procedure, local typed COB policy and service invocation, bounded verification evidence.
- **Testing**: actual browser timestamps across timezones, unknown-time rejection, native six-type COB conformance, live empty/missing-object/filter reads, packaged service checks and required repository gates.
- **Owner / consumer**: Kiln dashboard maintainers own rendering and read conformance; the existing local dashboard operator owns explicit public-store selection and activation. No signature, synchronization, publication, lifecycle acceptance or release claim.
