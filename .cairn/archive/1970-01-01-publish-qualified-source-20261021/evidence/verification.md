# Verification

## Candidate

- Source revision `c955334f54bdfe6e155e56c00573ca7ba77c4fb1`; previous canonical `main` `305f85d80b9ea05fbd91db73de8a6662a55650ee`.
- Deterministic normalized USTAR source archive BLAKE3 `28fec0fa2b3a71c4c4e725d4640080a07d18d4a5929ada05d7cd9b81eff38004`, 60,989,440 bytes, 4,439 tracked members.
- Lexically sorted path manifest `source-paths.json` BLAKE3 `8001e6b5cbc334cdb1d46611e71d824a4deee82d399937983a19743c75a45383`, 4,439 entries.
- Hosting profile source `1f650f733802d74f9064da668a337948a86f82e2f9340f2d68e6c670fbf15de0`, export `31c16d1a45192767661610661c3534a17e0f927e887c9edbb24f4d98191e4d82`.
- License map `LICENSES.md` BLAKE3 `a099bfe50cec65cead787f60588dfa2d751bc5c4c88e80698b53f8c364eed6c0`.
- Self-CI verification `5cd60bda97b05aeb6a9ae41023954bf26a9f8fcfa542e77d69f6077b8f590161`; cases `Success`, `DomainFailure`, `Pending`.

## Gates

| Gate | Result |
|---|---|
| `cargo test --locked --workspace` | passed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | passed |
| `cargo fmt --check` | passed |
| `cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | valid |
| `cairn traceability coverage --root . --policy ...` | pass, no missing or dangling references |
| `cairn release-readiness --root .` | verdict pass |
| `nix build path:$PWD#checks.x86_64-linux.octet --no-link -L` | 0 warnings, 0 errors |
| `nix flake check path:$PWD --no-write-lock-file -L` | all checks passed |

## Publication

- One `jj git push --remote rad --bookmark main` fast-forwarded canonical `refs/heads/main` from `305f85d8` to `c955334f` and synced with 2 seeds. No force push, history rewrite, governance change, announcement, deployment or mirror mutation occurred.
- Signed refs advanced from `e9d43a1cbe8befa6a1fc08dbe5630a99e23599f7` to `27455a353402ecaa0bd9c6500a953c6d341c1575`. One delegate, threshold one, public visibility, unchanged.
- Native Radicle and anonymous Garden HTTPS acquisitions both reproduced `c955334f`, 4,439 tracked paths, the path manifest and the archive identity.
- A credential-isolated anonymous clone with a cleared home, disabled global and system Git configuration, no credential helper, no askpass, no inherited SSH agent and Git protocol v0 ran `nix develop path:<clone> -c cargo test --locked --workspace` to completion with exit 0.
- The Garden namespace and the read-only GitHub projection were read; the projection still serves `305f85d8` and was not mutated.
