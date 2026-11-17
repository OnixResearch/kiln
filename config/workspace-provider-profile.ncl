# Versioned capability profile for a durable projected-workspace provider (#82).
#
# A profile declares what one provider configuration claims to supply behind
# Kiln's pure core and provider shell. Canonical source, projection and
# candidate identities stay BLAKE3 and Kiln-owned; provider disk, branch,
# checkpoint and mount identifiers are opaque references, never identities.
# The contract refuses profiles that would rely on a mechanism the provider
# documents as insufficient: a branch as an access boundary, checkpoints on a
# native-format bucket, bucket synchronization as a durable acknowledgement,
# forced ownership as an implicit retry, or forks that share an attached disk.
# Declaring a profile grants no workspace, source or execution authority.
let MaxProfileBytes = 256 in
let MaxWorkspaceBytes = 1099511627776 in
let MaxWorkspaceFiles = 10000000 in
let MaxAttempts = 256 in
let MaxDeadlineMs = 86400000 in
let MaxCostCents = 100000 in

let BoundedText =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= MaxProfileBytes
  )
in
let Bounded = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && std.number.is_integer value && value > 0 && value <= maximum
  )
in
let Literal = fun expected =>
  std.contract.from_predicate (fun value => value == expected)
in
let OneOf = fun allowed =>
  std.contract.from_predicate (fun value =>
    std.is_string value && std.array.elem value allowed
  )
in

let Profile = {
  schema | Literal "kiln.workspace-provider-profile.v1",
  profile_id | BoundedText,
  provider | OneOf ["local-materialization", "archil"],
  # Evidence behind the declared capabilities. A vendor claim or a
  # deterministic fixture never qualifies a profile.
  evidence_class | OneOf ["deterministic_fixture", "adapter_execution", "vendor_claim"],
  qualification | OneOf ["unqualified", "qualified"],
  identities
    | {
      canonical_algorithm | Literal "blake3",
      # Provider identifiers are references bound to a canonical identity.
      provider_references | Array (OneOf ["directory", "disk", "branch", "checkpoint", "mount", "sandbox"]),
    },
  storage
    | {
      format | OneOf ["local_filesystem", "archil_flat_block", "archil_native_bucket"],
      checkpoints | Bool,
      branches | Bool,
      # Mode bits, ownership and links survive only where the provider keeps them.
      posix_metadata | OneOf ["preserved", "lost_in_bucket_copy"],
    },
  # How independent attempts get private writable state.
  writable_attempts | OneOf ["private_copy", "branch_per_attempt", "disk_per_attempt"],
  # Scope of the credential a worker holds to reach the workspace.
  credential_scope | OneOf ["none", "whole_disk"],
  # Where a restricted view is enforced; a branch or checkpoint is not an
  # access boundary because credentials cover the whole disk.
  restricted_views | OneOf ["kiln_materialization_boundary", "disk_per_view", "ineligible"],
  durability
    | {
      acknowledgement | OneOf ["fsync", "provider_sync", "unmount", "checkin"],
      bucket_copy_is_acknowledgement | Literal false,
    },
  ownership
    | {
      stale_writer_boundary | OneOf ["private_directory", "path_delegation", "credential_expiry"],
      forced_takeover_is_retry | Literal false,
    },
  cleanup
    | {
      lifecycle | OneOf ["local_directory", "disposable_disk"],
      delete_branches | Bool,
      delete_checkpoints | Bool,
    },
  bounds
    | {
      max_workspace_bytes | Bounded MaxWorkspaceBytes,
      max_workspace_files | Bounded MaxWorkspaceFiles,
      max_attempts | Bounded MaxAttempts,
      deadline_ms | Bounded MaxDeadlineMs,
      max_cost_cents | Bounded MaxCostCents,
    },
  non_claims | Array BoundedText,
}
in

# Cross-field rules over a profile that already has the declared shape.
let cross_checked = fun value =>
  if value.qualification == "qualified" && value.evidence_class != "adapter_execution" then
    'Error { message = "only an adapter execution can qualify a workspace provider profile" }
  else if value.storage.format == "archil_native_bucket" && (value.storage.checkpoints || value.storage.branches) then
    'Error { message = "native-format bucket disks support neither checkpoints nor branches" }
  else if value.writable_attempts == "branch_per_attempt" && !value.storage.branches then
    'Error { message = "branch-per-attempt isolation requires branch support" }
  else if value.provider == "local-materialization" && value.credential_scope != "none" then
    'Error { message = "local materialization holds no provider credential" }
  else if value.credential_scope == "whole_disk" && value.restricted_views == "kiln_materialization_boundary" then
    'Error { message = "a worker holding a whole-disk credential can reach data outside its view" }
  else if value.storage.branches && !(value.cleanup.delete_branches && value.cleanup.delete_checkpoints) && value.cleanup.lifecycle != "disposable_disk" then
    'Error { message = "branches or checkpoints that cannot be deleted require a disposable-disk lifecycle" }
  else
    'Ok value
in

{
  WorkspaceProviderProfile =
    std.contract.custom (fun label value =>
      std.contract.check Profile label value
      |> match {
        'Ok checked => cross_checked checked,
        'Error error => 'Error error,
      }
    ),
}
