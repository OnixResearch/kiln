## Tasks

- [x] Add OpenSpec proposal/design/tasks/spec delta for caveat-preserving attenuation.
- [x] Audit the current effective-delegation/caveat-preservation code path and identify whether an exact vendored primitive exists or must be added first.
- [x] Implement an exact verified bridge or add focused shell regressions documenting why the check remains shell-local.
- [x] Add tests for same-parent containment plus caveat preservation, wrong-parent preservation, missing caveats, and successful attenuated caveat preservation.
- [x] Update docs/bridge guard commentary to avoid claiming opaque caveat preservation is verified unless it routes through an exact primitive.
- [x] Run focused/full validation, archive the change, commit, push, and verify a clean tree.
