# Hosted Microvm Execution Specification

## ADDED Requirements

### Requirement: Firecracker VMM CPU affinity

r[kiln.hosted.microvm.cpu_affinity] A Firecracker runner MAY declare a reviewed
`cpu_affinity` list. Admission MUST accept only comma-separated CPU numbers and
ranges in `0..=4095`, without duplicates, and MUST require a configured immutable
`taskset` executable when any runner declares affinity. Execution MUST start the
VMM through `taskset -c <list>` and MUST record the list in the attempt receipt.
This pins the VMM process and MUST NOT claim guest-vCPU, cache, LLC or
covert-channel isolation.

#### Scenario: Valid affinity

- GIVEN a Firecracker runner with `cpu_affinity = "0-3,8"` and a configured taskset
- WHEN the VMM starts
- THEN the command is wrapped with `taskset -c 0-3,8` and the receipt records it.

#### Scenario: Invalid affinity

- GIVEN an empty, reversed, duplicate or out-of-range CPU list
- WHEN execution configuration is admitted
- THEN admission fails before any effect.

#### Scenario: Missing taskset

- GIVEN a runner with affinity and no configured taskset
- WHEN configuration is admitted
- THEN admission fails.
