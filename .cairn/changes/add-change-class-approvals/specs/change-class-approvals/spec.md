# Change Class Approvals Specification

## Purpose

Defines ordered policy-declared change classes and explicit evidence-only
approval for matching ownership rules. Ordinary review authority is unchanged.

## ADDED Requirements

### Requirement: Bounded class policy

r[kiln.review.class.policy] An ownership policy MAY declare ordered
`classes`. Each class MUST have a unique non-`logic` identity and a nonempty
bounded list of valid ownership path patterns. A rule MAY declare `approval`
keyed by declared class or `logic`; values are `owner_approval` or
`evidence_only`. `evidence_only` MUST be explicit and MUST accompany at least
one evidence requirement. Unknown classes, duplicate identities, the reserved
`logic` identity, excessive classes or invalid patterns MUST be rejected. Empty
class and approval lists MUST serialize as absent.

#### Scenario: Explicit evidence-only class
r[kiln.review.class.policy.scenario.explicit]
- GIVEN a rule with `project_ci` evidence and an `evidence_only` entry for class `docs`
- WHEN the policy is admitted
- THEN it is valid.

#### Scenario: Evidence-only without evidence
r[kiln.review.class.policy.scenario.missing_evidence]
- GIVEN a rule with `evidence_only` and no evidence requirements
- WHEN the policy is admitted
- THEN it MUST be rejected.

### Requirement: Ordered exact classification

r[kiln.review.class.select] A change MUST belong to the first declared class
whose patterns match every current changed path and every renamed old path. A
change with complete facts that matches no class MUST use the reserved `logic`
class. Empty or incomplete changed-file facts MUST NOT select a class; ownership
evaluation fails before classification.

#### Scenario: Rename crosses a class boundary
r[kiln.review.class.select.scenario.rename]
- GIVEN a change that renames `src/old.rs` to `docs/guide.md`
- AND class `docs` matches `docs/**`
- WHEN changes are classified
- THEN the change MUST be `logic`, not `docs`.

### Requirement: Scoped evidence-only authority

r[kiln.review.class.authority] For a matching rule, an explicit `evidence_only`
entry MUST replace only that rule's owner approval requirement. Its evidence
items MUST still gate readiness. The owner item MUST remain and state the
selected class and whether approval is required. The ordinary approval count,
vetoes, blocking threads, dependencies, CI and merge preflight MUST remain
unchanged.

#### Scenario: Evidence-only rule
r[kiln.review.class.authority.scenario.docs]
- GIVEN a matching rule with satisfied evidence and `evidence_only` for class `docs`
- AND no owner vote
- WHEN a `docs` change is evaluated
- THEN the owner item is satisfied but all evidence and ordinary policy items still apply.

#### Scenario: Logic keeps approval
r[kiln.review.class.authority.scenario.logic]
- GIVEN the same rule on a `logic` change
- WHEN the checklist is evaluated
- THEN an eligible owner approval MUST still be required.
