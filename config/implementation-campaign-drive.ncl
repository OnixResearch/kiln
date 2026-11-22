# Canonical human-authored profile contract for the opt-in native campaign driver.
# Export a value checked with .Profile; runtime owners revalidate identities,
# permissions, exact source bindings, authority, and native capabilities.
let Text =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let Digest =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Absolute =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/" value
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let Integer = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= minimum
    && value <= maximum
    && value == std.number.truncate value
  )
in
let Positive = Integer 1 9007199254740991 in
let UserId = Integer 1 4294967295 in
let AtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= maximum
  )
in
let Choice = fun choices => std.contract.from_predicate (fun value => std.array.elem value choices) in
let ExternalRef = {
  contract_version | Integer 1 1,
  provider_namespace | Text,
  native_kind | Text,
  native_id | Text,
}
in
let RoleContract = {
  program | Absolute,
  arguments | Array String | AtMost 32 | default = [],
  blake3 | Digest,
  uid | UserId,
  gid | UserId,
}
in
let Lifecycle = {
  lifecycle_action | Choice ["observe_reviewed_sync", "observe_reviewed_archive"],
  lifecycle_receipt_template | Text,
}
in
let GitHubDelivery =
  Lifecycle
  & {
    publisher | RoleContract,
    integrator | RoleContract,
    lifecycle_observer | RoleContract,
    enrollment | Absolute,
    secret_file | Absolute,
    remote_profile | Absolute,
    publication_policy | { base_branch | Text, title | Text, body | String },
    landing_policy
      | {
        required_checks | Array Text | AtMost 64,
        required_approvals | Integer 0 16,
      },
    repository | Absolute,
    authoritative_report_directory | Absolute,
    timeout_ms | Integer 1 300000,
  }
in
let NativeGitDelivery =
  Lifecycle
  & {
    qualifier | RoleContract,
    lifecycle_observer | RoleContract,
    enrollment | Absolute,
    service_profile | Absolute,
    review_profile | Absolute,
    repository_path | Absolute,
    change_id | Text,
    remote
      | {
        remote_name | Text,
        remote_url | Text,
        transport | Choice ["https", "ssh_strict_host_key"],
        ssh_host_key_sha256 | Text | optional,
      },
    provider_profile | Text,
    expected_old_target | ExternalRef,
    target_ref | Text,
    authoritative_report_directory | Absolute,
  }
in
let Delivery = {
  route | Choice ["github_pull_request", "native_git"],
  github | GitHubDelivery | optional,
  native_git | NativeGitDelivery | optional,
}
in
let DeliverySelection =
  std.contract.from_predicate (fun value =>
    if value.route == "github_pull_request" then
      std.record.has_field "github" value && !std.record.has_field "native_git" value
    else
      std.record.has_field "native_git" value && !std.record.has_field "github" value
  )
in
{
  RoleProgram = RoleContract,
  Profile = {
    schema | Choice ["kiln.implementation-campaign-drive.v1"],
    campaign_id | Text,
    campaign_database | Absolute,
    implementation_database | Absolute,
    verification_routes
      | {
        _ : {
          ci_service_profile | Absolute,
          verification_profile | Absolute,
        }
      },
    verifier_authority | Absolute,
    # Explicit local-operator profile for protected campaign effects.
    local_operator | String | optional,
    decision_contexts | { _ : Absolute } | default = {},
    bound_input_directory | Absolute,
    worker
      | {
        database | Absolute,
        enrollment | Absolute,
        dispatch_plan | Absolute,
        request | Absolute,
        transport_profile | Absolute,
        admission_observations | Absolute | optional,
      },
    candidate_export | { output_parent | Absolute },
    roles
      | {
        setpriv | Absolute,
        setpriv_blake3 | Digest,
        worker | RoleContract,
        exporter | RoleContract,
        capture | RoleContract,
        verifier | RoleContract,
      },
    native_cairn
      | {
        cairn
          | {
            executable | Absolute,
            repository_root | Absolute,
            policy_path | Absolute,
            executable_identity | Digest,
            policy_identity | Digest,
          },
        implementation_policy
          | {
            repository | Text,
            project | Text,
            base | ExternalRef,
            actor | Text,
            requested_outcome | Text,
            acceptance | Array { id | Text, statement | Text },
            permitted_paths | Array Text,
            permitted_effects | Array Text,
            verification_profile | Digest,
            budget | { attempts | Integer 1 255, minutes_per_attempt | Positive },
            terminal_policy | Choice ["verified_pull_request", "verified_or_already_satisfied", "verified_integrated"],
            priority | Integer 0 255,
          },
        source | ExternalRef,
        # Explicit intake. Absent keeps the legacy readiness/intake reader
        # (`legacy_readiness_compatibility`); `typed_snapshot_v1` also reads
        # and retains `cairn.selected-work.v1` per member and attempt.
        intake | Choice ["typed_snapshot_v1", "legacy_readiness_compatibility"] | optional,
        obligation_definition | Text | optional,
      },
    delivery | Delivery | DeliverySelection,
    claim_lease_ms | Positive,
    maximum_operations | Integer 1 128,
    # Optional pacing for continue-implementation-campaign; absent profiles
    # use the named defaults. retry_base_ms must not exceed retry_max_ms.
    continuation
      | {
        rescan_interval_ms | Integer 1000 86400000,
        retry_base_ms | Integer 1000 86400000,
        retry_max_ms | Integer 1000 86400000,
        max_consecutive_failures | Integer 1 64,
        max_batches | Integer 1 32,
      }
      | optional,
    workspace
      | {
        repository | Text,
        repository_path | Absolute,
        work_root | Absolute,
        max_entries | Positive,
        max_path_bytes | Positive,
        max_file_bytes | Positive,
        max_total_bytes | Positive,
      },
  },
}
