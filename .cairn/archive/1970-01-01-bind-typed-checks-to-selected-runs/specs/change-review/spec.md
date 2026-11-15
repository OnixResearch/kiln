# Change Review Delta

## MODIFIED Requirements

### Requirement: Typed check evidence

r[kiln.review.checks] CI evidence MUST expose named check results bound to the exact reviewed or integration-candidate commit, with a known check kind, outcome and optional artifact digest. A configured required check MUST be satisfied only by a passing check of the required kind and optional name on the ordinary policy-selected full clean run for the review project and its current applicable scope. The run MUST be nonempty, successful, terminal and eligible to count for required CI. A newer selected run without the check MUST NOT inherit historical evidence. In a monorepo other affected projects MUST retain their independent ordinary CI requirements and MUST NOT satisfy the review project's typed requirements by coincidentally named checks. The checklist MUST identify the actual selected run or explain its absence. Unknown or duplicate requirements MUST fail policy admission. Typed checks MUST NOT grant gate, merge or release authority.

#### Scenario: Required check passes

- GIVEN a policy requiring a property check named property-suite
- WHEN the current eligible selected review-project run retains that exact passing check
- THEN the checks item is satisfied and its run identity agrees with the evidence.

#### Scenario: A newer run lacks the check

- GIVEN an older pass and a newer selected successful run without the required check
- WHEN the checklist is evaluated
- THEN the check remains unmet and the newer run remains selected.

#### Scenario: Ineligible evidence cannot lend a pass

- GIVEN a matching typed pass only in a wrong-project, wrong-scope, dirty, partial, empty-ID, unsuccessful, nonterminal, noncounting-untrusted or wrong-subject fact
- WHEN the checklist is evaluated
- THEN the required typed check remains unmet.

#### Scenario: Multi-project selection and subject drift

- GIVEN selected CI for the review project and another affected project
- WHEN the other project alone has the typed pass, or the integration candidate or current review-project scope changes
- THEN unrelated or stale evidence cannot satisfy the typed requirement.

#### Scenario: Wrong kind, outcome or invalid policy

- GIVEN a failing, skipped, cancelled, unknown or absent check; another kind/name; or an unknown/duplicate/oversized policy requirement
- WHEN admission is evaluated
- THEN evidence remains unmet or policy admission rejects without granting authority.
