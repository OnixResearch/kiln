## Phase 1: Vocabulary and port

- [x] [serial] Add the closed source-view vocabulary and coverage map with positive and negative tests. r[kiln.source.projection.authority.vocabulary]
- [x] [serial] [depends:kiln.source.projection.authority.vocabulary] Add the exact source-view request, per-component effective scope and snapshot rules with tests. r[kiln.source.projection.authority.request] r[kiln.source.projection.authority.scope]
- [x] [serial] [depends:kiln.source.projection.authority.scope] Add the source authority port, the local-operator full-source profile and a fake verified port for tests. r[kiln.source.projection.authority.port] r[kiln.source.projection.authority.compatibility]

## Phase 2: Disclosure

- [x] [serial] [depends:kiln.source.projection.authority.port] Add the restricted audience view, filtered before it is shaped, with the forbidden-only invariance property. r[kiln.source.projection.disclosure.restricted]
- [x] [serial] [depends:kiln.source.projection.disclosure.restricted] Add keyed handles, cursors and view bindings rechecked on use. r[kiln.source.projection.disclosure.handles]
- [x] [serial] [depends:kiln.source.projection.disclosure.restricted] Add the uniform unavailable shape and the distinct coverage states. r[kiln.source.projection.disclosure.unavailable]
- [x] [serial] [depends:kiln.source.projection.disclosure.handles] Add restricted Cairn context through change path authority, with compile-fail evidence that it is never complete. r[kiln.source.projection.disclosure.cairn]

## Phase 3: Sharing and fixtures

- [x] [serial] [depends:kiln.source.projection.disclosure.restricted] Add frozen view ceilings and attenuated share admission with the attenuation property. r[kiln.source.projection.sharing.ceiling] r[kiln.source.projection.sharing.attenuation]
- [x] [serial] [depends:kiln.source.projection.sharing.attenuation] Add the Nickel source-authority contracts, negative share fixtures and the shared restricted corpus export. r[kiln.source.projection.disclosure.restricted.scenario.audiences] r[kiln.source.projection.sharing.attenuation.scenario.corpus]
- [ ] [serial] Document capability-scoped views in the operator guide and run the focused Rust, Nickel and Cairn checks. r[kiln.source.projection.authority.vocabulary]

Evidence for the checked tasks: `evidence/verification.txt` (focused Rust tests, clippy, fmt, Nickel gate, Cairn gates and coverage). The last task stays open: the guide is updated and the focused checks pass, but they ran with the dev shell's toolchain directly, not through `nix develop`. The `nix develop` route, `nix flake check`, the corpus export check in `scripts/check-nickel-fixtures.sh` and a `CHANGELOG.md` entry are left for integration.
