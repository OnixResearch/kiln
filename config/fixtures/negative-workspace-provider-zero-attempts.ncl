let Provider = import "../workspace-provider-profile.ncl" in
let base = import "workspace-provider-archil-base.ncl" in
(base |> std.record.update "bounds" (base.bounds |> std.record.update "max_attempts" 0))
  | Provider.WorkspaceProviderProfile
