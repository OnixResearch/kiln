let Provider = import "../workspace-provider-profile.ncl" in
let base = import "workspace-provider-archil-base.ncl" in
(base |> std.record.update "cleanup" (base.cleanup |> std.record.update "lifecycle" "local_directory"))
  | Provider.WorkspaceProviderProfile
