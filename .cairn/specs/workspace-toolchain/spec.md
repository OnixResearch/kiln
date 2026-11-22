# Workspace Toolchain Specification

## Purpose

Defines the `workspace-toolchain` capability.

## Requirements

### Requirement: Tested workspace compiler pin

r[kiln.toolchain.pin] The root `rust-toolchain.toml` MUST select Rust 1.97.1 with the `clippy` and `rustfmt` components and the `wasm32-unknown-unknown` target. The workspace `rust-version` MUST name the same tested release, and every flake toolchain for the root workspace MUST derive from that file.

#### Scenario: Development shell

r[kiln.toolchain.pin.scenario.shell]
- GIVEN the root flake development shell
- WHEN a maintainer runs `rustc --version`
- THEN it reports Rust 1.97.1

#### Scenario: Older compiler

r[kiln.toolchain.pin.scenario.older]
- GIVEN a compiler older than the declared `rust-version`
- WHEN Cargo builds a root workspace package
- THEN Cargo refuses the build instead of claiming untested support

### Requirement: Clean gates under the pinned compiler

r[kiln.toolchain.gates] Under the pinned compiler the root workspace MUST pass rustfmt checks, all-target Clippy with warnings denied, the workspace tests and the no-std wasm32 checks without changing behavior. Lint suppressions MUST state why the lint is wrong for that code.

#### Scenario: New lint finding

r[kiln.toolchain.gates.scenario.lint]
- GIVEN a lint the new Clippy release adds or extends
- WHEN it flags workspace code
- THEN the code is corrected, or the suppression names why the lint does not apply
