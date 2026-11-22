# Independent Verification Specification

## Purpose

Defines the `independent-verification` capability.

## Requirements

### Requirement: Independently executed acceptance profile

r[kiln.implementation.verification] Kiln MUST execute an immutable admitted verification profile outside worker authority, independently resolve repository/base/candidate/tree and complete changed paths, retain actual command and artifact identities, cover every acceptance case, and use the same frozen bug reproducer on baseline and candidate. Nonpassing or unknown observations MUST NOT authorize completion. Cairn MUST reuse its authoritative completion evaluator and canonical CI; no-change MUST require explicit policy and independent passing evidence.

#### Scenario: Exact independent evidence passes

r[kiln.implementation.verification.scenario.independent]
- GIVEN exact admitted work and independently resolved candidate facts
- WHEN all frozen checks execute and required artifacts match
- THEN canonical typed checks MUST bind that exact candidate

#### Scenario: Worker success cannot substitute

r[kiln.implementation.verification.scenario.forged]
- GIVEN forged worker success, missing evidence, stale facts or changed protected requirements
- WHEN independent verification runs
- THEN completion MUST remain unmet

### Requirement: Installed canonical verification graph constructor

r[kiln.implementation.verification_graph_cli] Kiln MUST install a command that constructs the dedicated canonical CI service profile from a complete frozen verification profile into an explicitly empty template project, deriving every check, kind, prerequisite and operation identity through the native codec and validating with the runtime graph validator and service admission. It MUST refuse an existing graph, a missing project, an inconsistent profile, a reserved check name, an unsupported check kind and an existing output file, and MUST emit configuration only.

#### Scenario: Dedicated graph is constructed

r[kiln.implementation.verification_graph_cli.scenario.constructed]
- GIVEN an empty-graph template and a frozen verification profile
- WHEN the installed command runs
- THEN it MUST write a new admitted service profile whose project graph equals the frozen checks and prerequisites and report that no check or effect was executed

#### Scenario: Replacement and overwrite are refused

r[kiln.implementation.verification_graph_cli.scenario.refused]
- GIVEN a template whose project already has a graph, an existing output, a drifted or reserved-name profile or an unsupported check kind
- WHEN the command runs
- THEN it MUST fail without writing a service profile
