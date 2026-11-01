# Agent Vote Policy Specification

## Purpose

Defines explicit agent principals and how their review votes count. Agent
identity is a trusted shell fact; no content or authorship inference is added.

## Requirements

### Requirement: Explicit bounded agent principals

r[kiln.review.agent.profile] The code-review profile MAY declare exact
`agent_principals`. Entries MUST be unique admitted write principals, bounded by
the existing principal limit; empty, duplicate or non-writer entries MUST be
rejected before service start. `policy.agent_votes` MUST be `advisory`
(default) or `binding`, and absence MUST serialize as the default.

#### Scenario: Non-writer agent is rejected
r[kiln.review.agent.profile.scenario.unknown]
- GIVEN an agent principal absent from the admitted write principals
- WHEN the review profile is admitted
- THEN it MUST be rejected.

### Requirement: Advisory agent approvals never satisfy intent gates

r[kiln.review.agent.votes] Under advisory mode, an eligible agent `Approve`
MUST be retained and displayed but MUST NOT satisfy the ordinary approval count
or any ownership rule owner item. Under explicit binding mode, agent approvals
MUST count exactly like human approvals. Agent `NeedsRevision` and `Veto` votes
MUST keep their existing blocking effect in either mode. No mode grants merge
authority.

#### Scenario: Advisory approval is inert
r[kiln.review.agent.votes.scenario.advisory]
- GIVEN an eligible agent Approve and advisory policy
- WHEN the checklist is evaluated
- THEN approvals and ownership MUST remain unmet.

#### Scenario: Binding approval counts
r[kiln.review.agent.votes.scenario.binding]
- GIVEN the same vote and explicit binding policy
- WHEN the checklist is evaluated
- THEN ordinary approvals and ownership MAY be satisfied by that vote.

The review detail MUST mark votes and reviewers that are declared agent
principals, include declared reviewer harness/model metadata when present,
report the effective `agent_votes` mode, and state whether those votes are
advisory or binding under the current policy.

#### Scenario: Agent veto still blocks
r[kiln.review.agent.votes.scenario.veto]
- GIVEN an advisory agent NeedsRevision or Veto
- WHEN the checklist is evaluated
- THEN blocking-vote checks MUST fail as for a human vote.
