# r[impl kiln.patchgate.profile] r[impl kiln.patchgate.nonclaims]
let MaximumChanges = 256 in
let MaximumEdges = 16384 in
let MaximumDocumentBytes = 16777216 in
let MaximumExecutableBytes = 268435456 in
let MaximumTimeoutMilliseconds = 3600000 in
let MaximumNativeTimeoutSeconds = 3600 in
let MaximumReviewRequests = 65536 in
let MaximumLabelCharacters = 128 in
let Integer = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= minimum && value <= maximum && std.number.floor value == value
  )
in
let Label =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= MaximumLabelCharacters
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^/[^\\x00]+$" value
  )
in
let Digest =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let NativeHash =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[A-Z2-7]{53}$" value
  )
in
let Nullable = fun Contract =>
  std.contract.custom (fun label value =>
    if value == null then 'Ok value else std.contract.check Contract label value
  )
in
let MainBinding = { channel | Label, state | Nullable NativeHash } in
let PolicyBinding = { path | Label, sha256 | Digest } in
let ConfigurationValues = {
  policy_path | Label,
  protected | Array String,
  documentation_paths | Array String,
  max_change_chars | Integer 1 MaximumDocumentBytes,
  timeout_seconds | Integer 1 MaximumNativeTimeoutSeconds,
  check_timeout_seconds | Integer 1 MaximumNativeTimeoutSeconds,
  max_review_requests | Integer 1 MaximumReviewRequests,
  review_timeout_seconds | Integer 1 MaximumNativeTimeoutSeconds,
}
in
let ConfigurationEvidence = {
  source | std.contract.from_predicate (fun value => std.array.elem value ["main", "environment"]),
  path | Nullable Label,
  sha256 | Nullable Digest,
  values | ConfigurationValues,
}
in
let Limits = {
  max_changes | Integer 1 MaximumChanges,
  max_edges | Integer 0 MaximumEdges,
  max_request_bytes | Integer 1 MaximumDocumentBytes,
  max_report_bytes | Integer 1 MaximumDocumentBytes,
  max_stderr_bytes | Integer 1 MaximumDocumentBytes,
  max_executable_bytes | Integer 1 MaximumExecutableBytes,
  timeout_ms | Integer 1 MaximumTimeoutMilliseconds,
  poll_interval_ms | Integer 1 MaximumTimeoutMilliseconds,
  teardown_timeout_ms | Integer 1 MaximumTimeoutMilliseconds,
}
in
let Settings = {
  id | Label,
  program | AbsolutePath,
  executable_identity | Digest,
  repository_root | AbsolutePath,
  environment | { _ : String },
  native
    | {
      main | MainBinding,
      inbox | MainBinding,
      configuration | ConfigurationEvidence,
      policy | Nullable PolicyBinding,
    },
  limits | Limits,
}
in
let Revision = "97c1216fe13000d9dcbb97f70ddd5423180eea2c" in
# Program, repository, model settings, native states and digests are operator
# inputs. Nothing here acquires private source or invents production identities.
fun settings =>
  let checked | Settings = settings in
  {
    source_revision = Revision,
    report_schema = 1,
    program = checked.program,
    executable_identity = checked.executable_identity,
    repository_root = checked.repository_root,
    environment = checked.environment,
    provider = {
      contract_version = 1,
      profile_id = checked.id,
      role = "Collaboration",
      namespace = {
        contract_version = 1,
        provider_namespace = "patchgate",
        native_kind = "review-provider",
        native_id = Revision,
      },
      operations = ["Observe", "Reconcile"],
      outcomes = [],
      limits = {
        max_request_bytes = checked.limits.max_request_bytes,
        max_result_bytes = checked.limits.max_report_bytes,
        max_in_flight = 1,
      },
    },
    native = checked.native,
    limits = checked.limits,
  }
