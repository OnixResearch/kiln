# Shared fixture body: a declared Archil managed flat-block profile with one
# branch per attempt. Positive and negative fixtures apply the contract.
{
  schema = "kiln.workspace-provider-profile.v1",
  profile_id = "archil-managed-branch-per-attempt-v1",
  provider = "archil",
  evidence_class = "vendor_claim",
  qualification = "unqualified",
  identities = {
    canonical_algorithm = "blake3",
    provider_references = ["disk", "branch", "checkpoint", "mount"],
  },
  storage = {
    format = "archil_flat_block",
    checkpoints = true,
    branches = true,
    posix_metadata = "preserved",
  },
  writable_attempts = "branch_per_attempt",
  credential_scope = "whole_disk",
  restricted_views = "ineligible",
  durability = {
    acknowledgement = "fsync",
    bucket_copy_is_acknowledgement = false,
  },
  ownership = {
    stale_writer_boundary = "path_delegation",
    forced_takeover_is_retry = false,
  },
  cleanup = {
    lifecycle = "disposable_disk",
    delete_branches = false,
    delete_checkpoints = false,
  },
  bounds = {
    max_workspace_bytes = 10737418240,
    max_workspace_files = 200000,
    max_attempts = 8,
    deadline_ms = 3600000,
    max_cost_cents = 500,
  },
  non_claims = [
    "Capabilities are documented vendor claims; no Archil adapter execution qualified this profile.",
    "Branches are not an access boundary; this profile is ineligible for restricted views.",
  ],
}
