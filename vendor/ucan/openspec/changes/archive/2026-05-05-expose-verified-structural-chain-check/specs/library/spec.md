## ADDED Requirements

### Requirement: Verified structural chain bridge [r[library.verified-bridges.structural-chain]]

The library MUST expose a structural chain bridge that accepts caller-supplied resolved UCAN chain entries and routes finite issuer/audience linkage, caller-provided signature-valid flags, exact attenuation narrowing, and temporal validity through the vendored verified chain checker.

#### Scenario: Structural chain bridge accepts a linked exact-attenuation chain [r[library.verified-bridges.structural-chain.accepts-linked-exact-chain]]

- GIVEN two caller-supplied chain entries whose parent audience equals the child issuer
- AND every entry is marked signature-valid by the caller
- AND the child capability narrows the parent capability with exact ability equality
- AND both entries are temporally valid at the supplied verification time
- WHEN the structural chain bridge is evaluated
- THEN it MUST return an allowed result from the verified chain checker.

#### Scenario: Structural chain bridge rejects broken links and attenuation escalation [r[library.verified-bridges.structural-chain.rejects-invalid-structure]]

- GIVEN caller-supplied chain entries with a broken issuer/audience link or exact attenuation escalation
- WHEN the structural chain bridge is evaluated
- THEN it MUST return a rejected result from the verified chain checker without invoking proof resolution, network lookup, revocation registries, caveat policy, or cryptographic signature verification.
