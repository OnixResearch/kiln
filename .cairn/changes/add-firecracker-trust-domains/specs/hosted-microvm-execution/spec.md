# Hosted Microvm Execution Specification

## ADDED Requirements

### Requirement: Firecracker trust-domain separation

r[kiln.hosted.microvm.trust_domain] Firecracker runners MUST carry an optional
reviewed placement-domain identity. An untrusted-capable runner MUST require a
non-empty domain. A domain MUST NOT contain both untrusted-capable and
workload-bearing runners. The selected domain MUST be recorded in the attempt
receipt. This is static runner-pool separation and MUST NOT claim physical host,
LLC, core or cache isolation.

#### Scenario: Shared domain is rejected

- GIVEN an untrusted-capable runner and a workload-policy runner in one domain
- WHEN execution configuration is admitted
- THEN admission fails before any effect.

#### Scenario: Separated domains are admitted

- GIVEN untrusted and workload-bearing runners in distinct reviewed domains
- WHEN configuration is admitted
- THEN each runner may be used and its domain is recorded in the receipt.

#### Scenario: Missing untrusted domain

- GIVEN an untrusted-capable Firecracker runner without a domain
- WHEN configuration is admitted
- THEN admission fails.
