let NonEmptyString =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0
  )
in
let Exact = fun expected =>
  std.contract.from_predicate (fun value => value == expected)
in
let OffByDefault =
  std.contract.from_predicate (fun value =>
    std.is_bool value && value == false
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 4096
    && std.string.is_match "^/[A-Za-z0-9._/+=-]+$" value
  )
in
let OptionalAbsolutePath =
  std.contract.from_predicate (fun value =>
    value == null
    || (std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 4096
      && std.string.is_match "^/[A-Za-z0-9._/+=-]+$" value)
  )
in
let OptionalIdentifier =
  std.contract.from_predicate (fun value =>
    value == null
    || (std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 64
      && std.string.is_match "^[A-Za-z0-9._-]+$" value)
  )
in
let SlurmIdentifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9._-]+$" value
  )
in
let SlurmCapability =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.array.elem value ["CasUpload", "Execute", "Poll", "Wait", "Cancel", "LogStream"]
  )
in
let BoundsRelations =
  std.contract.from_predicate (fun bounds =>
    bounds.min_cpus <= bounds.max_cpus
    && bounds.min_memory_mb <= bounds.max_memory_mb
    && bounds.min_time_limit_minutes <= bounds.max_time_limit_minutes
    && bounds.poll_interval_ms <= bounds.command_timeout_ms
  )
in
let SlurmProfileShape = {
  schema | Exact "kiln.slurm-remote.profile.v1",
  enabled | OffByDefault,
  provider | SlurmIdentifier,
  cluster_name | OptionalIdentifier,
  executables
    | {
      sbatch_path | AbsolutePath,
      squeue_path | AbsolutePath,
      scancel_path | AbsolutePath,
      sacct_path | OptionalAbsolutePath,
    },
  allowed_partitions | Array SlurmIdentifier,
  allowed_accounts | Array SlurmIdentifier,
  allowed_working_dir_prefixes | Array AbsolutePath,
  log_root | OptionalAbsolutePath,
  capabilities | Array SlurmCapability,
  bounds
    | {
      min_cpus | Positive,
      max_cpus | Positive,
      min_memory_mb | Positive,
      max_memory_mb | Positive,
      min_time_limit_minutes | Positive,
      max_time_limit_minutes | Positive,
      max_argv_items | Positive,
      max_arg_bytes | Positive,
      max_action_bytes | Positive,
      max_stdout_bytes | Positive,
      max_stderr_bytes | Positive,
      max_log_bytes | Positive,
      max_in_flight | Positive,
      max_wait_polls | Positive,
      command_timeout_ms | Positive,
      poll_interval_ms | Positive,
      teardown_timeout_ms | Positive,
      stdin_max_bytes | Positive,
    }
    | BoundsRelations,
}
in
let MinCpus = 1 in
let MaxCpus = 64 in
let MinMemoryMb = 128 in
let MaxMemoryMb = 262144 in
let MinTimeLimitMinutes = 1 in
let MaxTimeLimitMinutes = 1440 in
let MaxArgvItems = 64 in
let MaxArgBytes = 4096 in
let MaxActionBytes = 65536 in
let MaxStdoutBytes = 65536 in
let MaxStderrBytes = 65536 in
let MaxLogBytes = 1048576 in
let MaxInFlight = 16 in
let MaxWaitPolls = 64 in
let CommandTimeoutMs = 15000 in
let PollIntervalMs = 25 in
let TeardownTimeoutMs = 1000 in
let StdinMaxBytes = 1 in
{
  profile | SlurmProfileShape = {
    schema = "kiln.slurm-remote.profile.v1",
    enabled = false,
    provider = "slurm-remote-v1",
    cluster_name = null,
    executables = {
      sbatch_path = "/run/current-system/sw/bin/sbatch",
      squeue_path = "/run/current-system/sw/bin/squeue",
      scancel_path = "/run/current-system/sw/bin/scancel",
      sacct_path = null,
    },
    allowed_partitions = ["ci-batch"],
    allowed_accounts = ["kiln-ci"],
    allowed_working_dir_prefixes = ["/var/lib/kiln/slurm-work"],
    log_root = "/var/lib/kiln/slurm-logs",
    capabilities = ["CasUpload", "Execute", "Poll", "Wait", "Cancel", "LogStream"],
    bounds = {
      min_cpus = MinCpus,
      max_cpus = MaxCpus,
      min_memory_mb = MinMemoryMb,
      max_memory_mb = MaxMemoryMb,
      min_time_limit_minutes = MinTimeLimitMinutes,
      max_time_limit_minutes = MaxTimeLimitMinutes,
      max_argv_items = MaxArgvItems,
      max_arg_bytes = MaxArgBytes,
      max_action_bytes = MaxActionBytes,
      max_stdout_bytes = MaxStdoutBytes,
      max_stderr_bytes = MaxStderrBytes,
      max_log_bytes = MaxLogBytes,
      max_in_flight = MaxInFlight,
      max_wait_polls = MaxWaitPolls,
      command_timeout_ms = CommandTimeoutMs,
      poll_interval_ms = PollIntervalMs,
      teardown_timeout_ms = TeardownTimeoutMs,
      stdin_max_bytes = StdinMaxBytes,
    },
  },
  unsupported_features | Array NonEmptyString = [
    "interactive_pty",
    "live_attach",
    "gpu_gres",
    "job_array",
    "multi_node",
    "guest_networking",
  ],
  required_non_claims | Array NonEmptyString = [
    "slurm placement does not replace Kiln tenant fairness or admission policy",
    "slurm cgroups or fairshare do not establish Kiln guest isolation",
    "slurm job submission does not prove guest execution or host KVM preflight",
    "missing or ambiguous squeue observations remain Unknown until reconciled",
  ],
}
