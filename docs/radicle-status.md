# Radicle CI status publication

Kiln can publish a local `xyz.radicle.ci-job` collaborative object. The object is a rebuildable projection of Kiln state.

Kiln remains the source of CI state. A Radicle object cannot complete a run, authorize a merge, or change a release decision.

## Pinned cohort

The adapter supports `x86_64-linux`. It pins these crates:

| Component | Version | Crates.io SHA-256 |
|---|---:|---|
| `radicle-cob` | `0.21.1` | `8a6b295464ff130975fc151feb8aef4d6fd22ddd1da2a5a2a561f283c7abd41e` |
| `radicle-crypto` | `0.19.0` | `76de6172f3ccae0f489578f4a5da45bc77ff8855ace6352b002d94c0e0783bb5` |
| `radicle-oid` | `0.2.2` | `7a0b9a4cbe2b75f8d95a4c887836b909542da704673e708d3b85186a4664400c` |
| `radicle-git-ref-format` | `0.2.0` | `8cd8067caf7de3c75f2736c24da52081b5fa9f9f2aa6357b8ad8d67ba5411ec6` |
| `git2` | `0.21.0` | `ddddbf932745a6be37109b6112d3ee09696106f848449069d3a57bba937ab82e` |

These crates use `MIT OR Apache-2.0` terms. The adapter does not link the full `radicle` runtime or its SQLite store.

This boundary avoids a native SQLite cohort conflict. It also keeps node, network, and collaboration authority outside this change.

## Shared Git storage source

The status shell and the declared patch-review transport reuse `onix-radicle-cob-git-store` v0.2.0 for the Radicle 0.21 cohort's local signed COB refs and Git storage traits. The library borrows the already opened Git repository; Kiln owns its ephemeral commit configuration, SecretSpec resolution, RID and actor admission, CI reducer, and provider observations. The older 0.20 v0.1.1 source used by Cairn and Valence is not interchangeable with this pin; v0.2.0 supports namespaced refs only.

Kiln vendors source commit `a87ca01412fb62e69171925d79d2d6b0a23a7e72`, verified against BLAKE3 `7c2902aee85ebcc923b1e7f9004d6235b550ddf18053194dd6c9286ef05859e6` of `git archive --format=tar` for that commit. The checked import lives at `vendor/radicle-cob-git-store/`; `vendor/radicle-cob-git-store.source.json` records its origin. Cargo resolves this in-repository source, never a mutable sibling checkout or a remote. The upstream source repository has no declared publication remote.

To verify or import from an explicitly supplied local source checkout, run `scripts/import-radicle-cob-git-store.sh <checkout>`. The script checks the recorded commit and tar digest, compares every existing vendored file's bytes and executable mode (including symlink targets), and refuses any drift; an identical import is a no-op.

A future upgrade first runs the **old** importer to verify the existing vendor tree against its old recorded archive. Only after that succeeds, review the new cohort/source commit and digest, change the importer and source record together, move the verified old vendor directory aside in a new change, import the new exact archive and compare its files, run consumer tests, then remove the old directory from that change. Never overwrite an unverified vendor tree or update a pin to make existing edits appear valid.

The adapter writes this exact reference layout:

```text
refs/namespaces/<signer-node-id>/refs/cobs/xyz.radicle.ci-job/<object-id>
```

## SecretSpec signer

The production profile pins SecretSpec `0.19.0` from its crate source. The raw source SHA-256 is `827137f2e4d608acb1331aaffc91b16e457903d41964939fb8e716c3be557827`.

The Nix build uses revision `b7c2ada94fe99c15b0dbcf4d11fd7850b957a436` for the package toolchain and recipe.

The pinned executable is:

```text
/nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec
```

Its BLAKE3 identity is:

```text
c15ea6408e2ff98fd976507a2ae5bec3cb99ef5580ce7342ba92eb465d9b28a6
```

