let Provider = import "../workspace-provider-profile.ncl" in
let base = import "workspace-provider-archil-base.ncl" in
(base |> std.record.update "ownership" (base.ownership |> std.record.update "forced_takeover_is_retry" true))
  | Provider.WorkspaceProviderProfile
