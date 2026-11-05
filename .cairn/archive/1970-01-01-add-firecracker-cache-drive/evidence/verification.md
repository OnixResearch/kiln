# Verification evidence

## Result

The real Firecracker KVM journey restores through a read-only second drive,
returns declared guest outputs, and publishes only after successful execution
and confirmed cleanup. The first run published 7 bytes; the second restored
those 7 bytes as a hit and retained the existing entry. Both runs confirmed
cleanup. Kiln maintainers own the reusable archive/transfer path and the
`firecracker_cache_restores_after_publication` journey, now enabled without an
ignore attribute.

## Evidence

- Guest build: `nix build --no-link --print-out-paths path:.#microvm-guest --builders '' --option build-hook ''` produced `/nix/store/9ylnhv606d4q5kd5zl8qz5r0l33a8qss-kiln-microvm-guest`. Local building bypassed an external build-hook filename-too-long failure.
- `cargo test --locked --offline -p kiln-microvm --lib`: 40 passed.
- `cargo test --locked --offline -p kiln-hosted --lib --test journey`: 74 library tests and 9 journey tests passed.
- With that guest image and `KILN_MICROVM_REQUIRED=1`, `cargo test --locked --offline -p kiln-hosted --test microvm firecracker_cache_restores_after_publication -- --nocapture`: 1 passed; no skip.
- Earlier unchanged web-service rail: 168 passed, 1 ignored.
- `cairn validate --root .`: zero issues.

The live run exposed two bugs not covered by the original synthetic return:
ustar overhead exceeded the content-only transfer limit, and BusyBox directory
headers ended with `/`. The transfer limit now includes headers, padding and
trailer; extraction accepts one directory suffix while rejecting traversal,
absolute paths and doubled separators. The host preserves guest rejection
reasons. Regression coverage includes source-key binding, changed-source
rejection, archive extraction and a transfer bound that cannot fit the frame.

## Non-claims

No object-storage cache tier, performance qualification, hostile-isolation
qualification, or lifecycle archive approval is established. These observations
cover one local KVM cohort and its exact guest image, not arbitrary guests.
