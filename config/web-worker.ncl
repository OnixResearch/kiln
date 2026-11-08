# Export this reviewed profile to JSON once. --identity pins BLAKE3 of those
# original JSON bytes, including whitespace; never hash a reserialized profile.
# Commands run in the exact admitted Git commit, not the mutable worktree.
# source_repository is the controller's canonical Git native identity (64 lowercase
# hex characters); repository_path is independent worker-local backing storage.
# Several steps may use the same native Lattice job node_id. Their real retained
# stdout/stderr are uploaded during execution with separate worker-owned byte
# cursors. Bounded Exec owns the process while private bounded transports retain
# output; native diagnostics remain separate. No progress or bytes are synthesized.
# Legacy profiles run reviewed commands with account authority. Optional execution
# policy requires Linux namespaces and the exact reviewed bubblewrap executable;
# unavailable isolation fails closed, without unsandboxed fallback.
let Integer = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= 1
    && value <= maximum
    && std.number.floor value == value
  )
in
let Between = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= minimum
    && value <= maximum
    && std.number.floor value == value
  )
in
let NonEmptyAtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value >= 1 && std.array.length value <= maximum
  )
in
let AtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= maximum
  )
in
let WorkerRelations =
  std.contract.from_predicate (fun worker =>
    let under = fun root path =>
      path == root || std.string.is_match ("^" ++ root ++ "/") path
    in
    !under worker.repository_path worker.token_file
    && !under worker.work_root worker.token_file
    && !under worker.repository_path worker.work_root
    && std.array.all
      (fun step =>
        !under worker.repository_path step.program
        && !under worker.work_root step.program
      )
      worker.steps
  )
in
let HeartbeatRelations =
  std.contract.from_predicate (fun heartbeat =>
    heartbeat.ttl_ms >= heartbeat.interval_ms * 3
  )
in
let ExecutionRelations =
  std.contract.from_predicate (fun execution =>
    let under = fun root path =>
      path == root || std.string.is_match ("^" ++ root ++ "/") path
    in
    let roots =
      if std.record.has_field "read_only_data_mount_roots" execution then
        execution.read_only_data_mount_roots
      else
        ["/srv", "/var/lib/kiln"]
    in
    let data_mounts =
      if std.record.has_field "read_only_data_mounts" execution then
        execution.read_only_data_mounts
      else
        []
    in
    std.array.any
      (fun mount => under mount execution.probe_program)
      execution.read_only_mounts
    && std.array.all
      (fun path => std.array.any (fun root => under root path) roots)
      data_mounts
  )
in
let CacheText =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 256
    && std.string.is_match "^[ -~]+$" value
  )
in
let RecordAtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.array.length (std.record.fields value) <= maximum
  )
in
let UniqueStrings =
  std.contract.from_predicate (fun values =>
    let check = fun state value =>
      if std.array.elem value state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [value], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/[^\\x00-\\x1f\\x7f]+$" value
    && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
  )
in
let Component =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[a-zA-Z0-9_.-]+$" value
    && value != "."
    && value != ".."
  )
in
let RelativeFile =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 4096
    && std.string.is_match "^[^/\\\\\\x00-\\x1f\\x7f]+(/[^/\\\\\\x00-\\x1f\\x7f]+)*$" value
    && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
  )
