let Provider = import "../workspace-provider-profile.ncl" in
let base = import "workspace-provider-archil-base.ncl" in
(base |> std.record.update "provider" "local-materialization" |> std.record.update "restricted_views" "disk_per_view")
  | Provider.WorkspaceProviderProfile
