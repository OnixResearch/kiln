let Source = import "../radicle-mutation-profile.ncl" in
let Signer = {
  project_root,
  project,
  profile,
  secret_key,
  provider,
  secretspec_version,
  executable_path,
  executable_blake3,
} in
std.record.remove "secret_key" Source.profile.signer | Signer