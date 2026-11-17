let Provider = import "../workspace-provider-profile.ncl" in
let base = import "workspace-provider-archil-base.ncl" in
(base |> std.record.update "durability" (base.durability |> std.record.update "bucket_copy_is_acknowledgement" true))
  | Provider.WorkspaceProviderProfile
