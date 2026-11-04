## Context

`IsolationPolicy` currently admits only `/nix/store`, `/usr`, `/bin`, `/lib`,
and `/lib64` as read-only runtime mounts. The writable checkout is bound
separately. A Pijul repository for a Patchgate review is neither a runtime root
nor part of the Git checkout.

## Decisions

### Explicit data roots and narrow admission

`read_only_data_mounts` is a separate list, default-empty for byte-compatible
existing profiles. At most four entries are admitted. Lexical admission allows
only `/srv` and `/var/lib/kiln` and their descendants, with no `.` or `..`
components.

### Execution-time revalidation

Before constructing bubblewrap arguments, each mount is canonicalized. A
canonical path that differs from the declared path is rejected, so a reviewed
path cannot silently resolve through a symlink to `/etc`, `/home`, a credential
file, or another authority. The path must be a directory. A mount inside the
writable checkout is rejected because the later checkout bind could override
the read-only mount.

Data mounts are bound with `--ro-bind` before the observer and checkout binds.
They do not add network, environment, capability, or write authority. The worker
still refuses account-authority fallback when the selected sandbox is
unavailable.

### Patchgate fixture

The Patchgate worker fixture uses:

- target `/srv/kiln/patchgate-review-target`;
- data mount `/srv/kiln/patchgate-review-target`;
- writable output in the checkout;
- writable review home under `/tmp` tmpfs;
- `PATH` pointing at an exact Nix-store `b3sum` directory.

## Risks

- The allowed roots are intentionally narrow. Repositories under other host
  paths require a separate reviewed policy change rather than an implicit
  fallback.
- A reviewed operator can still expose data under `/srv` or `/var/lib/kiln` to
  the sandbox. Isolation is not a claim that arbitrary hostile code is confined
  or that mounted data is non-sensitive.
- Path canonicalization rejects symlinked mounts; operators must configure the
  canonical repository path.
