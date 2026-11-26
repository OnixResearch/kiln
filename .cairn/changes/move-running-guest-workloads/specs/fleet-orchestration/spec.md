## ADDED Requirements

### Requirement: Running guest workloads move under exclusive placement

r[kiln.sdlc.guest_move]
Kiln MUST move a running guest attempt only to an operator-selected destination admitted for the guest's recorded compatibility profile. It MUST record each step before its effect. It MUST restore on the destination only after the placement owner grants it, and MUST retire the source only after the source's resources are observed fenced. It MUST fail closed before any effect when the placement owner, reconnect gateway or destination admission is unavailable.

#### Scenario: Guest restored on the selected host

r[kiln.sdlc.guest_move.scenario.restored]
- GIVEN a running guest attempt and an admitted destination
- WHEN the operator confirms the exact move plan
- THEN the guest is paused, checkpointed, transferred and restored on the destination under a new placement grant
- AND the source is retired only after its fence is observed

#### Scenario: Lost reply does not duplicate the guest

r[kiln.sdlc.guest_move.scenario.uncertain]
- GIVEN any step whose reply is lost
- WHEN Kiln reconciles the move
- THEN it reads native placement and guest state
- AND it never repeats the effect or runs two owned guests
