# Operator-owned startup profile. Export to JSON and supply under execution in
# the hosted service profile; customer roles cannot alter executable identities.
# All executable/image paths resolve inside immutable /nix/store and are checked
# against exact BLAKE3 immediately before use. No ambient environment is inherited.
let c = import "github-contracts.ncl" in
let CpuAffinity = (import "field-contracts.ncl").CpuAffinity in
let GuestShell = (import "field-contracts.ncl").GuestShell in
let CacheRelative = (import "field-contracts.ncl").CacheRelative in
let Positive = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 1 && value <= maximum && std.number.floor value == value
  )
in
let RecordAtMost = (import "field-contracts.ncl").RecordAtMost in
let Identity =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Identifier = (import "field-contracts.ncl").Identifier in
let Environment = (import "field-contracts.ncl").Environment in
let CacheText = (import "field-contracts.ncl").CacheText in
let Audience = (import "field-contracts.ncl").Audience in
let Absolute =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/[^\\x00-\\x1f\\x7f]+$" value
    && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
  )
in
let Immutable = {
  path
    | std.contract.from_predicate (fun value =>
      std.is_string value && std.string.is_match "^/nix/store/[^\\x00-\\x1f\\x7f]+$" value
    ),
  blake3 | Identity,
}
in
let Backend = {
  kind | std.contract.from_predicate (fun value => std.array.elem value ["namespaces", "qemu", "firecracker", "cloud-hypervisor", "unikernel-test"]),
  # namespaces: every namespace-specific field is required by Rust admission;
  # qemu: qemu/kernel/initramfs/guest_shell/acceleration are required there;
  # firecracker: firecracker/kernel/initrd/guest_shell/vcpus/memory_mib/
  # boot_timeout_ms/source_image_mib are required, warm_pool is optional.
  # Unknown or cross-backend fields are rejected, not ignored.
  unshare | Immutable | optional,
  python | Immutable | optional,
  mount | Immutable | optional,
  shell | Immutable | optional,
  runtime_roots | Array Absolute | optional,
  qemu | Immutable | optional,
  kernel | Immutable | optional,
  initramfs | Immutable | optional,
  root_image | Dyn | optional,
  guest_shell | GuestShell | optional,
  acceleration | std.contract.from_predicate (fun value => std.array.elem value ["tcg", "kvm"]) | optional,
  firecracker | Immutable | optional,
  cloud_hypervisor | Immutable | optional,
  # Trusted cold-only Unikraft test runner; kernel and Firecracker are immutable.
  project | std.contract.from_predicate (fun value => value == "unikraft") | optional,
  initrd | Immutable | optional,
  vcpus | Positive 1 | optional,
  memory_mib
    | std.contract.from_predicate (fun value =>
      std.is_number value && value >= 32 && value <= 16384 && std.number.floor value == value
    )
    | optional,
  boot_timeout_ms | Positive 60000 | optional,
  source_image_mib | Positive 1024 | optional,
  # Snapshot/restore warm pool: a private 0700 directory outside work_root and
  # at most 16 slots, never more than maximum_concurrent (checked in Rust).
  warm_pool | Dyn | optional,
  # Opt-in debug hold of failed jobs: grace at most 900 s, at most 16 held VMs
  # and never more than maximum_concurrent (checked in Rust). Omit to disable.
  debug_hold
    | {
      enabled | Bool,
      grace_seconds | Positive 900,
      max_concurrent | Positive 16,
    }
    | optional,
  # Recorded SMT/KSM-only exception; never overrides unfixed/unknown kernel status.
  host_preflight_override
    | std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 1024
      && !(std.string.is_match "\\x00" value)
    )
    | optional,
  # Reviewed placement pool; untrusted and workload-bearing runners must differ.
  trust_domain
    | std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 128
      && !(std.string.is_match "\\x00" value)
    )
    | optional,
  # Reviewed physical host; untrusted and workload-bearing runners must differ.
  host_identity
    | std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 128
      && !(std.string.is_match "\\x00" value)
    )
    | optional,
  # Existing operator-created resctrl CAT group; assignment is checked before dispatch.
  cache_partition
    | std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value <= 128
      && std.string.is_match "^[A-Za-z0-9][A-Za-z0-9_-]*$" value
    )
    | optional,
  # Reviewed CPU list for this runner's VMM, e.g. "0-3,8".
  network | std.contract.from_predicate (fun value => value == "none") | optional,
  # Reviewed VMM version; recorded in receipts and snapshot identity v2.
  version
    | std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 128
      && !(std.string.is_match "\x00" value)
    )
    | optional,
  cpu_affinity | CpuAffinity | optional,
  # Omit/null until independent adversarial VM qualification actually exists.
  # Immutable images and a running VM alone are not hostile qualification.
  qualification_identity | Dyn | optional,
}
in
fun input =>
  input
    | {
      systemd_run | Immutable,
      systemctl | Immutable,
      taskset | Immutable | optional,
      env | Immutable,
      manager_runtime_directory | Absolute,
      # Only externally acquired customer snapshots below organization/project may
      # be placed here. Never point this at the platform database or credentials.
      work_root | Absolute,
      runners
        | { _ : Backend }
        | RecordAtMost 32
        | std.contract.from_predicate (fun runners =>
          std.array.all
            (fun label =>
              std.string.length label <= 128
              && std.string.is_match "^[A-Za-z0-9_.-]+$" label
            )
            (std.record.fields runners)
        ),
      maximum_concurrent | Positive 128,
      memory_bytes | Positive 68719476736,
      processes | Positive 1024,
      cpu_percent | Positive 800,
      maximum_timeout_ms | Positive 3600000,
      maximum_output_bytes | Positive 1048576,
      # Cache outputs are expendable products, never source or credentials.
      # The private root must not overlap work_root. Secret/audience jobs always
      # disable restore/publication; disabled timing is null rather than zero.
      cache
        | c.Nullable {
          directory | Absolute,
          maximum_entry_bytes | Positive 1048576,
          maximum_total_bytes | Positive 1073741824,
          maximum_entries | Positive 4096,
          maximum_age_seconds | Positive 31536000,
          maximum_files | Positive 4096,
          policies
            | Array {
              organization | Identifier,
              project | Identifier,
              runner | Identifier,
              worker
                | {
                  producer | CacheText,
                  toolchain | CacheText,
                  platform | CacheText,
                  inputs | Array CacheRelative,
                  paths | Array CacheRelative,
                  maximum_entry_bytes | Positive 1048576,
                  maximum_files | Positive 4096,
                },
            },
        }
        | default
        = null,
      # Customer trust strings grant no authority. This exact operator allowlist is
      # mandatory even for trusted namespace execution and unqualified VM smoke.
      trusted_projects | Array { organization | Identifier, project | Identifier } | default = [],
      # Every compiled runner must match an operator price policy. Absent pricing
      # denies dispatch; customer-admitted prices alone cannot grant compute.
      pricing
        | Array {
          organization | Identifier,
          runner | Identifier,
          price_id | Identity,
          class | Identifier,
          max_units | Positive 1000000000
        }
        | default
        = [],
      workload
        | Array {
          organization | Identifier,
          project | Identifier,
          runner | Identifier,
          audience | c.Nullable Audience | default = null,
          secrets | Array { environment | Environment, name | Identifier, revision | Positive 9007199254740991 },
        }
        | default
        = [],
    }
