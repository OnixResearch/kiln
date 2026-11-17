# Restricted views from one disk per view: the worker's credential covers only
# the disk holding its view.
let Provider = import "../workspace-provider-profile.ncl" in
let base = import "workspace-provider-archil-base.ncl" in
(
  base
  |> std.record.update "profile_id" "archil-managed-disk-per-view-v1"
  |> std.record.update "writable_attempts" "disk_per_attempt"
  |> std.record.update "restricted_views" "disk_per_view"
)
  | Provider.WorkspaceProviderProfile
