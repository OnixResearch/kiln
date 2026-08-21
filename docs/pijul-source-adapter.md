# Pijul source adapter

Kiln uses `kiln-adapter-pijul` for bounded, read-only access to local Pijul repositories. The adapter preserves Pijul state and change identities. It does not convert them to Git commits or branches.

## Compatibility cohort

The admitted cohort is exact:

| Component | Version or identity | Source revision | License |
|---|---|---|---|
| Pijul fixture producer | `1.0.0-beta.11` | crates.io SHA-256 `8a4fc27aa81ee061310d57fce2df9cc45f3149ddb00bdfab2b816beb0359b13d` | `GPL-2.0-or-later` |
| `libpijul` | `1.0.0-beta.11` | crates.io SHA-256 `f625e90234ef1fd164dfe7980ffb04a3036a08e31e051cc29e6273a0c9a7a7e7` | `GPL-2.0-or-later` |
| `pijul-repository` | `1.0.0-beta.10` | crates.io SHA-256 `12b9f22e42a5149909b2d1d20706a5ee5042a1ec60330d94124a213e4e294aa5` | `GPL-2.0` |
| Pristine database | Sanakirja format version `1` | `sanakirja-pristine-v1` | provider format |

The Pijul `1.0.0-beta.11` lock file selects this exact library cohort. Later Pijul releases use `pijul-core`. They are outside this profile.

The Nix fixture producer is `/nix/store/dm7km3dr8q5jbyakb7bbw8j6v4vz9lck-pijul-1.0.0-beta.11/bin/pijul`. Its BLAKE3 identity is `8000c44ea14cdcca49730073837737339c55852d6af3f4d3703092a944e38f77`.

The supported platform is `x86_64-linux`. The adapter crate uses `GPL-2.0-only` because the selected repository wrapper declares `GPL-2.0`.

## Profile

Use `config/pijul-source-profile.ncl`. The profile admits these capabilities only:

- discover the configured local channel;
- resolve one exact native channel state;
- acquire the bytes for that state; and
- observe the bound identities.

The profile rejects network access, credentials, collaboration, triggers, status publication, and source mutation.

Named bounds limit repository members, repository bytes, channels, changes, entries, path bytes, file bytes, and total snapshot bytes. The profile admits regular files, executable files, and directories.

## Read behavior

The operator supplies one explicit repository root and one channel. The adapter performs these steps:

1. It validates the profile before repository access.
2. It checks the `.pijul/pristine/db` and `.pijul/changes` layout.
3. It bounds repository members and stored bytes before opening the native database.
4. It opens one native transaction view.
5. It reads the channel Merkle state and bounded change sequence.
6. It archives that same view into bounded memory entries.
7. It checks the state again in the same view.
8. Pure core code validates and orders the entries.
9. Kiln computes the BLAKE3 tree identity from the canonical entry manifest.

The adapter does not read source bytes from the working copy. Unrecorded working-copy changes cannot alter the admitted snapshot.

## Native identity

The exact revision is the provider-produced Pijul Merkle state. It is not one change hash. The observation also binds:

- the canonical local repository identity;
- the requested channel;
- the ordered change sequence;
- the pristine format cohort; and
- the state before and after acquisition.

Kiln computes a separate observation identity over these facts. A changed, duplicated, malformed, or substituted fact fails admission.

## Diagnostics

The adapter keeps these conditions distinct:

- missing repository;
- missing channel;
- busy repository;
- damaged repository or missing change data;
- unsupported pristine format;
- changed state;
- unsupported or unsafe entry; and
- exceeded bound.

No failure returns partial source success.

## Non-claims

A passing result proves only the checked local state, bytes, profile, bounds, and identities. It does not prove:

- source trust;
- signature validity;
- complete history;
- remote availability;
- collaboration status;
- mutation success; or
- history equivalence with Git, Jujutsu, or another Pijul state.

Equal canonical trees prove only equal admitted bytes under the Kiln framing rule.
