# Campaign controls delta

## ADDED Requirements

### Requirement: Authenticated campaign controls through the canonical owner

r[kiln.implementation.campaign.controls] A principal MUST operate a selected campaign only through typed bounded intents (start a configured definition, continue, pause, cancel, issue a decision, resolve a decision). Intents MUST NOT carry executables, database paths, credentials, contracts or owner observations. The canonical campaign owner MUST apply each intent and its durable command record in one store transaction bound to an expected version and request ID. The receiving shell MUST authorize every mutation by a current, unexpired grant for that principal that names the campaign and command. Campaign projections, the decision inbox and start previews MUST remain read-only.

#### Scenario: Start a frozen, previewed definition

r[kiln.implementation.campaign.controls.scenario.start]
- GIVEN a configured definition with a frozen contract and an admitted delivery route
- WHEN an authorized principal reads its preview and starts it by identity
- THEN the preview MUST name the exact members, tasks, prerequisite inclusion, workflow profile, policy, repository instructions, completion goals, required checks and reviews, parent budget and delivery route
- AND the start MUST create exactly that selection, while a changed definition or an existing campaign MUST be refused without joining anything to a running campaign

#### Scenario: Offered endpoints follow the delivery route

r[kiln.implementation.campaign.controls.scenario.endpoints]
- GIVEN a definition whose member goal its delivery route does not support, such as a verified pull request on a native no-PR route
- WHEN its preview is read or a start is requested
- THEN the preview MUST mark that goal unsupported and list the goals the route offers, and the start MUST be refused

#### Scenario: Stale or unauthorized commands change nothing

r[kiln.implementation.campaign.controls.scenario.conflict]
- GIVEN a stale expected version, a changed definition or changed decision scope, a missing, narrower or expired grant, or the wrong review audience
- WHEN a command is submitted
- THEN no transition MAY occur; staleness MUST return a refreshable conflict and missing authority MUST be refused before the owner is reached

#### Scenario: Duplicate submissions reconcile one operation

r[kiln.implementation.campaign.controls.scenario.replay]
- GIVEN a command that was applied but whose response was lost
- WHEN the same principal resubmits the same intent with the same request ID, including after a restart
- THEN the owner MUST return the recorded outcome without a second transition, claim or dispatch, and the same request ID with another intent or principal MUST conflict

#### Scenario: Decisions resolve only their own current obligation

r[kiln.implementation.campaign.controls.scenario.decisions]
- GIVEN outstanding, unissued and resolved decisions
- WHEN the inbox is read and answers are submitted
- THEN the inbox MUST show each decision's question, options, recommendation, category, bound scope, scope freshness and queue state
- AND an answer MUST be refused unless it names an outstanding decision, its exact bindings and one of its options while the member's current scope still matches; it MUST NOT resolve another decision
- AND a required review MUST be answered only by a principal holding one of the member's review roles, and resolved decisions MUST remain visible

#### Scenario: Admission controls do not claim termination

r[kiln.implementation.campaign.controls.scenario.admission]
- GIVEN a campaign with retained or running work
- WHEN it is paused, continued or cancelled
- THEN pause MUST stop successor admission and decision issuance, continue MUST reopen it, and cancel MUST close it permanently
- AND no result MAY state or imply observed worker termination, provider cleanup or refunded usage

#### Scenario: Viewers read, workers are excluded, operators are narrowed

r[kiln.implementation.campaign.controls.scenario.roles]
- GIVEN viewer, worker, granted operator and ungranted operator principals
- WHEN each reads the inbox and previews and submits commands, including after a service restart and after a grant is removed from the profile
- THEN viewers MUST read with no open operations and be refused every mutation, workers MUST be refused reads and mutations, and operators MUST act only within their current grants

#### Scenario: The console offers only admitted operations

r[kiln.implementation.campaign.controls.scenario.console]
- GIVEN an inbox and previews from the service
- WHEN the console renders them and the operator submits a command
- THEN controls and answer forms MUST appear only for listed operations and actions, each submission MUST carry the displayed store version and a fresh request ID, a retry after a lost response MUST reuse that request ID, and a conflict MUST report that nothing was applied and reload the state
