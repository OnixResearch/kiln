# Nix evaluation job discovery

<!-- r[impl kiln.nix_eval_jobs.nonclaims] -->

Kiln uses `nix-eval-jobs` only as an optional discovery provider. The adapter does not build tasks or change canonical CI state directly.

## Selected cohort

The initial profile admits this exact stock Nix cohort:

| Fact | Value |
|---|---|
| Upstream project | `NixOS/nix-eval-jobs` |
| Upstream revision | `a0cd02231c58974a6b5aaa3712069b071047162e` |
| Upstream license | `GPL-3.0` |
| Package version | `2.35.0` |
| Source version | `0.1.6` |
| Nix revision | `aa2613fa02ca6199fe0ebf61a0f7f6d781d32a47` |
| Nix version | `2.35.2` |
| Nixpkgs revision | `346dd96ad74dc4457a9db9de4f4f57dab2e5731d` |
| Platform | `x86_64-linux` |
| Executable | `/nix/store/bcv7hkwwpmfjmvxh6i106gdwr956svsp-nix-eval-jobs-2.35.0/bin/nix-eval-jobs` |
| Executable BLAKE3 | `dc42bc0e330c598ec93c2361afbb493417fa5c3364722897bd156d3f2ba9c063` |
| Cohort manifest | `/nix/store/wasviibxqrxh6h8wckaifm2s6lancgpn-kiln-nix-eval-jobs-cohort.json` |
| Manifest BLAKE3 | `102b4af9a753d00dfb6489e1593dd4b9ac8ffe9eed42518705329c6018e63c31` |
| Bounded Exec revision | `29dac88ecded94457572db3fdfaaaab95fa91525` |

The upstream executable has no `--version` option. The immutable cohort manifest binds both version values to the exact executable path.

The adapter verifies the executable bytes and manifest bytes before each new discovery. It does not use `PATH` or another evaluator.

## Approach decision

The selected mechanism runs one pinned process behind an application-owned discovery port. Bounded Exec owns process-group timeout, cancellation, output limits, and teardown.

Kiln rejected these mechanisms:

- Linking Nix libraries into `kiln-core` would add private ABI and provider dependencies.
- Copying upstream evaluator source would add maintenance and GPL source-distribution scope to Kiln.
- Converting every jobset to Mantle first would not support conventional external Nix jobsets.
- Using the host `nix-eval-jobs` command would permit silent executable and ABI drift.

## Source admission

The initial profile accepts only flake evaluation from an immutable `/nix/store` source path. Each request also binds these values:

- the source content identity;
- the root selector;
- the target system;
- the pipeline and policy identities;
- the discovery and build-provider profiles;
- the worker and memory bounds;
- the task, record, dependency, metadata, and output bounds.

Floating Git references, registries, expression mode, `--apply`, impurity, forced recursion, and undeclared imports are unavailable.

## Process and stream behavior

The shell clears the process environment. It supplies only the admitted home path and a fixed Nix configuration.

The operator provisions the home path. Nix can retain evaluator cache data there under operator policy.

The fixed argument policy can enable these upstream fields:

- `attr` and `attrPath`;
- `name`, `system`, `storeDir`, and `drvPath`;
- `outputs` and `inputDrvs`;
- `meta` and `requiredSystemFeatures`;
- `cacheStatus`, `isCached`, `neededBuilds`, and `neededSubstitutes`;
- per-job `error` and `fatal` values.

Bounded Exec captures a named maximum byte count. The schema decoder then consumes fixed-size chunks and admits complete newline-delimited records.

Malformed JSON, a partial final line, unknown fields, oversized data, duplicate attributes, missing outputs, and dependency cycles fail closed. Distinct attributes that share one derivation (flake package aliases) are admitted as separate task proposals; the selected task disambiguates them.

Only the explicit Onix decoder additionally admits the optional 2.36 `stats` object on success and error rows. It validates exactly `allocBytes` and `wallMs` as unsigned 64-bit integers, rejecting missing members, unknown members and malformed values, then excludes these producer timing/allocation advisories from normalized jobs, errors, tasks and observation identities. Stock decoding still rejects `stats`; this does not relax either cohort's other unknown-field checks.

A successful process exit does not complete discovery by itself. The stream, stderr, process, and GC-root facts must also be complete and consistent.

Timeout, cancellation, signal loss, output overflow, provider error, and unknown remain separate outcomes. An unknown observation can only reconcile to its recorded value.

## Task mapping

The pure core maps admitted successful jobs into task proposals. Each BLAKE3 mapping identity binds these facts:

- the discovery schema;
- the source, selector, and system;
- the pipeline and mapping policy;
- the upstream attribute and derivation;
- the output names and paths;
- the selected build provider.

