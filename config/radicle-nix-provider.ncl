let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0
  )
in
let NonEmpty =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^/[^\\x00?#]+$" value
  )
in
let Blake3Hex =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let SafeLabel =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[A-Za-z0-9_-]+$" value
  )
in
let HttpsUrl =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^https://[^\\x00]+$" value
  )
in
let Settings = {
  allowed_user_id | Positive,
  repository | NonEmpty,
  source_root | AbsolutePath,
  wrapper_path | AbsolutePath,
  wrapper_blake3 | Blake3Hex,
  working_directory | AbsolutePath,
  report_root | AbsolutePath,
  report_namespace | SafeLabel,
  report_base_url | HttpsUrl,
  timeout_milliseconds | Positive,
  stdout_max_bytes | Positive,
  stderr_max_bytes | Positive,
  poll_interval_milliseconds | Positive,
  teardown_timeout_milliseconds | Positive,
  wrapper_max_bytes | Positive,
  report_max_bytes | Positive,
  stage_collision_attempts | Positive,
}
in
let fixed_stdin_max_bytes = 1 in
fun settings =>
  let checked | Settings = settings in
  {
    schema = "kiln.radicle-nix-provider.v1",
    bounded_exec_revision = "29dac88ecded94457572db3fdfaaaab95fa91525",
    durable_file_publication_revision = "8e05e74e24b45f752d77145c4455385daaf6d6ab",
    allowed_user_id = checked.allowed_user_id,
    repository = checked.repository,
    source_root = checked.source_root,
    wrapper_path = checked.wrapper_path,
    wrapper_blake3 = checked.wrapper_blake3,
    working_directory = checked.working_directory,
    report_root = checked.report_root,
    report_namespace = checked.report_namespace,
    report_base_url = checked.report_base_url,
    limits = {
      timeout_milliseconds = checked.timeout_milliseconds,
      stdin_max_bytes = fixed_stdin_max_bytes,
      stdout_max_bytes = checked.stdout_max_bytes,
      stderr_max_bytes = checked.stderr_max_bytes,
      poll_interval_milliseconds = checked.poll_interval_milliseconds,
      teardown_timeout_milliseconds = checked.teardown_timeout_milliseconds,
      wrapper_max_bytes = checked.wrapper_max_bytes,
      report_max_bytes = checked.report_max_bytes,
      stage_collision_attempts = checked.stage_collision_attempts,
    },
    non_claims = [
      "a local source path does not prove source trust or revision authority",
      "bounded execution does not prove sandboxing or hermeticity",
      "a process exit does not prove CI correctness",
      "report publication does not prove external report delivery",
      "filesystem synchronization does not prove storage-device persistence",
      "a provider receipt does not prove distributed exactly-once execution",
      "provider execution does not establish release eligibility",
    ],
  }
