# r[impl kiln.onixos_pilot.runtime] r[impl kiln.onixos_pilot.evidence]
# Human-reviewed pilot policy; the runner only fills measured identities and private paths.
let Eval = (import "nix-eval-jobs-profile.ncl").profile in
let Build = (import "nix-build-profile.ncl").profile in
let Aspen = import "aspen-runtime-profile.ncl" in
let Radicle = (import "radicle-workflow-shell.ncl").profile in
let NonEmpty = std.contract.from_predicate (fun value => std.is_string value && std.string.length value > 0) in
let Absolute = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^/[^\\x00]+$" value) in
let StoreRoot = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[A-Za-z0-9+._?=-]+$" value) in
let Blake3 = std.contract.from_predicate (fun value => std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value) in
let Case = std.contract.from_predicate (fun value => value == "success" || value == "build-failure" || value == "evaluation-failure") in
# Self-CI pilot policy: the real Kiln package build exceeds the stock 60 s
# build deadline. The controller observation bound is 600 s (600 polls at
# 1000 ms), so the build and callback deadlines are aligned to it; the worker
# workflow and host get the same 1200 s ceiling. All values remain finite.
let EvalExecutionTimeoutMilliseconds = 60000 in
let BuildExecutionTimeoutMilliseconds = 600000 in
let LifecyclePolls = 7200 in
let PollIntervalMilliseconds = 250 in
let WorkflowTimeoutSeconds = 1200 in
let RecoveryTransportExchanges = 16 in
let CallbackTimeoutMilliseconds = 1200000 in
let HostTimeoutMilliseconds = 1200000 in
fun inputs =>
  let selected
    | {
      root | Absolute,
      source_path | StoreRoot,
      source_identity | Blake3,
      case | Case,
      operation_identity | Blake3,
      worker_identity | Blake3,
      store_identity | Blake3,
      plan_identity | Blake3,
      policy_identity | Blake3,
      user_id | Number,
    }
    = inputs
  in
  {
    eval_selection = {
      kind = "stock",
      runtime = {
        executable_path = Eval.executable_path,
        executable_identity = Eval.executable_identity,
        cohort_manifest_path = Eval.cohort_manifest_path,
        cohort_manifest_identity = Eval.cohort_manifest_identity,
        gc_root_base = selected.root ++ "/eval-roots",
        home_base = selected.root ++ "/eval-home",
      },
      limits = Eval.limits & { timeout_milliseconds | force = EvalExecutionTimeoutMilliseconds },
      source_binding = {
        source = { contract_version = 1, provider_namespace = "kiln-source", native_kind = "nix-store-path", native_id = selected.source_path },
        source_identity = selected.source_identity,
        root_selector = "cases." ++ selected.case,
        system = "x86_64-linux",
      },
    },
    # Existing task composition replaces the template derivation/outputs with the
    # admitted real task and measures its derivation before provider dispatch.
    build_profile =
      Build
      & {
        home_base | force = selected.root ++ "/build-home",
        gc_root_base | force = selected.root ++ "/build-roots",
        worker_identity | force = selected.worker_identity,
        store_identity | force = selected.store_identity,
        plan_identity | force = selected.plan_identity,
        policy_identity | force = selected.policy_identity,
        limits.timeout_milliseconds | force = BuildExecutionTimeoutMilliseconds,
      },
    aspen_document = Aspen & { bounds.callback_timeout_millis | force = CallbackTimeoutMilliseconds },
    radicle_profile =
      Radicle
      & {
        socket_path | force = selected.root ++ "/lattice.sock",
        replay_database_path | force = selected.root ++ "/radicle-replay.sqlite",
        maximum_polls | force = LifecyclePolls,
        allowed_user_ids | force = [selected.user_id],
        # Lattice counts lifetime exchanges, not concurrent connections.
        transport_limits.maximum_connections | force = LifecyclePolls * 2 + RecoveryTransportExchanges,
        workflow.nodes | force = [{ id = "build", kind = "build", effect_class = "build", adapter_kind = "local", operation_identity = selected.operation_identity }],
      },
    host_options = {
      timeout_milliseconds = HostTimeoutMilliseconds,
      poll_interval_milliseconds = PollIntervalMilliseconds,
      maximum_requests = 32,
    },
    workflow_timeout_seconds = WorkflowTimeoutSeconds,
  }
