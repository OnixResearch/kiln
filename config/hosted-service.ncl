# Host-owned service profile builder. JSON admission and filesystem/tool checks remain
# authoritative in Rust. Optional modules are validated, never discarded.
let c = import "github-contracts.ncl" in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let Peer =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Https =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 2048
    && std.string.is_match "^https://[^ @?#]+$" value
  )
in
let SocketAddress =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^([0-9.]+|\\[[0-9a-fA-F:]+\\]):[0-9]+$" value
  )
in
let AtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= maximum
  )
in
let Executable = { path | c.AbsolutePath, blake3 | c.Blake3 } in
let RuntimeRoot =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["/nix/store", "/usr", "/bin", "/lib", "/lib64"]
  )
in
let Backend =
  std.contract.custom (fun label value =>
    if value.kind == "namespaces" then
      std.contract.check
        {
          kind | std.contract.from_predicate (fun v => v == "namespaces"),
          unshare | Executable,
          python | Executable,
          mount | Executable,
          shell | Executable,
          runtime_roots | Array RuntimeRoot | AtMost 6,
        }
        label
        value
    else if value.kind == "qemu" then
      std.contract.check
        {
          kind | std.contract.from_predicate (fun v => v == "qemu"),
          qemu | Executable,
          kernel | Executable,
          initramfs | Executable,
          root_image | c.Nullable Executable | default = null,
          guest_shell | c.AbsolutePath,
          acceleration | std.contract.from_predicate (fun v => v == "tcg" || v == "kvm"),
          qualification_identity | c.Nullable c.Blake3 | default = null,
        }
        label
        value
    else if value.kind == "firecracker" then
      # One microVM per job from the `microvm-guest` flake output (see its
      # manifest.json for exact paths and BLAKE3). Rust admission checks memory
      # and vCPUs against the unit limits; there is no network or jailer field.
      std.contract.check
        {
          kind | std.contract.from_predicate (fun v => v == "firecracker"),
          firecracker | Executable,
          kernel | Executable,
          initrd | Executable,
          guest_shell | c.AbsolutePath,
          vcpus | c.BoundedInteger 1 8,
          memory_mib | c.BoundedInteger 128 16384,
          boot_timeout_ms | c.BoundedInteger 100 60000,
          source_image_mib | c.BoundedInteger 1 1024,
          warm_pool
            | c.Nullable {
              directory | c.AbsolutePath,
              slots | c.BoundedInteger 1 16,
            }
            | default
            = null,
          qualification_identity | c.Nullable c.Blake3 | default = null,
        }
        label
        value
    else
      'Error { message = "Backend kind must be namespaces, qemu or firecracker" }
  )
in
let CacheRelative =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 1024
    && std.string.is_match "^[^/\\\\\\x00-\\x1f\\x7f]+(/[^/\\\\\\x00-\\x1f\\x7f]+)*$" value
    && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
  )
