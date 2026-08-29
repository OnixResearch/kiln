# Verification evidence

Date: 2026-08-28

## Live defect

The paced production closure admitted runtime profile identity `da2996d3637e4ba693a234e713d464bfa06daeec5f32b2aa40421bff50e5f725`, whose callback timeout is `7,290,000` milliseconds. The deployed extension still admitted its compiled fixture, whose timeout is `30,000` milliseconds. The prior live profile identity `2008b2d661855cf7a384706b6641d8baf697bb66dc78a1a3fa97322a9e6be723` had matched that fixture.

The fresh status-disabled shadow reserved operation `44548a9302505778d3e43384a67be779ffa201e30ba0fea8d23ada664d4b7bf2`. Its durable record remained `reserved`, with no callback receipt, effect intent, provider observation, acknowledgement, Lattice operation, or report. The adapter received `aspen_ingress_malformed`. This is pre-provider defect evidence, not provider-backed canary evidence.

The exact trigger used actor `did:key:z6MkmkA6sEzzMffaWqKEKJcDh8LjAzgrJLrTNi971KN3X6sh`, before revision `d88cc41b0145d5dc118a6313054c5d3e66efbe19`, and after revision `5f659dce24e13b30e996f0aab3419dac4c21f934`.

## Repair

The host now canonicalizes the admitted profile path and binds this explicit argv into each extension execution:

```text
--runtime-profile <canonical-path>
--runtime-profile-identity <admitted-blake3-identity>
```

The extension shell reads the document under a one-MiB bound, admits it, compares the resulting identity, and only then reads and handles the callback frame. The deterministic callback core still receives an admitted profile and performs no file or environment access.

## Pre-change evidence

- `cargo test -p kiln-aspen-extension`: PASS, 7 integration tests.
- `nix build path:$PWD#checks.x86_64-linux.kiln-aspen-host-package --no-link -L`: PASS.
- Native Cairn validation and proposal, design, and tasks gates: PASS.

## Changed-surface evidence

- `cargo test -p kiln-aspen-extension --all-targets --all-features`: PASS, including three new shell tests and four hosted tests.
- `cargo clippy -p kiln-aspen-extension --all-targets --all-features -- -D warnings`: PASS.
- Root workspace tests with all targets and features, excluding only the separately gated Pijul native fixture: PASS.
- Root workspace strict Clippy with all targets and features: PASS.
- Standalone host workspace tests with all targets and features: PASS, 19 tests. The new negative test proves profile drift after reservation remains `Unknown`, repeats without Lattice dispatch, and fails closed on restart.
- Standalone host strict Clippy with all targets and features: PASS.
- Root and standalone host formatting checks: PASS.
- `checks.x86_64-linux.aspen-runtime-profile`: PASS.
- `checks.x86_64-linux.aspen-runtime-octet-deny-all`: PASS with zero findings, warnings, or errors.
- `checks.x86_64-linux.kiln-aspen-host-package`: PASS with release-mode build and all-target, all-feature tests.
- `checks.x86_64-linux.kiln-aspen-host-octet-deny-all`: PASS with zero findings, warnings, or errors.
- Native Cairn validation: PASS.

The full all-feature root command reached only the existing native Pijul tests without `KILN_PIJUL_EXE`; every preceding test passed. The malformed-Cargo diagnostic comes from Aspen's intentional negative fixture. Neither diagnostic belongs to this changed surface.

Publication and live-shadow evidence will be appended before completion.

## Non-claims

These results do not prove provider truth, CI correctness, host sandboxing, global durability, distributed exactly-once execution, production availability, cutover readiness, or release eligibility. The failed operation is retained and must not be redispatched.
