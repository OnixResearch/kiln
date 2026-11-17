## 1. Retained host effects

- [x] [serial] Persist exact retained operations before existing CH host effects; preserve reconciliation and occupied capacity after crash. r[kiln.hosted.mobility.runtime.scenario.replay]
- [x] [serial] Require observed cleanup for retirement and exact confinement before workload continuation. r[kiln.hosted.mobility.runtime]
- [x] [serial] Bind pause/resume request, in-progress, observation and uncertainty to exact operation/owner; refuse checkpoint/handoff or conflicting commands until native VMM state and confinement are observed. r[kiln.hosted.mobility.runtime.scenario.control]

## 2. Mobility channels

- [x] [serial] Implement authenticated bounded non-replayable Iroh migration stream and durable bounded object transfer. r[kiln.hosted.mobility.runtime.scenario.durable]
- [x] [serial] Add exact compatibility and evidence-based drain decisions; deny unqualified live cutover. r[kiln.hosted.mobility.runtime]

## 3. Evidence

- [x] [serial] Add positive and negative behavioral fixtures and update owned runbooks. r[kiln.hosted.mobility.runtime]
- [ ] [serial] Parent runs coordinated focused tests; operator qualifies two real KVM hosts and actual Aspen/fencing provider before enablement. r[kiln.hosted.mobility.runtime]
- [ ] [serial] Obtain external Aspen placement/resource authority, admitted guest reconnect and cross-host stream providers, and compatible two-host qualification before enabling mobility. r[kiln.hosted.mobility.runtime]
