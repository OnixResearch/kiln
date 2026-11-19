# iddqd-Backed Policy Identity Maps

Basalt treats checked-in generated policy JSON as untrusted input and builds
validated in-memory identity indexes before enforcement. This boundary uses the
`iddqd` crate's `IdOrdMap` so that contract and backend keys are borrowed from
each value's own stable id rather than trusted as duplicated external keys.

The public Nickel-generated policy JSON object shape is unchanged. Rust accepts
the same object-shaped policy, then builds identity indexes that fail closed on
any identity drift before enforcement.

## Identity conversion

`build_policy_identity_indexes` converts a parsed `Policy` into:

- an `IdOrdMap<ContractPolicy>` keyed by `ContractPolicy.id`;
- an `IdOrdMap<ContractBackendPolicy>` keyed by `ContractBackendPolicy.id`.

`IdOrdMap` is preferred over `IdHashMap` because policy artifacts are
externally supplied and deterministic ordering is useful for diagnostics and
stable tests. Conversion uses explicit unique insertion so duplicate ids fail
closed instead of silently overwriting prior entries.

## Validation

Before enforcement, this core rejects:

- contract or backend object keys that do not match the mapped value's id;
- empty or oversized ids;
- duplicate contract or backend ids;
- unsupported backend ids;
- malformed policy input that previously could reach enforcement with ambiguous
  identity.

Violations return a deterministic ordered issue report and refuse to hand back
usable indexes.

## Claim boundary

iddqd provides deterministic ordered identity maps, not UCAN authority, policy
correctness, caveat satisfaction, Steel safety, or release readiness. It does
not change UCAN capability matching, Steel adapter authority, or enforcement
receipt claims.

Non-claims carried by the identity boundary are consistent with the crate's
other policy surfaces: it does not prove policy correctness, UCAN authority,
caveat satisfaction, Steel safety, or release readiness.
