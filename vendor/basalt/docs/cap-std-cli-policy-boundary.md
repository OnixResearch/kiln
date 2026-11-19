# Capability-Bounded CLI Filesystem Boundary

The Basalt CLI opens operator-declared policy and output roots with `cap-std`'
capability directories. Every policy read and receipt/output write is relative
to a typed `Dir` root, so a `../` traversal, absolute path, symlink escape,
missing root, or output escape attempt fails closed before it can touch an
unrelated host path.

## CLI-only dependency

`cap-std` is added only to the `cli` feature. No-default library consumers,
the enforcement core, UCAN validation, Nickel/Steel backend DTOs, and the
packaged policy helpers compile without `cap-std` and never open ambient
filesystem paths directly. The library surface operates on in-memory policy
data, DTOs, or validated relative locators.

## Typed roots

- `PolicyRoot` bounds policy input reads under a declared policy directory.
- `OutputRoot` bounds receipt/output writes under a declared output directory.

Both open the declared ambient path once (to establish the boundary) and then
resolve every later path relative to the capability root.

## Positive and negative coverage

Positive fixtures cover valid policy reads under a declared root, packaged
policy fallback, output writes under a declared root, and the CLI smoke path.
Negative fixtures cover `../` traversal, absolute paths, missing roots, and
output escape attempts. Both the unit module and the `cli_feature` integration
suite exercise the bounded CLI boundary.

## Claim boundary

Capability roots bound local CLI filesystem authority only. They do not prove
policy truth, UCAN issuer trust, caveat semantics, downstream deployment
readiness, release readiness, or whole-system safety.
