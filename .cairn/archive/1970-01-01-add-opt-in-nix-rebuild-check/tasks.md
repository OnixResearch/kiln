## Phase 1: Profile and observation

- [x] [serial] Add the optional off-by-default `rebuild_check` profile field and Nickel field. r[kiln.nix_build.rebuild_check]
- [x] [serial] Add typed matched/differed/not_checkable rebuild observations with exact paths and bounded log identities. r[kiln.nix_build.rebuild_check]
- [x] [serial] Admit rebuild observations only for exact successful products and refuse retained paths on non-differences. r[kiln.nix_build.rebuild_check.scenario.admission]

## Phase 2: Execution and verification

- [x] [serial] Run the bounded `--realise --check -K` invocation after realisation and rooting. r[kiln.nix_build.rebuild_check]
- [x] [serial] Record matched, retained difference and not-checkable cases without changing the build outcome. r[kiln.nix_build.rebuild_check.scenario.matched] r[kiln.nix_build.rebuild_check.scenario.differed] r[kiln.nix_build.rebuild_check.scenario.not_checkable]
- [x] [serial] Document the option and retain the reproducibility non-claim; run focused adapter and core tests. r[kiln.nix_build.rebuild_check]