in
let Cache = {
  directory | c.AbsolutePath,
  maximum_entry_bytes | c.BoundedInteger 1 1048576,
  maximum_total_bytes | c.BoundedInteger 1 1073741824,
  maximum_entries | c.BoundedInteger 1 4096,
  maximum_age_seconds | c.BoundedInteger 1 31536000,
  maximum_files | c.BoundedInteger 1 4096,
  policies
    | Array {
      organization | Identifier,
      project | Identifier,
      runner | Identifier,
      worker
        | {
          producer | c.NonEmpty,
          toolchain | c.NonEmpty,
          platform | c.NonEmpty,
          inputs | Array CacheRelative | AtMost 4096,
          paths | Array CacheRelative | AtMost 4096,
          maximum_entry_bytes | c.BoundedInteger 1 1048576,
          maximum_files | c.BoundedInteger 1 4096,
        },
    }
    | AtMost 128,
}
in
let Execution = {
  systemd_run | Executable,
  systemctl | Executable,
  manager_runtime_directory | c.AbsolutePath,
  cache | c.Nullable Cache | default = null,
  trusted_projects
    | Array {
      organization | Identifier,
      project | Identifier,
    }
    | AtMost 128
    | default
    = [],
  pricing
    | Array {
      organization | Identifier,
      runner | Identifier,
      price_id | c.Blake3,
      class | Identifier,
      max_units | c.BoundedInteger 1 1000000000,
    }
    | AtMost 128
    | default
    = [],
  workload
    | Array {
      organization | Identifier,
      project | Identifier,
      runner | Identifier,
      audience | c.Nullable c.NonEmpty | default = null,
      secrets
        | Array {
          environment
            | std.contract.from_predicate (fun value =>
              std.is_string value && std.string.is_match "^[A-Za-z_][A-Za-z0-9_]*$" value
            ),
          name | Identifier,
          revision | c.Positive,
        }
        | AtMost 32,
    }
    | AtMost 128
    | default
    = [],
  env | Executable,
  work_root | c.AbsolutePath,
  runners | { _ : Backend },
  maximum_concurrent | c.BoundedInteger 1 128,
  memory_bytes | c.BoundedInteger 67108864 68719476736,
  processes | c.BoundedInteger 8 1024,
  cpu_percent | c.BoundedInteger 1 800,
  maximum_timeout_ms | c.BoundedInteger 100 3600000,
  maximum_output_bytes | c.BoundedInteger 1024 1048576,
}
in
let Fleet = {
  driver | Executable,
  minimum | c.BoundedInteger 0 128,
  maximum | c.BoundedInteger 1 128,
  slots_per_instance | c.BoundedInteger 1 256,
  organizations | Array Identifier | AtMost 128,
  pricing
    | c.Nullable {
      price_id | c.Blake3,
      class | Identifier,
      max_units | c.BoundedInteger 1 1000000000,
    }
    | default
    = null,
  timeout_ms | c.BoundedInteger 100 300000,
}
in
# Import module builders as contracts so non-null optional sections retain all
# their nested checks, including SecretSpec references and operations bounds.
let OptionalBuilder = fun builder =>
  std.contract.custom (fun label value =>
    if value == null then 'Ok null else 'Ok (builder value)
  )
in
fun input =>
  let settings =
    input
      | {
        schema
          | std.contract.from_predicate (fun value => value == "kiln.hosted.v1")
          | default
          = "kiln.hosted.v1",
        service_id | Identifier,
        state_directory | c.AbsolutePath,
        network
          | {
            key_file | c.AbsolutePath,
            bind_address | SocketAddress,
            relay_urls | Array Https | AtMost 8,
          },
        bootstrap_peer | Peer,
        bootstrap_organization | Identifier,
        operator_peers | Array Peer | AtMost 16 | default = [],
        source_profiles
          | Array {
            path | c.AbsolutePath,
            blake3 | c.Blake3,
            organization | Identifier,
            project | Identifier,
            network_activation | Bool | default = false,
            price_id | c.Nullable c.Blake3 | default = null,
            class | c.Nullable Identifier | default = null,
            max_units | c.Nullable (c.BoundedInteger 1 1000000000) | default = null,
          }
          | AtMost 128
          | default
          = [],
        operations | OptionalBuilder (import "hosted-operations.ncl") | default = null,
        secrets | OptionalBuilder (import "hosted-secrets.ncl") | default = null,
        execution | c.Nullable Execution | default = null,
        fleet | c.Nullable Fleet | default = null,
      }
  in
  {
    schema = settings.schema,
    service_id = settings.service_id,
    state_directory = settings.state_directory,
    network = settings.network,
    bootstrap_peer = settings.bootstrap_peer,
    bootstrap_organization = settings.bootstrap_organization,
    operator_peers = settings.operator_peers,
    source_profiles = settings.source_profiles,
    operations = settings.operations,
    secrets = settings.secrets,
    execution = settings.execution,
    fleet = settings.fleet,
  }
