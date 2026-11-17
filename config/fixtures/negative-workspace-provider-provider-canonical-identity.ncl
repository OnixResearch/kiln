let Provider = import "../workspace-provider-profile.ncl" in
let base = import "workspace-provider-archil-base.ncl" in
(base |> std.record.update "identities" (base.identities |> std.record.update "canonical_algorithm" "archil-disk-id"))
  | Provider.WorkspaceProviderProfile
