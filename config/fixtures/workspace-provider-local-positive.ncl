# The existing local materialization route, the comparison baseline.
let Provider = import "../workspace-provider-profile.ncl" in
{
  schema = "kiln.workspace-provider-profile.v1",
  profile_id = "local-private-copy-v1",
  provider = "local-materialization",
  evidence_class = "deterministic_fixture",
  qualification = "unqualified",
  identities = { canonical_algorithm = "blake3", provider_references = ["directory"] },
  storage = { format = "local_filesystem", checkpoints = false, branches = false, posix_metadata = "preserved" },
  writable_attempts = "private_copy",
  credential_scope = "none",
  restricted_views = "kiln_materialization_boundary",
  durability = { acknowledgement = "fsync", bucket_copy_is_acknowledgement = false },
  ownership = { stale_writer_boundary = "private_directory", forced_takeover_is_retry = false },
  cleanup = { lifecycle = "local_directory", delete_branches = false, delete_checkpoints = false },
  bounds = {
    max_workspace_bytes = 10737418240,
    max_workspace_files = 200000,
    max_attempts = 8,
    deadline_ms = 3600000,
    max_cost_cents = 1,
  },
  non_claims = ["A private local copy is the baseline, not a durable cross-worker workspace."],
}
  | Provider.WorkspaceProviderProfile
