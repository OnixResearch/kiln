## Decisions

Use explicit immutable executable paths and exact source/policy/toolchain inputs. Local scripted admission fixtures and actual OMP worker execution remain distinct cohorts. Every stage records argv, return status and raw stdout/stderr; missing prerequisites fail closed before attempting the affected stage. Independent checks use the existing trusted verifier, never harness-generated success evidence. Remote effects are excluded from local execution and require exact point-of-risk operator approval.

## Verification

Parent invokes the smallest prerequisite-only command first, then explicitly selected local cases after all source changes land. A live campaign additionally requires real Lattice/OMP provider configuration, separate-UID isolation authority, and approved provider targets. Frozen bug baseline must fail and exact candidate pass; bounded feature and adversarial cases have explicit outcomes.

## Risks and Non-claims

No harness receipt itself proves candidate acceptance, descendant termination or remote integration. Preserve raw native observations. Unsupported providers and unavailable authority remain missing/unqualified, never successful skipped steps. A verified PR remains distinct from integration and source reconciliation.