Input derivations create task dependencies only when they match another discovered job. External build inputs remain provider facts.

Per-job evaluation errors remain visible. They do not suppress independent successful jobs after a complete process outcome.

The process shell cannot create tasks directly. `admit_discovery_observation` repeats the canonical mapping before it returns task proposals.

The core also recomputes the complete observation identity. Restored observations cannot substitute process, stream, task, or GC-root facts.

## Cache advisories

`cacheStatus` remains advisory. The deprecated `isCached` field must agree with the current field.

Cache observations cannot complete tasks, skip build-provider admission, authorize substitution, or prove output presence.

## GC-root handoff

The adapter gives each discovery an owned GC-root directory. The observed roots must exactly match the successful derivation records.

The adapter retains roots until every proposed task has a durable downstream handoff. Cleanup before that handoff fails.

A cleanup error changes the root state to `CleanupFailed`. It does not become clean completion.

## Explicit optional Onix profile

Stock Nix remains the default cohort above. `NixEvalProfile::onix(runtime, limits, source_binding)` selects the separate executable `onix-nix-eval-v1` profile on `x86_64-linux`; `NixEvalProfile::pinned` cannot select it. Stock admission rejects an Onix profile or evaluator requirement before GC-root creation or process launch. Stock profiles carry no Onix source binding, and absent optional fields retain the stock serialized shape and observation identity. Neither selection searches `PATH`, substitutes another plugin/evaluator, or falls back between stock and Onix.

### Frozen source and measured runtime

| Fact | Exact value |
|---|---|
| Fixture upstream | `https://github.com/onixcomputer/onix-modules` |
| Fixture commit | `69a131d71a509f9c57646b71113f75f39ab637b1` |
| Upstream fixture file | `fixtures/kiln-wasm-discovery/flake.nix` |
| Verbatim Kiln import | `runtime/onix-eval-cohort/jobset/flake.nix` |
| Runtime source root | `/nix/store/yxjjwvja5c8qfz10b6qh14bjd2bwrpww-kiln-onix-discovery-jobset-v1` |
| Source identity (BLAKE3 of `flake.nix` bytes, not a directory/NAR hash) | `dbb21080c65c18ce318cf95106f8b112edb1c29219700f846d0721c77d5abbfb` |
| Selector / system | `jobs.x86_64-linux` / `x86_64-linux` |
| Evaluator upstream | `NixOS/nix-eval-jobs@9f1553afabd95f9f59951f5c946e86005af7196a` |
| Evaluator / source version | `2.36.4` / `0.1.6` |
| Fork / Nix version | `onixcomputer/nix@f93f28ad9dce007baae13aa461c16abf656d1171` / `2.36.0` |
| Package review source | `OnixResearch/onixpkgs@3bdf51c00947ffd6b8bd3355ff83509a03752042` |
| Nixpkgs | `34ca302a9572963c02e385c056be37c85ff51b77` |
| Executable | `/nix/store/69k3iijnj5s8417anq7ncgc9f006gc1n-nix-eval-jobs-2.36.4/bin/nix-eval-jobs` |
| Executable BLAKE3 | `4b4e221bfe3645da5f9d89198f211e07f63e4ea5f4fbd6064d6311bfa5090361` |
| Required features (exact ordered set) | `flakes`, `nix-command`, `wasm-builtin` |
| Source-defined host handshake | `nix_wasm_init_v1` |
| Plugin source | `onixcomputer/onix-wasm@54bf09574c42a21f9f38dc7e01bc44440e6fbc07` |
| Plugin | `/nix/store/di3jrnpmrc4lff4581vxmprmcsgswd0r-nickel-plugin-0.1.0/nickel_plugin.wasm` |
| Plugin BLAKE3 | `6bbf0180617b4d87746416eb8d58d09dea1076312935038b4d92706e5c7626f6` |
| Plugin directory NAR (pure source import) | `sha256-T44jy5Ax7YawXUBgd76O5gjO1ERS6LBaU0D7axpE8+o=` |
| Effective pure-import plugin file | `/nix/store/5xm536a70fvv5k6xp7bijcy41xl7j628-source/nickel_plugin.wasm` (same plugin BLAKE3) |
| Cohort manifest | `/nix/store/bnigspvq8r3pxwy2cvkzaa6cgarbh1mi-kiln-nix-eval-jobs-onix-cohort.json` |
| Manifest BLAKE3 | `1e7df147057fde6b439541427a51c2d4ba70f6a42ee15d30021d120e9dda8263` |

The [importer](../scripts/import-onix-discovery-fixture.sh) reads that exact Git object, verifies its BLAKE3, and refuses to replace an existing different import. The [source receipt](../runtime/onix-eval-cohort/jobset.source.json) binds the upstream commit, original file and verbatim runtime import; dirty working-tree bytes are never imported. This is a dedicated review-source identity, not main/production or Radicle publication authority.

