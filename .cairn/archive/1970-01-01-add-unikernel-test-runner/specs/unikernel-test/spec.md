# Unikernel test execution

## MODIFIED Requirements

### Requirement: Single native test executable

r[kiln.hosted.unikernel.test] A unikernel test job MUST execute exactly one declared static x86_64 Linux ELF with fixed argv and read-only fixtures. It MUST refuse shell, multiple steps, exec, subprocess harness and network requests before effects. Firecracker MUST declare and observe no network device. Native guest termination MUST emit exactly one bounded KILN_RESULT frame; absent, malformed or duplicate status MUST fail. Receipts MUST retain native project/version/image BLAKE3, firecracker backend, guest_kind unikernel and hostile_isolation_claim false. Warm pools, debug hold and writable output collection MUST NOT be available. Repeated flake images MUST be byte-identical. Native static Rust and Go pass/fail, denied sockets/connect/vsock and single-host cold boot MUST be measured before qualification.

Native Firecracker MUST inherit the reviewed explicit default-deny syscall
policy before executable startup. Before InstanceStart, the actual unit's exact
allowlist and errno MUST match the reviewed policy, and the main and worker
threads MUST satisfy strict Seccomp 2 observation. Receipts MUST bind the policy,
authored/exported BLAKE3 identities and observed manager cohort. A refusal MUST
stop the owning unit before launcher capture teardown; the application MUST NOT
start under a missing or changed policy.

#### Scenario: Changed preboot confinement policy

- GIVEN an admitted native job and an actually missing or changed unit allowlist or errno
- WHEN the prestart policy is observed
- THEN dispatch MUST fail before InstanceStart and cleanup MUST stop the owning unit before joining capture workers.

#### Scenario: Read-only fixture reopened by the application

- GIVEN a declared read-only fixture with verified input bytes
- WHEN the native application reads the fixture and reopens it for writing
- THEN reading MUST succeed, writing MUST be refused, and subsequent reads MUST retain the declared bytes even after vnode recreation.

#### Scenario: Unrecoverable termination

- GIVEN a native test guest
- WHEN no unique complete exit frame is observed
- THEN the job MUST fail rather than infer success from VMM termination.

### Requirement: Fixed guest kernel input

r[kiln.hosted.guest.kernel.fixed] The Linux guest image MUST use a separately pinned guest nixpkgs source and MUST refuse a kernel below the issue-required fixed-series thresholds. Its manifest MUST record kernel_version. This gate MUST NOT be described as independent CVE qualification.

#### Scenario: Old guest kernel

- GIVEN a guest kernel below its fixed series threshold
- WHEN the image is evaluated
- THEN evaluation MUST fail before building the guest.
