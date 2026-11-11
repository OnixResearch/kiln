# Hosted Microvm Execution Specification

## ADDED Requirements

### Requirement: Measured guest kernel driver surface

r[kiln.hosted.microvm.guest_surface] The guest build MUST measure the pinned
kernel image against a reviewed deny list of unused device classes using exact
Kconfig symbols and guest-closure module fragments. It MUST record each class
as `absent`, `builtin` or `loadable_module_present` together with the kernel
version, kernel-configuration BLAKE3, guest nixpkgs revision and closure module
count. An enforced class that is `builtin` or `loadable_module_present` MUST
fail the check. The checked-in inventory MUST match the measured current cohort,
so a kernel or configuration change requires a reviewed refresh. Measured-only
classes MUST be recorded without claiming they are absent.

#### Scenario: Enforced classes are absent

r[kiln.hosted.microvm.guest_surface.scenario.absent]
- GIVEN the pinned guest kernel and module closure
- WHEN the surface check runs
- THEN every enforced class MUST be `absent`
- AND the inventory MUST match the checked-in fixture

#### Scenario: A deny-listed class is present

r[kiln.hosted.microvm.guest_surface.scenario.violation]
- GIVEN a kernel that compiles an enforced class in, or a closure that ships a matching module, or a changed kernel/configuration without a refreshed inventory
- WHEN the surface check runs
- THEN it MUST fail and name the class
- AND no build or runtime result MAY be reinterpreted as a pass

#### Scenario: Live guest sysfs agrees

r[kiln.hosted.microvm.guest_surface.scenario.escape]
- GIVEN a booted guest
- WHEN the escape suite probes the enforced classes' sysfs entries
- THEN the entries MUST be absent
- AND the probe MUST fail closed if any appears
