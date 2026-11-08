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
let WebRepository =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 2048
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
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
let Origin =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 2048
    && std.string.is_match "^https://[^/?#@ ]+$" value
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
let NonEmptyAtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value >= 1 && std.array.length value <= maximum
  )
in
let UniqueCacheScopes =
  std.contract.from_predicate (fun policies =>
    let check = fun state policy =>
      let key =
        policy.organization ++ "\u{0}" ++ policy.project ++ "\u{0}" ++ policy.runner
      in
      if std.array.elem key state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [key], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } policies).duplicate
  )
in
let UniquePricing =
  std.contract.from_predicate (fun policies =>
    let check = fun state policy =>
      let key =
        policy.organization
        ++ "\u{0}"
        ++ policy.runner
        ++ "\u{0}"
        ++ policy.price_id
        ++ "\u{0}"
        ++ policy.class
      in
      if std.array.elem key state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [key], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } policies).duplicate
  )
in
let UniqueWorkload =
  std.contract.from_predicate (fun policies =>
    let check = fun state policy =>
      let key =
        policy.organization ++ "\u{0}" ++ policy.project ++ "\u{0}" ++ policy.runner
      in
      if std.array.elem key state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [key], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } policies).duplicate
  )
in
let UniqueSecretEnvironments =
  std.contract.from_predicate (fun secrets =>
    let check = fun state secret =>
      if std.array.elem secret.environment state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [secret.environment], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } secrets).duplicate
  )
in
let NonEmptyRecord =
  std.contract.from_predicate (fun value => std.array.length (std.record.fields value) >= 1)
in
let Audience =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0 && std.string.length value <= 256
  )
in
let ActivationPricing =
  std.contract.from_predicate (fun sources =>
    std.array.all
      (fun source =>
        !source.network_activation
        || (source.price_id != null && source.class != null && source.max_units != null)
      )
      sources
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
    else if value.kind == "unikernel-test" then
      std.contract.check
        {
          kind | std.contract.from_predicate (fun v => v == "unikernel-test"),
          firecracker | Executable,
          kernel | Executable,
          project | std.contract.from_predicate (fun v => v == "unikraft"),
          version | String,
          vcpus | c.BoundedInteger 1 1,
          memory_mib | c.BoundedInteger 32 16384,
          network | std.contract.from_predicate (fun v => v == "none") | optional,
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
          # Opt-in debug hold of failed jobs without secrets, workload tokens or
          # untrusted trust (docs/hosted-ci.md). Omitted means disabled; Rust also
          # requires max_concurrent <= maximum_concurrent.
          debug_hold
            | {
              enabled | Bool,
              grace_seconds | c.BoundedInteger 1 900,
              max_concurrent | c.BoundedInteger 1 16,
            }
            | optional,
          # r[impl kiln.untrusted.execution] Operator declaration that this runner
          # may execute approved untrusted pull-request runs, always without
          # secrets, workload token or cache. Omitted means it may not.
          untrusted_capable | Bool | optional,
          # SMT/KSM-only operator exception. Unfixed/unknown kernel status
          # cannot be overridden. The reason is retained in the receipt.
          host_preflight_override
            | std.contract.from_predicate (fun value =>
              std.is_string value
              && std.string.length value > 0
              && std.string.length value <= 1024
              && !(std.string.is_match "\\x00" value)
            )
            | optional,
          # Reviewed placement pool. Untrusted and workload-bearing runners must
          # not share a domain; untrusted-capable runners require one.
          trust_domain
            | std.contract.from_predicate (fun value =>
              std.is_string value
              && std.string.length value > 0
              && std.string.length value <= 128
              && !(std.string.is_match "\\x00" value)
            )
            | optional,
          # Reviewed physical-host identity. Untrusted runners require one and
          # cannot share it with a workload-bearing runner.
          host_identity
            | std.contract.from_predicate (fun value =>
              std.is_string value
              && std.string.length value > 0
              && std.string.length value <= 128
              && !(std.string.is_match "\\x00" value)
            )
            | optional,
          # Existing operator-created resctrl CAT group; Rust verifies assignment.
          cache_partition
            | std.contract.from_predicate (fun value =>
              std.is_string value
              && std.string.length value <= 128
              && std.string.is_match "^[A-Za-z0-9][A-Za-z0-9_-]*$" value
            )
            | optional,
          # Pin VMM and inherited vCPU threads; physical host exclusion still applies.
          cpu_affinity
            | std.contract.from_predicate (fun value =>
              std.is_string value
              && std.string.length value > 0
              && std.string.length value <= 256
              && std.string.is_match "^[0-9,-]+$" value
            )
            | optional,
          qualification_identity | c.Nullable c.Blake3 | default = null,
        }
        label
        value
    else
      'Error { message = "Backend kind must be namespaces, qemu, firecracker, cloud-hypervisor or unikernel-test" }
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
          inputs | Array CacheRelative | NonEmptyAtMost 4096,
          paths | Array CacheRelative | NonEmptyAtMost 4096,
          maximum_entry_bytes | c.BoundedInteger 1 1048576,
          maximum_files | c.BoundedInteger 1 4096,
        },
    }
    | NonEmptyAtMost 128
    | UniqueCacheScopes,
}
in
let Execution =
  std.contract.custom (fun label value =>
    let result =
      std.contract.check
        {
          systemd_run | Executable,
          systemctl | Executable,
          taskset | Executable | optional,
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
            | UniquePricing
            | default
            = [],
          workload
            | Array {
              organization | Identifier,
              project | Identifier,
              runner | Identifier,
              audience | c.Nullable Audience | default = null,
              secrets
                | Array {
                  environment
                    | std.contract.from_predicate (fun value =>
                      std.is_string value && std.string.is_match "^[A-Za-z_][A-Za-z0-9_]*$" value
                    ),
                  name | Identifier,
                  revision | c.Positive,
                }
                | AtMost 32
                | UniqueSecretEnvironments,
            }
            | AtMost 128
            | UniqueWorkload
            | default
            = [],
          env | Executable,
          work_root | c.AbsolutePath,
          runners | { _ : Backend } | NonEmptyRecord,
          maximum_concurrent | c.BoundedInteger 1 128,
          memory_bytes | c.BoundedInteger 67108864 68719476736,
          processes | c.BoundedInteger 8 1024,
          cpu_percent | c.BoundedInteger 1 800,
          maximum_timeout_ms | c.BoundedInteger 100 3600000,
          maximum_output_bytes | c.BoundedInteger 1024 1048576,
        }
        label
        value
    in
    result
    |> match {
      'Ok admitted =>
        let runners = std.record.fields admitted.runners in
        if std.array.all (fun policy => std.array.elem policy.runner runners) admitted.pricing
        && std.array.all (fun policy => std.array.elem policy.runner runners) admitted.workload
        && (
          admitted.cache == null
          || std.array.all
            (fun policy => std.array.elem policy.runner runners)
            admitted.cache.policies
        )
        && std.string.is_match "^/run/user/[0-9]+$" admitted.manager_runtime_directory
        && admitted.work_root != "/"
        && !(std.string.is_match "^/nix/store(/|$)" admitted.work_root)
        && (
          !std.array.any
            (fun runner => std.record.has_field "cpu_affinity" runner)
            (std.record.values admitted.runners)
          || std.record.has_field "taskset" admitted
        )
        && std.array.all
          (fun runner =>
            !(
              runner.kind == "firecracker"
              && std.record.has_field "untrusted_capable" runner
              && runner.untrusted_capable
            )
            || (
              std.record.has_field "trust_domain" runner
              && std.record.has_field "host_identity" runner
            )
          )
          (std.record.values admitted.runners) then
          'Ok admitted
        else
          'Error { message = "execution policy references an unknown runner or invalid root" },
      'Error error => 'Error error,
    }
  )
