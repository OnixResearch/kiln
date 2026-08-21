let Source = import "../radicle-status-profile.ncl" in
let Signer = {
  project_root,
  project,
  profile,
  secret_key,
  provider,
  secretspec_version,
  secretspec_source_revision,
  secretspec_license,
  nixpkgs_revision,
  executable_path,
  executable_blake3,
} in
std.record.remove "secret_key" Source.profile.signer | Signer