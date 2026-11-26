## 1. External prerequisites

- [ ] [serial] Obtain Aspen exclusive guest placement and resource fencing (`fence-guest-placement-ownership`, Aspen #15). r[kiln.sdlc.guest_move]
- [ ] [serial] Obtain Lattice epoch-scoped guest reconnect (`reconnect-restored-guest-sessions`). r[kiln.sdlc.guest_move.scenario.restored]

## 2. Kiln

- [ ] [serial] Add pure guest-move plan and step transitions, with refusals for an incompatible destination, a stale fence or placement version, and an uncertain step. r[kiln.sdlc.guest_move]
- [ ] [serial] Bind the hosted retained-mobility shell to the ordered steps and Aspen placement grant; retire the source only after an observed fence. r[kiln.sdlc.guest_move.scenario.restored]
- [ ] [serial] Add operator-only Fleet routes, the web bridge, authority registration and console controls showing plan, step and outcome. r[kiln.sdlc.guest_move]

## 3. Verification

- [ ] [serial] Add positive and negative tests for a completed move, a lost reply at each step, a duplicate source, an incompatible destination and a failed restore. r[kiln.sdlc.guest_move.scenario.uncertain]
- [ ] [serial] Qualify on two operator-owned compatible KVM hosts and retain the evidence. r[kiln.sdlc.guest_move.scenario.restored]
