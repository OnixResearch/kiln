# Design: Exact Extension Runtime Profile Binding

## Ownership

Kiln owns the extension protocol and exact profile admission. Aspen owns generic bounded process execution. The host composition shell owns profile file I/O. The callback core continues to receive an already admitted profile and performs no file or environment access.

## Interface

The `kiln-aspen-host` composition root already requires `--aspen-profile <path>`. It will canonicalize that path, admit the document once, and bind these explicit extension arguments into every native execution request:

```text
--runtime-profile <canonical-path>
--runtime-profile-identity <admitted-blake3-identity>
```

The `kiln-aspen-extension` binary requires exactly those named arguments. It reads the profile under the existing bounded profile-file limit, admits the document, and compares the resulting identity with the supplied identity before reading or handling the callback envelope.

## Exactness and Failure

The expected identity prevents a changed path target or changed profile document from silently altering callback meaning. The production path is an immutable Nix store object, but the extension enforces identity equality for every supported deployment.

A missing file, unsafe or incomplete argument set, oversized or malformed document, failed profile admission, or identity mismatch returns a nonzero extension exit. If the host already reserved the operation, every later construction, callback, decoding, or acknowledgement failure marks it `Unknown`. Reconciliation does not redispatch it. The host retains the bounded process observation and does not select another profile or runtime.

## Functional Core and Shell

Profile loading, argument parsing, and process exit remain in the extension shell. Existing `KilnAspenCallback` logic remains deterministic and takes `AdmittedAspenRuntimeProfile` as an explicit value. The host template is the visible composition root for the concrete profile path and identity.

## Recovery

Durable operation and callback identities already include the admitted profile identity. Normal accepted uncertainty uses the same stored cohort and immutable execution arguments. This change does not permit redispatch of a reserved or uncertain operation.

A denied extension process moves the native Aspen lifecycle to `Failed`. Restart then fails closed instead of reusing that state. The operator must retain it under quarantine and start a fresh host root. It cannot become provider or completion evidence.

## Tests

Positive tests cover a non-default timeout profile that matches its expected identity. Negative tests cover missing, duplicate, malformed, unreadable, oversized, invalid, and identity-mismatched profile inputs. Host template tests prove both arguments are bound. Existing hosted, recovery, and pacing tests remain required.
