let Provider = import "../workspace-provider-profile.ncl" in
let base = import "workspace-provider-archil-base.ncl" in
(base |> std.record.update "qualification" "qualified")
  | Provider.WorkspaceProviderProfile
