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
- [x] [serial] Document capability-scoped views in the operator guide and run the focused Rust, Nickel and Cairn checks. r[kiln.source.projection.authority.vocabulary]

Evidence for the first nine tasks: `evidence/verification.txt` (focused Rust tests, clippy, fmt, Nickel gate, Cairn gates and coverage, run with the dev shell's toolchain directly). Evidence for the last task: `evidence/guide-checks.txt`, through `nix develop` on Rust 1.97.1: the operator guide's capability-scoped views section, the restricted corpus export check added to `scripts/check-nickel-fixtures.sh`, a `CHANGELOG.md` entry, and the focused Rust, Nickel, Cairn gate and development and accepted-only traceability checks.
