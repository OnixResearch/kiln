## ADDED Requirements

### Requirement: Observed host placement qualification

r[kiln.hosted.microvm.host_placement]

Firecracker admission MUST retain bounded host SMT, KSM, mitigation, vulnerability, kernel and resctrl facts. Untrusted-capable runners MUST reject unknown or vulnerable issue-policy kernel status. Only SMT/KSM safety refusal MAY be explicitly overridden with retained reason. The hosted shell MUST prevent untrusted and workload-bearing overlap on its physical machine, and MUST retain actual managed job overlap rather than planned pipeline peers. Opt-in cooperative guest probes MUST report raw observations and measured errors without claiming hostile isolation or fabricating separate-host qualification.

#### Scenario: Unsafe host refusal

- GIVEN a runner declared untrusted-capable and unknown kernel status
- WHEN admission occurs
- THEN no work is dispatched even with an SMT override.

#### Scenario: Cross-attempt overlap

- GIVEN an active workload-bearing attempt
- WHEN an untrusted attempt reserves the same physical host
- THEN admission refuses; overlapping same-class jobs retain symmetric peer identities.

#### Scenario: Honest measurement

- GIVEN operator-supplied guest launchers and physical placement evidence
- WHEN measurement runs
- THEN the report includes observed bit/error rates and raw samples; absent remote hosts remain an explicit missing prerequisite.
