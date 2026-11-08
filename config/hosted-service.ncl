# Host-owned service profile builder. JSON admission and filesystem/tool checks remain
# authoritative in Rust. Optional modules are validated, never discarded.
let c = import "github-contracts.ncl" in
let Identifier = (import "field-contracts.ncl").Identifier in
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
let HttpsAuthority = fun value =>
  let authority =
    std.array.first (std.string.split "/" (std.array.last (std.string.split "://" value)))
  in
  let bracketed = std.string.is_match "^\\[" authority in
  let valid_brackets =
    !bracketed || std.string.is_match "^\\[[0-9a-fA-F:.]*:[0-9a-fA-F:.]*\\](:[0-9]+)?$" authority
  in
  let has_port =
    if bracketed then
      std.string.is_match "\\]:" authority
    else
      std.string.is_match ":" authority
  in
  let valid_colons =
    bracketed || std.array.length (std.string.split ":" authority) <= 2
  in
  let host = std.array.first (std.string.split ":" authority) in
  let port = std.array.last (std.string.split ":" authority) in
  valid_brackets
  && valid_colons
  && std.string.length host > 0
  && (
    !has_port
    || (
      std.string.is_match "^[0-9]+$" port
      && (
        let number = std.string.to_number port in
        std.is_number number && number >= 0 && number <= 65535
      )
    )
  )
in
let Https =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 2048
    && std.string.is_match "^https://[^/ @?#][^ @?#]*$" value
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
    && HttpsAuthority value
  )
in
let Origin =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 2048
    && std.string.is_match "^https://[^/?#@ ]+$" value
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
    && HttpsAuthority value
  )
in
let SocketAddress =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^([0-9.]+|\\[[0-9a-fA-F:.]*:[0-9a-fA-F:.]*\\]):[0-9]+$" value
    && (
      let parts = std.string.split ":" value in
      let host = std.array.first parts in
      let port = std.string.to_number (std.array.last parts) in
      let octets = std.string.split "." host in
      let ipv4 =
        std.string.is_match "^\\[" host
        || (
          std.array.length octets == 4
          && std.array.all
            (fun octet =>
              std.string.length octet > 0
              && (octet == "0" || !(std.string.is_match "^0" octet))
              && (
                let number = std.string.to_number octet in
                std.is_number number
                && number >= 0
                && number <= 255
                && std.number.floor number == number
              )
            )
            octets
        )
      in
      ipv4
      && std.is_number port
      && port >= 0
      && port <= 65535
      && std.number.floor port == port
    )
  )
in
let GuestShell = (import "field-contracts.ncl").GuestShell in
let AtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= maximum
  )
in
let RecordAtMost = (import "field-contracts.ncl").RecordAtMost in
let helpers = import "field-contracts.ncl" in
let ExactFields = helpers.ExactFields in
let CpuAffinity = helpers.CpuAffinity in
let ExactSourceProfiles =
  std.contract.from_predicate (fun rows =>
    std.array.all
      (
        ExactFields ["path", "blake3", "organization", "project", "network_activation", "price_id", "class", "max_units"]
      )
      rows
  )
in
let ExactLattice =
  std.contract.from_predicate (fun rows =>
    std.array.all
      (fun row =>
        ExactFields
          ["id", "peer", "workflow_policy_id", "node_id", "operation_identity", "web_service", "organization", "web_project", "web_repository", "repository_path", "source_limits", "hosted_project", "pipeline_id", "price_id", "class"]
          row
        && ExactFields
          ["service_origin", "tls_ca_path", "token_file", "http_timeout_ms", "maximum_response_bytes"]
          row.web_service
        && ExactFields
          ["max_entries", "max_path_bytes", "max_file_bytes", "max_total_bytes"]
          row.source_limits
      )
      rows
  )
in
let ExactRows = fun allowed =>
  std.contract.from_predicate (fun rows =>
    std.array.all (ExactFields allowed) rows
  )
in
let ExactTrusted = ExactRows ["organization", "project"] in
let ExactPricing = ExactRows ["organization", "runner", "price_id", "class", "max_units"] in
let ExactWorkload = ExactRows ["organization", "project", "runner", "audience", "secrets"] in
let ExactSecrets = ExactRows ["environment", "name", "revision"] in
let ExactPolicies = ExactRows ["organization", "project", "runner", "worker"] in
let ExactWorker = std.contract.from_predicate (ExactFields ["producer", "toolchain", "platform", "inputs", "paths", "maximum_entry_bytes", "maximum_files"]) in
let Version =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.trim value != ""
    && std.string.length value > 0
    && std.string.length value <= 128
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
  )
in
let CacheText = (import "field-contracts.ncl").CacheText in
let NonEmptyAtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value >= 1 && std.array.length value <= maximum
  )
