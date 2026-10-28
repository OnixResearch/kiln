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
  argv | Array String,
  environment | { _ : String },
  artifacts | Array RelativeFile,
  # Omitted declarations remain omitted, preserving existing profile JSON identity.
  reports | Array Report | optional,
  cache
    | {
      producer | String,
      toolchain | String,
      platform | String,
      inputs | Array RelativeFile,
      paths | Array RelativeFile,
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
              ),
            network | std.contract.from_predicate (fun value => std.array.elem value ["deny", "host"]),
            cpu_seconds | Integer 86400,
            address_space_bytes | Integer 1099511627776,
            maximum_file_bytes | Integer 1099511627776,
            maximum_open_files | Integer 65536,
          }
          | optional,
        heartbeat
          | {
            worker_id | Component,
            target_id | std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value),
            interval_ms | Integer 30000,
            ttl_ms | Integer 120000,
            slots | Integer 256,
          }
          | optional,
        steps | Array Step,
      }
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