in
let Fleet =
  std.contract.custom (fun label value =>
    let result =
      std.contract.check
        {
          driver | Executable,
          minimum | c.BoundedInteger 0 128,
          maximum | c.BoundedInteger 1 128,
          slots_per_instance | c.BoundedInteger 1 256,
          organizations | Array Identifier | NonEmptyAtMost 128,
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
        label
        value
    in
    result
    |> match {
      'Ok admitted =>
        if admitted.minimum <= admitted.maximum then
          'Ok admitted
        else
          'Error { message = "fleet minimum must not exceed maximum" },
      'Error error => 'Error error,
    }
  )
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
          | ActivationPricing
          | default
          = [],
        lattice_execution
          | Array {
            id | Identifier,
            peer | Peer,
            workflow_policy_id | Identifier,
            node_id | Identifier,
            operation_identity | c.Blake3,
            web_service
              | {
                service_origin | Origin,
                tls_ca_path | c.AbsolutePath,
                token_file | c.AbsolutePath,
                http_timeout_ms | c.BoundedInteger 1 300000,
                maximum_response_bytes | c.BoundedInteger 1 1048576,
              },
            organization | Identifier,
            web_project | Identifier,
            web_repository | WebRepository,
            repository_path | c.AbsolutePath,
            source_limits
              | {
                max_entries | c.BoundedInteger 1 100000,
                max_path_bytes | c.BoundedInteger 1 4096,
                max_file_bytes | c.BoundedInteger 1 67108864,
                max_total_bytes | c.BoundedInteger 1 268435456,
              },
            hosted_project | Identifier,
            pipeline_id | c.Blake3,
            price_id | c.Blake3,
            class | Identifier,
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
  & (if std.array.length settings.lattice_execution == 0 then {} else { lattice_execution = settings.lattice_execution })
