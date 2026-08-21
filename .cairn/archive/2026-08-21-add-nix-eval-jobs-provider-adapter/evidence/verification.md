# Verification evidence

Date: 2026-08-21

## Goal

The change adds bounded stock Nix job discovery without moving Nix authority into Kiln core crates.

Completion requires exact cohort admission, bounded process observations, pure task mapping, GC-root handoff, positive tests, and negative tests.

False completion includes ambient `PATH` lookup, floating sources, unbounded output, direct shell task creation, trusted cache claims, or Onix Nix claims.

## Selected cohort

- Upstream revision: `a0cd02231c58974a6b5aaa3712069b071047162e`.
- Upstream license declaration: `GPL-3.0`.
- Package version: `2.35.0`.
- Source version: `0.1.6`.
- Nix revision: `aa2613fa02ca6199fe0ebf61a0f7f6d781d32a47`.
- Nix version: `2.35.2`.
- Nixpkgs revision: `346dd96ad74dc4457a9db9de4f4f57dab2e5731d`.
- Platform: `x86_64-linux`.
- Executable: `/nix/store/bcv7hkwwpmfjmvxh6i106gdwr956svsp-nix-eval-jobs-2.35.0/bin/nix-eval-jobs`.
- Executable BLAKE3: `dc42bc0e330c598ec93c2361afbb493417fa5c3364722897bd156d3f2ba9c063`.
- Cohort manifest: `/nix/store/wasviibxqrxh6h8wckaifm2s6lancgpn-kiln-nix-eval-jobs-cohort.json`.
- Manifest BLAKE3: `102b4af9a753d00dfb6489e1593dd4b9ac8ffe9eed42518705329c6018e63c31`.
- Bounded Exec revision: `29dac88ecded94457572db3fdfaaaab95fa91525`.

The upstream executable has no `--version` option. The immutable manifest binds the Nix and adapter versions to the executable path.

The executable build passed the upstream C++ unit suite. Its Nix store references bind the Nix `2.35.2` component set.

## Search and audit record

The search used three correlated evidence lenses because subagent consent was not available.

| Family | Mechanism | Result |
|---|---|---|
| optional process adapter | Run one exact executable behind a discovery port | validated |
| native core integration | Link Nix private libraries into Kiln | rejected because it breaks the core boundary |
| source copy | Copy evaluator protocol and implementation code | rejected because it adds maintenance and license coupling |
| Mantle conversion | Require conversion before conventional job discovery | deferred because it does not meet external compatibility needs |
| ambient host tool | Resolve `nix-eval-jobs` from `PATH` | falsified by executable substitution tests |

The retrieval budget was three source lanes and two audit rounds. The lanes covered upstream locks, upstream source, and local architecture.

The adversarial round covered the missing version option, malformed streams, executable drift, GC-root mismatch, cache conflicts, and Onix-only builtins.

This round also found two implementation gaps. The final code cleans empty GC-root directories after spawn errors and recomputes observation identities in the core.

## Adapter behavior

- `kiln-core` owns deterministic task mapping, terminal classification, and GC-root release decisions.
- `kiln-provider-api` owns the narrow discovery port.
- `kiln-adapter-nix-eval` owns executable access, manifest reads, process execution, stream parsing, and GC-root filesystem effects.
- Bounded Exec clears the environment and owns process-group timeout, cancellation, output limits, and teardown.
- The parser consumes fixed-size chunks under line, byte, record, metadata, diagnostic, dependency, output, and task bounds.
- Complete task proposals require an exact process, stream, source, policy, and GC-root match.
- Per-job errors remain separate from process errors.
- Unknown outcomes reconcile without redispatch.
- Cache status remains advisory.
- GC roots remain until every task has a durable downstream handoff.
- Cleanup errors remain `CleanupFailed` observations.

## Validation

- Pre-change provider, workflow, task, attempt, and standalone tests: pass.
- Workspace tests with all targets and all features: 221 passed.
- Nix evaluation adapter unit tests: 13 passed.
- Deterministic fixture-process tests: 3 passed.
- Strict workspace Clippy with all targets and all features: pass.
- Core `wasm32-unknown-unknown` check: pass.
- Rust formatting: pass.
- Positive Nickel profile and request fixtures: pass.
- Negative Nickel fixtures: 9 rejected as required.
- Exact upstream positive integration fixture: pass.
- Exact upstream per-job error fixture: pass.
- Exact upstream dependency and GC-root fixture: pass.
- Exact upstream cache advisory fixture: pass.
- Negative `builtins.wasm` fixture: reported as unsupported stock-cohort behavior.
- Nix fixture-process check: pass.
- Full `nix flake check path:$PWD -L --option builders ''`: pass with 10 checks.
- The full Nix command omitted incompatible `aarch64-linux` checks.
- Tracey: 446 requirements and 446 referenced requirements.
- Tracey receipt: `b3bd9976627d3ae65139e1b2591ba323594b1d39cb32db95addc0774b5a4edaa`.
- Proposal gate receipt: `73c7a3028fc8a52c307381e5b4586b41ea4c3cefa1c0a3e95a30721c126e686b`.
- Design gate receipt: `a2eaed07c60521325b758ea8e268c13ce8fa10d3b17b73205ba2a1254a17606b`.
- Task gate receipt: `1a9e1f335859d41299bfb8036895de3f977cf2d325a196ed80a4d9caeb42362f`.
- Specification sync receipt: `28f503c94a8cab5a87b3ac7b281f7cfe525f35f0cf7062a72d9f443947e7b9b0`.
- Archive receipt: `ae84077e8da50e671d9c90c00869a195d7f48a9cc0180f793a4efbea51c62c0b`.
- Post-archive Cairn validation and Tracey coverage: pass.
- Release evidence identity: `e13c4712a502475e8d7109485b94808dbe883f3db61acffe58605f198a516da2`.
- Git diff check: pass.

The isolated Git worktree has no colocated Jujutsu workspace. Git supplied the version-control fallback for this change.

Kiln has no root Octet metadata profile. No Octet result is claimed.

## Boundaries and non-claims

Passing evidence does not prove Nix evaluation correctness, source trust, complete dependency semantics, or cache truth.

It does not prove build execution, output correctness, Onix Nix compatibility, reproducibility, or release readiness.

The adapter does not authorize builds, substitutions, merges, releases, or cleanup before durable handoff.