in
let UniqueLatticeIds =
  std.contract.from_predicate (fun policies =>
    let check = fun state policy =>
      if std.array.elem policy.id state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [policy.id], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } policies).duplicate
  )
in
let PathUnder = fun root path =>
  let split = fun value => std.string.split "/" value in
  let root_parts = split root in
  let path_parts = split path in
  std.array.length path_parts >= std.array.length root_parts
  && std.string.join "/" (std.array.slice 0 (std.array.length root_parts) path_parts) == root
in
let NonOverlappingPaths =
  std.contract.from_predicate (fun values =>
    let check = fun state value =>
      if std.array.any
        (fun prior => PathUnder prior value || PathUnder value prior)
        state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [value], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
  )
in
let CacheIo =
  std.contract.from_predicate (fun policies =>
    std.array.all
      (fun policy =>
        std.array.all
          (fun input =>
            std.array.all
              (fun output => !PathUnder input output && !PathUnder output input)
              policy.worker.paths
          )
          policy.worker.inputs
      )
      policies
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
let Audience = (import "field-contracts.ncl").Audience in
let Environment = (import "field-contracts.ncl").Environment in
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
    let allowed =
      if value.kind == "namespaces" then
        ["kind", "unshare", "python", "mount", "shell", "runtime_roots", "network"]
      else if value.kind == "qemu" then
        ["kind", "qemu", "kernel", "initramfs", "root_image", "guest_shell", "acceleration", "qualification_identity", "network"]
      else if value.kind == "unikernel-test" then
        ["kind", "firecracker", "kernel", "project", "version", "vcpus", "memory_mib", "network"]
      else if value.kind == "firecracker" then
        ["kind", "firecracker", "kernel", "initrd", "guest_shell", "vcpus", "memory_mib", "boot_timeout_ms", "source_image_mib", "warm_pool", "debug_hold", "untrusted_capable", "version", "host_preflight_override", "trust_domain", "host_identity", "cache_partition", "cpu_affinity", "qualification_identity", "network"]
      else if value.kind == "cloud-hypervisor" then
        ["kind", "cloud_hypervisor", "kernel", "initrd", "guest_shell", "vcpus", "memory_mib", "boot_timeout_ms", "source_image_mib", "warm_pool", "qualification_identity", "network", "version", "cpu_affinity"]
      else
        ["kind"]
    in
    if !(
      std.array.all
        (fun field => std.array.elem field allowed)
        (std.record.fields value)
    ) then
      'Error { message = "unknown backend field" }
    else if value.kind == "namespaces" then
      std.contract.check
        {
          kind | std.contract.from_predicate (fun v => v == "namespaces"),
          unshare | Executable,
          python | Executable,
          mount | Executable,
          shell | Executable,
          runtime_roots | Array RuntimeRoot | AtMost 6,
          network | std.contract.from_predicate (fun v => v == "none") | optional,
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
          guest_shell | GuestShell,
          acceleration | std.contract.from_predicate (fun v => v == "tcg" || v == "kvm"),
          qualification_identity | c.Nullable c.Blake3 | default = null,
          network | std.contract.from_predicate (fun v => v == "none") | optional,
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
          version | Version,
          vcpus | c.BoundedInteger 1 1,
          memory_mib | c.BoundedInteger 32 16384,
          network | std.contract.from_predicate (fun v => v == "none") | optional,
        }
        label
        value
    else if value.kind == "cloud-hypervisor" then
      std.contract.check
        {
          kind | std.contract.from_predicate (fun v => v == "cloud-hypervisor"),
          cloud_hypervisor | Executable,
          kernel | Executable,
          initrd | Executable,
          guest_shell | GuestShell,
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
          network | std.contract.from_predicate (fun v => v == "none") | optional,
          version | Version | optional,
          cpu_affinity | CpuAffinity | optional,
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
          guest_shell | GuestShell,
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
          # Reviewed VMM version; recorded in snapshot identity.
          version | Version | optional,
          # SMT/KSM-only operator exception. Unfixed/unknown kernel status
          # cannot be overridden. The reason is retained in the receipt.
          host_preflight_override
            | std.contract.from_predicate (fun value =>
              std.is_string value
              && std.string.trim value != ""
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
              && std.string.trim value != ""
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
              && std.string.trim value != ""
              && std.string.length value > 0
              && std.string.length value <= 128
              && !(std.string.is_match "\\x00" value)
            )
            | optional,
          # Existing operator-created resctrl CAT group; Rust verifies assignment.
          cache_partition | Identifier | optional,
          # Pin VMM and inherited vCPU threads; physical host exclusion still applies.
          cpu_affinity | CpuAffinity | optional,
          qualification_identity | c.Nullable c.Blake3 | default = null,
          network | std.contract.from_predicate (fun v => v == "none") | optional,
        }
        label
        value
    else
      'Error { message = "Backend kind must be namespaces, qemu, firecracker, cloud-hypervisor or unikernel-test" }
  )
