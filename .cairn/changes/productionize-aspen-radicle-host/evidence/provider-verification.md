# Bounded Radicle Nix provider verification

Date: 2026-08-28

## Composition

The provider is the standalone `runtime/kiln-radicle-nix-provider` workspace. The root flake packages it separately from Kiln's domain workspace.

The provider pins:

- Bounded Exec `29dac88ecded94457572db3fdfaaaab95fa91525`;
- Durable File Publication `8e05e74e24b45f752d77145c4455385daaf6d6ab`;
- Octet `cf04e894e53eb0947230118a086ef6066ddba38c`.

## Checks

- workspace tests, all targets, all features: PASS, 4 positive and negative tests;
- strict Clippy, all targets, all features, `-D warnings`: PASS;
- direct Octet with deny warnings: PASS, zero findings;
- Nix package check: PASS;
- Nix Octet deny-all check: PASS, zero findings;
- Nickel positive profile and negative zero-user fixture: PASS.

The tests cover exact arguments, a cleared child environment, successful durable replay, failed exit, output flood, timeout, wrong repository, wrong user, wrapper identity drift, malformed JSON, and unknown profile fields.

## Claim boundary

The provider reports bounded local process and publication observations. It does not prove source trust, sandboxing, hermeticity, CI correctness, external report delivery, storage-device persistence, distributed exactly-once execution, or release eligibility.