Kiln verifies this identity before each resolution. Kiln then starts SecretSpec through Bounded Exec with a cleared environment.

The declaration contains a project root, project name, profile, secret name, and optional provider restriction. It never contains the secret value.

Configure the declared secret as a 64-character hexadecimal Ed25519 seed. Do not put this value in Nickel, command arguments, logs, or receipts.

The resolver runs this command shape:

```text
secretspec get <SECRET_NAME> --profile <PROFILE> --provider <PROVIDER> --reason <REASON>
```

The process has fixed time, input, output, polling, and teardown bounds. Missing values, drift, invalid seeds, timeouts, and output overflow fail closed.

Diagnostics never include standard output or standard error. `ResolvedSigner` debug output replaces seed data with `[redacted]`.

## Target admission

`config/radicle-status-profile.ncl` owns the typed review profile. A deployment must replace the fixture RID with each exact admitted repository RID.

The profile admits only `xyz.radicle.ci-job`. It denies network-peer access, merge authority, and canonical-state mutation.

Kiln checks the RID and payload before it resolves the signer. Foreign RIDs and object types cannot reach repository storage.

## First write and replay

A first-write plan omits the object ID. Radicle derives the object ID from the signed root change.

Kiln returns the native object ID and current head in the successful observation. The journal must retain both values for later exact updates.

Before first-write creation, Kiln scans a bounded set of local ci-job objects for the idempotency key. One matching object becomes an idempotent replay.

No match creates a new object. More than one match rejects the plan as ambiguous.

An exact update names the returned object ID and expected head. Kiln rejects a missing object or stale head instead of writing a substitute.

The Git reference update uses an atomic expected-target comparison. A same-signer race becomes unknown and requires reconciliation.

A repeated key must bind the same run ID and outcome. Different facts under the same key produce `status_idempotency_conflict`.

## Single-writer policy

Kiln accepts no current head or one head from the resolved signer. It rejects a foreign head, an unknown author, or multiple heads.

The shell supplies deterministic commit name and email values through an ephemeral Git configuration. It does not use ambient Git identity.

Discovery also applies this policy to each object it reads. The adapter does not merge concurrent histories.

## Unknown results and reconciliation

A COB write can store a change before its final reference result is clear. Kiln returns `StatusDispatch::Unknown` for this condition.

Unknown does not mean failed. The application must persist it and call `reconcile` without redispatch.

Reconciliation reads storage and searches by the same bounded idempotency key. It never signs or writes a new change.

## Bounds and diagnostics

The default payload limit is 65,536 bytes. The default object-scan limit is 256 objects.

Profiles can reduce these limits. Rust admission rejects zero limits and values above the absolute adapter limits.

Stable diagnostics include these cases:

- `status_repository_not_admitted`
- `status_object_missing`
- `status_foreign_head`
- `status_multi_head`
- `status_stale_head`
- `status_idempotency_conflict`
- `status_idempotency_ambiguous`
- `status_payload_bound`
- `radicle_object_scan_bound`
- `radicle_status_executable_drift`
- `radicle_status_signer_unresolved`

## Evidence boundary

Twelve native status tests call the public `RadicleStatusShell::publish` and `.reconcile` against temporary bare Git repositories with fixture-only signers. They cover creation, exact updates, repeats, authenticated two-namespace heads, malformed and mismatched-author refs, bounds, stale preflight, ambiguous publication, redaction, and reconciliation. Four shared-store fixtures additionally exercise signature rejection, symbolic refs, duplicate-namespace distinct-ID counting, zero/overflow bounds, and exact CAS against a moved ref. Offline Kiln workspace check, test, strict clippy, and formatting checks pass. The import script was exercised with an exact first import, an identical reimport, and refusals for edited bytes, changed executable mode, and a mismatched source record.

These checks prove bounded local writes under one profile. They do not prove a stable filesystem snapshot, universal concurrency safety, peer convergence, remote visibility, actor trust, node availability, CI correctness, or release readiness.