in
let Report = {
  kind
    | std.contract.from_predicate (fun value =>
      std.array.elem value ["junit", "cobertura", "lcov", "gate", "review", "provenance"]
    ),
  path | RelativeFile,
}
in
let Step = {
  node_id | Component,
  program | AbsolutePath,
  argv | Array String | AtMost 4096,
  environment | { _ : String } | RecordAtMost 4096,
  artifacts | Array RelativeFile | AtMost 4096,
  # Omitted declarations remain omitted, preserving existing profile JSON identity.
  reports | Array Report | AtMost 128 | optional,
  cache
    | {
      producer | CacheText,
      toolchain | CacheText,
      platform | CacheText,
      inputs | Array RelativeFile | NonEmptyAtMost 4096,
      paths | Array RelativeFile | NonEmptyAtMost 4096,
      maximum_entry_bytes | Integer 8388608,
      maximum_files | Integer 4096,
    }
    | optional,
}
in
fun input =>
  let settings =
    input
      | {
        project_id | Component,
        source_repository
          | std.contract.from_predicate (fun value =>
            std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
          ),
        repository_path | AbsolutePath,
        service_origin
          | std.contract.from_predicate (fun value =>
            std.is_string value && std.string.is_match "^https://[^/?#@]+$" value
          ),
        token_file | AbsolutePath,
        tls_ca_path | AbsolutePath,
        work_root | AbsolutePath,
        snapshot_limits
          | {
            max_entries | Integer 524288 | default = 65536,
            max_path_bytes | Integer 4096 | default = 4096,
            max_file_bytes | Integer 1073741824 | default = 16777216,
            max_total_bytes | Integer 1073741824 | default = 268435456,
          }
          | default
          = {},
        command_timeout_ms | Integer 86400000 | default = 900000,
        maximum_output_bytes | Integer 16777216 | default = 1048576,
        teardown_timeout_ms | Integer 86400000 | default = 2000,
        poll_interval_ms | Integer 86400000 | default = 10,
        http_timeout_ms | Integer 86400000 | default = 10000,
        lookup_timeout_ms | Integer 86400000 | default = 10000,
        maximum_response_bytes | Integer 1048576 | default = 1048576,
        maximum_artifact_bytes | Integer 67108864 | default = 16777216,
        maximum_total_artifact_bytes | Integer 1073741824 | default = 67108864,
        maximum_artifacts | Integer 4096 | default = 32,
        execution
          | {
            bubblewrap | AbsolutePath,
            probe_program | AbsolutePath,
            read_only_mounts
              | Array (
                std.contract.from_predicate (fun value =>
                  std.array.elem value ["/nix/store", "/usr", "/bin", "/lib", "/lib64"]
                )
              )
              | NonEmptyAtMost 16
              | UniqueStrings,
            # Explicit operator-reviewed data roots; Rust admits only /srv and
            # /var/lib/kiln by default, binds them read-only, and rejects links.
            read_only_data_mounts | Array AbsolutePath | AtMost 4 | UniqueStrings | optional,
            # Optional reviewed roots for data mounts. Omitted keeps the default
            # roots; explicit roots must stay in the reviewed data families.
            read_only_data_mount_roots
              | Array AbsolutePath
              | NonEmptyAtMost 4
              | UniqueStrings
              | optional,
            network | std.contract.from_predicate (fun value => std.array.elem value ["deny", "host"]),
            cpu_seconds | Integer 86400,
            address_space_bytes | Between 16777216 1099511627776,
            maximum_file_bytes | Integer 1099511627776,
            maximum_open_files | Between 16 65536,
          }
          | ExecutionRelations
          | optional,
        heartbeat
          | {
            worker_id | Component,
            target_id | std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value),
            interval_ms | Between 100 30000,
            ttl_ms | Integer 120000,
            slots | Integer 256,
          }
          | HeartbeatRelations
          | optional,
        # r[impl kiln.untrusted.execution] Declares that this worker runs inside a
        # per-job microVM without credentials or cache; only then does it accept
        # untrusted run bindings. Omitted means it refuses them.
        untrusted_capable | Bool | optional,
        steps | Array Step | NonEmptyAtMost 128,
      }
      | std.contract.from_predicate (fun value =>
        value.snapshot_limits.max_file_bytes <= value.snapshot_limits.max_total_bytes
        && value.maximum_total_artifact_bytes >= value.maximum_artifact_bytes
      )
      | WorkerRelations
  in
  {
    schema = "kiln.web-worker.v2",
    project_id = settings.project_id,
    source_repository = settings.source_repository,
    repository_path = settings.repository_path,
    service_origin = settings.service_origin,
    token_file = settings.token_file,
    tls_ca_path = settings.tls_ca_path,
    work_root = settings.work_root,
    snapshot_limits = settings.snapshot_limits,
    command_timeout_ms = settings.command_timeout_ms,
    maximum_output_bytes = settings.maximum_output_bytes,
    teardown_timeout_ms = settings.teardown_timeout_ms,
    poll_interval_ms = settings.poll_interval_ms,
    http_timeout_ms = settings.http_timeout_ms,
    lookup_timeout_ms = settings.lookup_timeout_ms,
    maximum_response_bytes = settings.maximum_response_bytes,
    maximum_artifact_bytes = settings.maximum_artifact_bytes,
    maximum_total_artifact_bytes = settings.maximum_total_artifact_bytes,
    maximum_artifacts = settings.maximum_artifacts,
    steps = settings.steps,
  }
  & (if std.record.has_field "execution" settings then { execution = settings.execution } else {})
  & (if std.record.has_field "heartbeat" settings then { heartbeat = settings.heartbeat } else {})
  & (if std.record.has_field "untrusted_capable" settings then { untrusted_capable = settings.untrusted_capable } else {})
