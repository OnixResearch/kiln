# r[impl kiln.radicle_ci.live.authority]
# Pure admission only; the shell atomically creates the fresh root before effects.
let Status = (import "radicle-status-profile.ncl").profile in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let CanaryRoot =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/var/tmp/[^/\\x00]+(/[^/\\x00]+)*$" value
    && !std.string.is_match "(^|/)\\.{1,2}(/|$)" value
  )
in
let StoreExecutable =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[A-Za-z0-9+._?=-]+/bin/[A-Za-z0-9+._-]+$" value
    && !std.string.is_match "(^|/)\\.{1,2}(/|$)" value
  )
in
fun inputs =>
  let selected | {
    root | CanaryRoot,
    source_branch | String | Exact "ci-canary",
    status_checkpoint_branch | String | Exact "kiln-status-checkpoint",
    tools | {
      radicle | StoreExecutable,
      radicle_node | StoreExecutable,
      broker | StoreExecutable,
      broker_tool | StoreExecutable,
      jj | StoreExecutable,
      git | StoreExecutable,
      secretspec | String | Exact Status.signer.executable_path,
    },
  } = inputs in
  {
    root = selected.root,
    source_branch = selected.source_branch,
    status_checkpoint_branch = selected.status_checkpoint_branch,
    tools = selected.tools,
    network = "test",
    listen_host = "127.0.0.1",
    command_seconds = 360,
    recovery_seconds = 360,
    maximum_service_log_bytes = 33554432,
    maximum_document_bytes = 4194304,
    callback_timeout_milliseconds = 240000,
    maximum_polls = 120,
    poll_interval_milliseconds = 1000,
    host_maximum_requests = 256,
    build_delay_seconds = 8,
  }
