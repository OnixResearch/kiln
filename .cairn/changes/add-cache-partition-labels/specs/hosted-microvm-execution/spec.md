# Hosted Microvm Execution Specification

## ADDED Requirements

### Requirement: Reviewed cache-partition labels

r[kiln.hosted.microvm.cache_partition] A Firecracker runner MAY declare a
reviewed `cache_partition` label. Admission MUST reject a label containing both
untrusted-capable and workload-bearing runners. The receipt MUST record the
label. This is a static placement declaration and MUST NOT claim resctrl
programming, cache allocation, LLC separation or covert-channel measurement.

#### Scenario: Shared partition is rejected

- GIVEN an untrusted-capable runner and a workload-bearing runner in one label
- WHEN execution configuration is admitted
- THEN admission fails before any effect.

#### Scenario: Label is recorded

- GIVEN a runner with a reviewed cache-partition label
- WHEN its receipt is produced
- THEN `vm.cache_partition` names the label.
