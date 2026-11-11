## Decisions

Landing observation is separated from landing authority. The plan carries the
operator's exact expected head/base/check set; the observer returns an
observation only if it matches. Check names are unique and bound to the exact
head, so a passing check from another commit cannot satisfy the plan.

The observer reads one bounded check-run page and refuses when GitHub reports
more than the page. It does not infer absence from truncation. It never follows
redirects or a provider-supplied next URL.

## Non-claims

The observation is not approval, merge authority, integration proof or release
eligibility. It does not prove check correctness beyond GitHub's reported
status and does not mutate remote state.
