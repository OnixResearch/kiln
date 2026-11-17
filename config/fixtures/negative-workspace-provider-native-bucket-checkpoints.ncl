let Provider = import "../workspace-provider-profile.ncl" in
let base = import "workspace-provider-archil-base.ncl" in
(base |> std.record.update "storage" (base.storage |> std.record.update "format" "archil_native_bucket"))
  | Provider.WorkspaceProviderProfile