in
let CacheRelative = (import "field-contracts.ncl").CacheRelative in
let Cache =
  std.contract.custom (fun label value =>
    let allowed = ["directory", "maximum_entry_bytes", "maximum_total_bytes", "maximum_entries", "maximum_age_seconds", "maximum_files", "policies"]
    in
    if !(ExactFields allowed value) then
      'Error { message = "unknown cache field" }
    else
      let result =
        std.contract.check
          {
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
                    producer | CacheText,
                    toolchain | CacheText,
                    platform | CacheText,
                    inputs | Array CacheRelative | NonEmptyAtMost 4096 | NonOverlappingPaths,
                    paths | Array CacheRelative | NonEmptyAtMost 4096 | NonOverlappingPaths,
                    maximum_entry_bytes | c.BoundedInteger 1 1048576,
                    maximum_files | c.BoundedInteger 1 4096,
                  },
              }
              | NonEmptyAtMost 128
              | UniqueCacheScopes
              | CacheIo,
          }
          label
          value
      in
      result
      |> match {
        'Ok cache =>
          if std.array.all
            (fun policy =>
              policy.worker.maximum_entry_bytes <= cache.maximum_entry_bytes
              && policy.worker.maximum_files <= cache.maximum_files
            )
            cache.policies then
            'Ok cache
          else
            'Error { message = "cache policy exceeds operator bounds" },
        'Error error => 'Error error,
      }
  )
in
let Execution =
  std.contract.custom (fun label value =>
    let allowed = ["systemd_run", "systemctl", "taskset", "manager_runtime_directory", "cache", "trusted_projects", "pricing", "workload", "env", "work_root", "runners", "maximum_concurrent", "memory_bytes", "processes", "cpu_percent", "maximum_timeout_ms", "maximum_output_bytes"]
    in
    if !(ExactFields allowed value) then
      'Error { message = "unknown execution field" }
    else
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
                price_id | Identifier,
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
                    environment | Environment,
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
            runners
              | { _ : Backend }
              | NonEmptyRecord
              | RecordAtMost 32
              | std.contract.from_predicate (fun runners =>
                std.array.all
                  (fun label =>
                    std.string.length label <= 128
                    && std.string.is_match "^[A-Za-z0-9_.-]+$" label
                  )
                  (std.record.fields runners)
              ),
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
            (std.record.values admitted.runners)
          && std.array.all
            (fun runner =>
              !(
                runner.kind == "firecracker"
                && std.record.has_field "debug_hold" runner
              )
              || runner.debug_hold.max_concurrent <= admitted.maximum_concurrent
            )
            (std.record.values admitted.runners)
          && std.array.all
            (fun runner =>
              !(std.record.has_field "warm_pool" runner)
              || runner.warm_pool == null
              || runner.warm_pool.slots <= admitted.maximum_concurrent
            )
            (std.record.values admitted.runners)
          && std.array.all
            (fun runner =>
              runner.kind != "unikernel-test"
              || runner.memory_mib
              * 1048576 <= admitted.memory_bytes
            )
            (std.record.values admitted.runners)
          && std.array.all
            (fun runner =>
              !(std.array.elem runner.kind ["firecracker", "cloud-hypervisor"])
              || (
                runner.memory_mib
                * 1048576
                + 67108864 <= admitted.memory_bytes
                && runner.vcpus
                + 4 <= admitted.processes
                && runner.boot_timeout_ms < admitted.maximum_timeout_ms
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
    let allowed = ["driver", "minimum", "maximum", "slots_per_instance", "organizations", "pricing", "timeout_ms"]
    in
    if !(ExactFields allowed value) then
      'Error { message = "unknown fleet field" }
    else
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
                price_id | Identifier,
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
          }
          | std.contract.from_predicate (ExactFields ["key_file", "bind_address", "relay_urls"]),
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
            price_id | c.Nullable Identifier | default = null,
            class | c.Nullable Identifier | default = null,
            max_units | c.Nullable (c.BoundedInteger 1 1000000000) | default = null,
          }
          | AtMost 128
          | ExactSourceProfiles
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
            pipeline_id | Identifier,
            price_id | Identifier,
            class | Identifier,
          }
          | AtMost 128
          | ExactLattice
          | UniqueLatticeIds
          | default
          = [],
        operations | OptionalBuilder (import "hosted-operations.ncl") | default = null,
        secrets | OptionalBuilder (import "hosted-secrets.ncl") | default = null,
        execution | c.Nullable Execution | default = null,
        fleet | c.Nullable Fleet | default = null,
      }
      | std.contract.from_predicate (ExactFields ["schema", "service_id", "state_directory", "network", "bootstrap_peer", "bootstrap_organization", "operator_peers", "source_profiles", "lattice_execution", "operations", "secrets", "execution", "fleet"])
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
