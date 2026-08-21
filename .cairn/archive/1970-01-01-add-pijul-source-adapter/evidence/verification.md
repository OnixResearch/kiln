# Verification evidence

## Result

The Pijul source adapter resolves one configured local channel through a concrete `libpijul` shell. It returns one exact Merkle state and one BLAKE3 tree identity.

The shell does not expose source mutation, network, credential, trigger, status, or collaboration operations.

## Compatibility evidence

The selected cohort is exact:

- Pijul `1.0.0-beta.11`;
- `libpijul` `1.0.0-beta.11`;
- `pijul-repository` `1.0.0-beta.10`;
- Sanakirja pristine format version `1`; and
- `x86_64-linux` support only.

The Pijul source archive SHA-256 is `8a4fc27aa81ee061310d57fce2df9cc45f3149ddb00bdfab2b816beb0359b13d`.

The pinned executable is `/nix/store/dm7km3dr8q5jbyakb7bbw8j6v4vz9lck-pijul-1.0.0-beta.11/bin/pijul`.

Its BLAKE3 identity is `8000c44ea14cdcca49730073837737339c55852d6af3f4d3703092a944e38f77`.

The Pijul lock file selects the exact library and repository-wrapper versions. The Nix cohort check verifies these values from the immutable source archive.

## Behavioral evidence

The pure core validates provider namespaces, state consistency, change bounds, observation identity, snapshot paths, entry kinds, duplicates, and byte bounds.

The native fixture gate covers these cases:

- one change;
- dependent changes;
- separate channels;
- repeatable acquisition;
- equal source bytes with different Pijul histories;
- unrecorded working-copy bytes;
- missing channels;
- missing changes;
- damaged pristine state; and
- repository storage bounds.

Fault fixtures also cover moved states, unsafe paths, duplicate paths, unsupported entries, oversized files, oversized histories, and total-byte limits.

## Validation evidence

The following checks passed:

- baseline workspace tests before core changes;
- 230 final workspace tests;
- 10 Pijul adapter unit tests;
- 4 native Pijul repository tests;
- strict workspace Clippy with warnings denied;
- Rust formatting;
- `kiln-core` for `wasm32-unknown-unknown`;
- the positive Nickel profile;
- 8 negative Nickel profiles;
- core, provider API, standalone, and CLI dependency-tree isolation;
- Pijul cohort source, lock, version, executable, and BLAKE3 checks;
- 11 `x86_64-linux` flake checks; and
- explicit unsupported Pijul checks for `aarch64-linux`.

The Cairn task gate receipt is `bac0d02f09c01fd0b6829f91c827e1a330aaf92d838a69134dedd4902cde6550`.

The specification sync receipt is `a9654d474884b37f49c2381707018748476249a1ef74f182b9046893966c1518`.

The archive receipt is `7638b007bd2b2fbaec65021b4113c4de0ad366a9c69434af66a617af8dfa556b`.

Tracey references all 470 accepted requirements. Its receipt is `4fb1bf534d97c03c678e368d881afc2e88cd38b9deb8bbf9ab921af062cc3f55`.

The deterministic release-evidence identity is `8ef2ec8f5a0dffe9c74b1857ebdb8438e48c711bedae9e0728a721e8600baa80`.

Kiln has no root Octet metadata profile. This change does not claim an Octet result.

## Claim boundary

These results prove bounded local observation under the pinned cohort. They do not prove source trust, signature validity, complete history, or remote availability.

Equal canonical trees prove equal admitted bytes only. They do not prove equal Pijul history or cross-provider history equivalence.
