# Design: Add an Onix Nix evaluator cohort

## Context

The existing adapter has strong boundaries for exact executable identity, immutable source, bounded streams, task mapping, GC roots, and non-claims. Onix support must be another cohort under those boundaries, not an exception to them.

## Success Contract

An explicit Onix profile discovers jobs from one immutable OnixOS fixture that uses `builtins.wasm`. The same request fails as unsupported under the stock profile. Wrong executable, host ABI, plugin, or feature facts fail before evaluation.

## Decisions

### Keep two closed named profiles

The profile vocabulary has separate stock and Onix variants. Each variant binds a complete cohort. No `auto` mode inspects a jobset and selects an evaluator.

A request names its profile. Missing or unsupported profile selection fails before process launch.

### Bind the complete Onix evaluator cohort

The Onix profile binds the `nix-eval-jobs` executable and BLAKE3, linked Onix Nix revision and version, platform, Wasm host ABI identity, required experimental feature set, exact plugin bundle and BLAKE3, cohort manifest, and fixed argument policy.

The manifest records source and license facts. It cannot use a mutable branch, ambient plugin directory, or unmeasured executable.

### Extend pure admission and identity

Request admission validates profile-specific facts and extends the discovery identity with evaluator kind, cohort manifest, host ABI, feature set, and plugin identity.

Task mapping remains unchanged after an observation passes profile and stream admission. Provider identities do not replace Kiln task identities.

### Reuse the existing bounded shell

The shell runs the exact executable through Bounded Exec. It uses a clear environment, fixed home, explicit plugin paths, fixed configuration, bounded streams, deadline, cancellation, teardown, and GC-root ownership.

The shell never resolves `nix`, `nix-eval-jobs`, or plugins through `PATH`. It does not link Nix libraries or load fork types into Kiln core crates.

### Deny fallback and misleading errors

A stock profile seeing `builtins.wasm` returns `UnsupportedEvaluatorCapability`. An Onix profile with a missing feature, wrong host ABI, or stale plugin returns a profile-specific preflight denial.

Kiln does not retry through stock Nix, another fork, another plugin, or a host command. These cases do not become generic job evaluation failures.

### Preserve the stock cohort

Existing stock fixtures, identities, documentation, and default behavior remain unchanged. Onix dependencies are optional and absent from standalone core and stock-profile builds.

### Use a frozen OnixOS fixture

OnixOS publishes one minimal immutable jobset that invokes the actual Nickel plugin through `builtins.wasm` and exposes bounded derivation records. Kiln vendors or fetches it by immutable content identity, not a sibling path.

Negative fixtures cover stock profile selection, missing feature, wrong ABI, changed plugin, partial stream, timeout, cleanup failure, and compatibility overclaim.

## Functional Core and Imperative Shell

- **Core**: profile admission, request identities, stream schema, task mapping, terminal classification, and non-claims.
- **Shell**: executable and plugin observations, process launch, pipes, deadlines, cancellation, GC roots, and safe reports.
- **OnixOS**: owns fixture meaning, plugin behavior, and system evaluation semantics.

## Risks and Controls

- Fork upgrades can drift host ABI. Every cohort member changes together under a new profile identity.
- GPL scope can widen. Exact upstream and fork license records remain in the cohort manifest.
- Passing discovery can be read as build proof. Existing non-claims remain mandatory.

## Non-Claims

Passing evidence does not prove Nix or Nickel correctness, source trust, complete dependencies, cache truth, build success, output correctness, reproducibility, deployment safety, or release eligibility.
