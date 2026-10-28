# Operator-owned startup profile. Export to JSON and supply under execution in
# the hosted service profile; customer roles cannot alter executable identities.
# All executable/image paths resolve inside immutable /nix/store and are checked
# against exact BLAKE3 immediately before use. No ambient environment is inherited.
let c = import "github-contracts.ncl" in
let CacheRelative =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 1024
    && std.string.is_match "^[^/\\\\\\x00-\\x1f\\x7f]+(/[^/\\\\\\x00-\\x1f\\x7f]+)*$" value
    && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
  )
in
let Positive = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 1 && value <= maximum && std.number.floor value == value
  )
in
let Identity =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
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
  kind | std.contract.from_predicate (fun value => std.array.elem value ["namespaces", "qemu"]),
  # namespaces: every namespace-specific field is required by Rust admission;
  # qemu: qemu/kernel/initramfs/guest_shell/acceleration are required there.
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
  guest_shell | Absolute | optional,
  acceleration | std.contract.from_predicate (fun value => std.array.elem value ["tcg", "kvm"]) | optional,
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
      env | Immutable,
      manager_runtime_directory | Absolute,
      # Only externally acquired customer snapshots below organization/project may
      # be placed here. Never point this at the platform database or credentials.
      work_root | Absolute,
      runners | { _ : Backend },
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
              organization | String,
              project | String,
              runner | String,
              worker
                | {
                  producer | String,
                  toolchain | String,
                  platform | String,
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
      trusted_projects | Array { organization | String, project | String } | default = [],
      # Every compiled runner must match an operator price policy. Absent pricing
      # denies dispatch; customer-admitted prices alone cannot grant compute.
      pricing
        | Array {
          organization | String,
          runner | String,
          price_id | Identity,
          class | String,
          max_units | Positive 1000000000
        }
        | default
        = [],
      workload
        | Array {
          organization | String,
          project | String,
          runner | String,
          audience | Dyn,
          secrets | Array { environment | String, name | String, revision | Positive 9007199254740991 },
        }
        | default
        = [],
    }