The input-free fixture explicitly admits the provisioned immutable plugin directory in pure evaluation with `builtins.fetchTree { type = "path"; path = "/nix/store/di3jrnpmrc4lff4581vxmprmcsgswd0r-nickel-plugin-0.1.0"; narHash = "sha256-T44jy5Ax7YawXUBgd76O5gjO1ERS6LBaU0D7axpE8+o="; }`, then appends `/nickel_plugin.wasm`. This content-pinned local import is neither a network fetch nor an auto-provisioning fallback. It supplies the effective plugin path to `builtins.wasm` with `function = "evalNickel"`; there is no speculative evaluator `--plugin` flag or `--impure` workaround. A bare external store path is not automatically accessible to pure flake evaluation. Real Nickel evaluation supplies the seed/report names and values. The report consumes the seed derivation; `broken` is an intentional independent evaluation error. The `/bin/sh` builders are discovery inputs, not evidence of host-shell availability or realized outputs.

The immutable manifest uses schema `kiln.nix-eval-jobs-cohort.v1`. `admit_cohort_manifest` checks the selected versions/executable and exact `OnixProvenance` returned by `onix_provenance()`, rejecting unknown fields. That provenance binds upstream revisions and source NARs, both fork-2.36 patch BLAKE3 values, the executable, source-defined handshake, feature set, plugin, package-review source, license declarations, fixture commit/path/content identity, and seven measured evaluator-linked `libnix*.so.2.36.0` path/hash pairs. External references include `contract_version = 1`. The manifest does not contain its own hash: the profile binds the original manifest bytes separately, avoiding a circular identity.

The seven library pins are recorded in [the manifest source](../runtime/onix-eval-cohort/manifest.nix) and exported as `ONIX_LINKED_COMPONENTS`. They describe the evaluator's measured Nix linkage, not the separately built OnixOS Nix CLI closure or every native/transitive dependency. Equal Nix version strings do not establish ABI equivalence, and the handshake label is not a versioned public host ABI guarantee.

### Separate acquisition and provisioning

The optional [subflake](../runtime/onix-eval-cohort/flake.nix) and its generated lock own this cohort. They are not root flake inputs or core dependencies. Default workspace builds, stock discovery, and root development/package selection do not acquire this Onix cohort; selecting the subflake is a separate source/credential/build boundary. The onixpkgs input is an exact `git+ssh` review-source pin and may require operator read access. A local cache hit does not prove anonymous acquisition. The review commit is not a production release.

From the Kiln repository root in the normal `nix develop path:$PWD` shell (which supplies Nickel and BLAKE3 tools without selecting Onix sources), with an OnixOS checkout containing the frozen commit:

```console
scripts/import-onix-discovery-fixture.sh "$ONIXOS_CHECKOUT"
nix flake archive --no-write-lock-file path:$PWD/runtime/onix-eval-cohort
nix build --no-write-lock-file path:$PWD/runtime/onix-eval-cohort#evaluator path:$PWD/runtime/onix-eval-cohort#plugin path:$PWD/runtime/onix-eval-cohort#manifest --no-link -L
nix eval --raw --no-write-lock-file path:$PWD/runtime/onix-eval-cohort#lib.jobset
nix build --no-write-lock-file path:$PWD/runtime/onix-eval-cohort#checks.x86_64-linux.cohort --no-link -L
nickel export --field profile config/nix-eval-jobs-onix-profile.ncl
```

The manifest references the source root; provision and retain the evaluator, plugin, manifest and jobset closures on the actual discovery host. The last Nix check measures artifacts and manifest facts; it does not run the Kiln discovery/handoff API. The typed Nickel export is the human-authored selection contract, not an automatic executable launcher or a serialized Rust `NixEvalProfile`. Only absolute `home_base` and `gc_root_base` are operator-variable in that contract; provision an owned home and root base with appropriate permissions.

The GC-root base must be UTF-8 and leave room for a path separator and the
64-byte BLAKE3 directory name inside Kiln's 128-byte provider-reference bound.
The same bound applies to stock and explicit Onix profiles. Profile admission
rejects unrepresentable derived root identities before directories or evaluator
processes are created; the shell must not silently discard a failed directory
reference and report missing retention only after evaluation.

<!-- r[impl kiln.onixos_pilot.source] -->

