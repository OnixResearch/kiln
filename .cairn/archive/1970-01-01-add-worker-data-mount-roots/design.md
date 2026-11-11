## Context

The existing data mount policy has two layers: a bounded set of mount paths and
a fixed set of roots those paths must be under. The fixed roots are safe but
prevent repositories outside `/srv` and `/var/lib/kiln` from being used.

## Decisions

### Optional reviewed roots

`IsolationPolicy.read_only_data_mount_roots` is optional. Omitted preserves the
current default roots and serialized profile behavior. An explicit list is
validated at admission:

- one to four entries;
- absolute, no `.` or `..`, not `/`;
- unique;
- within `/srv`, `/home`, `/mnt`, `/opt`, or `/var/lib/kiln`.

The individual `read_only_data_mounts` must still lie under one of the selected
roots and pass canonical directory, symlink, and checkout-overlap checks.

### Preserved security boundary

Roots extend where a reviewed mount may live, not what a mount may do. Mounts
remain read-only, carry no environment or credential authority, and unavailable
isolation still fails closed.

## Risks

- `/home` and `/opt` can contain operator credentials or unrelated data. A
  reviewed profile explicitly exposing them may make those bytes readable to
  the sandbox. Isolation is not a claim that mounted data is non-sensitive.
- The root families are still a deny-by-default policy; arbitrary paths need a
  further reviewed change.
