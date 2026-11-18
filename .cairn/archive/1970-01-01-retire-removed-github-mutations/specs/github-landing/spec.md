# GitHub landing delta

## MODIFIED Requirements

### Requirement: Guarded merge-commit landing

r[kiln.github.landing.merge] Kiln MUST NOT issue an unattended merge. GitHub
offers no qualified guard that makes a merge conditional on the admitted target
head, so `land` MUST refuse before any request. Kiln MAY recover a result that
was landed outside Kiln, without any mutation. Recovery MUST return a receipt
only for a pull request merged at the exact candidate head and base whose merge
commit has the admitted base head as first parent and the candidate as second
parent, whose tree equals the candidate tree, and whose landing evidence and base
read-back still hold. An unobservable merge commit MUST remain unknown. The route
MUST NOT close issues, mutate source status or claim atomic base protection or
merge-queue qualification.

#### Scenario: An external exact merge is recovered

r[kiln.github.landing.merge.scenario.recovered]
- GIVEN a pull request merged outside Kiln at the exact candidate head onto the admitted base head
- WHEN recovery runs
- THEN the receipt MUST bind the candidate head, merge commit, base and base head, and no request MUST mutate GitHub

#### Scenario: Merge does not land

r[kiln.github.landing.merge.scenario.refused]
- GIVEN a refused merge or no observable merged state
- WHEN landing reconciliation runs
- THEN it MUST fail or remain unknown without claiming a landed result