Compose `NixEvalRuntimeBinding` from the exact exported executable/manifest paths and BLAKE3 values plus the owned absolute home/root paths. Separately supply `OnixSourceBinding { source: ExternalRef, source_identity: ContentId, root_selector: Alias, system: Alias }`, then call `NixEvalProfile::onix(runtime, limits, source_binding)` and `NixEvalAdapter::new(profile, BoundedNixEvalRuntime)`. The source reference must use `kiln-source` / `nix-store-path` and a direct canonical `/nix/store/<32-character Nix base32 hash>-<name>` root, not a mutable checkout, URI, nested path, traversal or fragment/query. Supported source names contain ASCII letters, digits, `+`, `-`, `.`, and `_`. The identity is BLAKE3 of that root's exact `flake.nix` bytes, not a directory/NAR hash. The selector must use the existing supported attribute-selector syntax; the system is exactly `x86_64-linux`, and requests must use Flake mode. Pure admission compares the request's complete source reference, identity, selector and system to this binding.

The fixed evaluator/plugin cohort is independent of this explicit consumer binding. A discovery plan must select `ONIX_PROFILE` and carry `Some(onix_cohort()?)`; pipeline, policy, task mapping, build-provider selection and finite limits remain explicit consumer inputs. The fixture above remains one explicitly selected conformance consumer: supply its source reference, `ONIX_SOURCE_BLAKE3`, `ONIX_ROOT_SELECTOR`, and `x86_64-linux`. There is no fixture default or compatibility constructor. A different frozen source needs its own explicit measured binding; fixture compatibility does not qualify that consumer or its build.

`config/nix-eval-jobs-onix-profile.ncl` remains the human-authored exact fixture-selection contract described above, not a generic source-binding builder or serialized Rust profile. Its fixed source fields select that fixture only; they must not be represented as the separately frozen OnixOS pilot consumer. Its only operator-variable fields remain the two owned absolute runtime directories.

At adapter construction and before each new discovery, bounded no-final-symlink opens measure the executable, manifest, plugin, seven linked Nix libraries, and the explicitly bound source's `flake.nix`. `O_NOFOLLOW` rejects a symlink final component; this is not a promise that all parent directory components are symlink-free or that the process is confined. Profile/source-binding admission, manifest schema/provenance, measured bound flake bytes and all request identities are checked before creating GC roots or launching the evaluator. Drift is a domain failure (`onix_executable_drift`, `onix_manifest_drift`, `onix_plugin_drift`, `onix_component_drift`, `onix_source_drift`, or the corresponding profile/feature/handshake mismatch), not permission to try stock or an ambient executable.

### Live rail and outcome contract

After provisioning the exact optional cohort, run the actual bounded Kiln API rail separately:

```console
cargo test --locked -p kiln-adapter-nix-eval --test onix_live exact_onix_discovers_nickel_jobs_maps_dependency_and_releases_roots -- --ignored --exact --nocapture
```

This ignored-by-default rail calls the real evaluator through `BoundedNixEvalRuntime`, admits the observation, checks the Nickel seed/report jobs and dependency, retains the independent `broken` error, rejects stock admission, refuses premature root cleanup, and performs durable task handoff before releasing roots. A command listing is not a passing receipt: retain its actual output against the exact checked source and measured cohort.

Onix uses the same stream, pure mapping, terminal classification and GC-root handoff as stock. `Complete` requires a successful exit, complete valid bounded stream/stderr and matching roots; a nonfatal per-job error can coexist with independent proposals. Fatal job errors yield `Failed`; nonzero exit, invalid/incomplete stream, output overflow or inconsistent roots yield `ProviderError`; timeout and cancellation stay distinct; a signal or unknown process completion yields `Unknown`, never synthetic success. Call `handoff_gc_roots` with every durably accepted task ID before `cleanup_gc_roots`; cleanup failure remains `CleanupFailed`. Discovery and cache advisories neither build tasks nor authorize substitution.

Onix job errors remain `nix_job_evaluation_error` even when a job name, source path or user-record error mentions `wasm`; the admitted Onix profile already supplies that capability. Unsupported capability and cohort drift are profile/preflight decisions, not a substring-based fallback under Onix. Stock retains its existing unsupported-builtin classification.

Retained qualification belongs in [the integration receipt](../.cairn/archive/1970-01-01-add-onix-nix-evaluator-cohort/evidence/verification.json), separately identifying artifact checks and the actual Kiln API rail. Earlier source/manifest checks do not qualify a changed fixture or prove live discovery.

## Evidence and non-claims

The pinned upstream package passes its unit suite. The Kiln Nix check also runs positive job, job-error, dependency, cache, GC-root, and `builtins.wasm` fixtures.

This evidence does not prove these properties:

- Nix or Nickel evaluation correctness;
- source trust;
- complete dependency semantics;
- cache truth;
- build execution or output correctness;
- general Onix or stock Nix compatibility;
- reproducibility;
- release readiness;
- hostile-code confinement;
- a stable public Wasm host ABI;
- production-release status of the package review source.
