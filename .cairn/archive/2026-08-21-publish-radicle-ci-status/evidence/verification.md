# Verification evidence

## Result

Kiln publishes bounded local `xyz.radicle.ci-job` objects through one explicit SecretSpec signer declaration.

The writer applies exact RID, object-type, signer, object, head, payload, scan, run, and idempotency checks. Unknown writes require read-only reconciliation.

Kiln remains the canonical CI state owner. The adapter has no merge, collaboration, peer-network, remote-visibility, or release authority.

## Compatibility evidence

The supported platform is `x86_64-linux`.

The SQLite-free Radicle cohort is exact:

- `radicle-cob` `0.21.1`, source SHA-256 `8a6b295464ff130975fc151feb8aef4d6fd22ddd1da2a5a2a561f283c7abd41e`;
- `radicle-crypto` `0.19.0`, source SHA-256 `76de6172f3ccae0f489578f4a5da45bc77ff8855ace6352b002d94c0e0783bb5`;
- `radicle-oid` `0.2.2`, source SHA-256 `7a0b9a4cbe2b75f8d95a4c887836b909542da704673e708d3b85186a4664400c`;
- `radicle-git-ref-format` `0.2.0`, source SHA-256 `8cd8067caf7de3c75f2736c24da52081b5fa9f9f2aa6357b8ad8d67ba5411ec6`; and
- `git2` `0.21.0`, source SHA-256 `ddddbf932745a6be37109b6112d3ee09696106f848449069d3a57bba937ab82e`.

The production signer uses SecretSpec `0.19.0`. Its raw crate SHA-256 is `827137f2e4d608acb1331aaffc91b16e457903d41964939fb8e716c3be557827`.

The pinned executable is `/nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec`.

Its BLAKE3 identity is `c15ea6408e2ff98fd976507a2ae5bec3cb99ef5580ce7342ba92eb465d9b28a6`.

The full Radicle runtime is absent from `Cargo.lock`. This avoids its conflicting SQLite dependency cohort.

## Behavioral evidence

Pure tests cover profile admission, allowlists, expected heads, outcome mapping, idempotency conflicts, substitutions, malformed signers, zero seeds, and bounds.

Native tests create bare Git repositories with the pinned cohort. They cover these cases:

- first-write object creation;
- repeated idempotent writes;
- exact-object updates;
- stale expected heads;
- foreign signer heads;
- missing exact objects;
- conflicting replay facts;
- object-scan bounds;
- unresolved signer declarations;
- secret redaction;
- unknown reference publication; and
- read-only reconciliation without redispatch.

The shell uses Bounded Exec with a cleared environment. It verifies the executable before each signer resolution and zeroizes resolved output text.

The writer supplies deterministic commit display metadata through an ephemeral Git configuration. It does not use ambient Git identity.

## Validation evidence

The following checks passed:

- 248 workspace tests;
- 30 Radicle adapter tests in the normal workspace run;
- 18 focused status unit tests;
- 9 native status repository tests;
- strict workspace Clippy with warnings denied;
- focused Radicle Clippy with native fixtures enabled;
- Rust formatting;
- `kiln-core` for `wasm32-unknown-unknown`;
- the positive Nickel status profile;
- 5 negative Nickel status profiles;
- core and standalone runtime-independence checks;
- the Radicle and SecretSpec cohort check;
- the native status Nix fixture check;
- strict Cairn validation;
- proposal, design, and task gates; and
- all `x86_64-linux` flake checks.

The proposal gate receipt is `09b0db907e0e009fdcf0640d3d2556bfdf10dbb421160c259a7380faf88ce781`.

The design gate receipt is `b8f42cdef725221d77d2f55197ce7ebfa09c41b4dcac1afaecdce4c287842abf`.

The task gate receipt is `5867549a1c90958d4a55ac3672ab4cbac19b74bf537c96f8915f15c7184d7a95`.

The specification sync receipt is `35358218f6b5ff69398a7764e24f1a8628a9b6c1b820f58ba7dc34dba7da7016`.

The archive receipt is `c3b551ba89f253b33082f1a962882d877fdc448792ef5625b79139175194afae`.

Tracey references all 482 accepted requirements. Its receipt is `f7002cd4eb8da615666c99237ef481e608646d7c2ded8a49a8c38b0d8dd2d9e4`.

The deterministic release-evidence identity is `e95f6dff0eaaa7400b042574a5cbf4fdb47b6c5110d98733b5b4122064f91611`.

Kiln has no root Octet metadata profile. This change does not claim an Octet result.

## Claim boundary

These results prove bounded local COB writes under the pinned profile. They do not prove peer convergence, remote visibility, actor trust, node availability, or CI correctness.
