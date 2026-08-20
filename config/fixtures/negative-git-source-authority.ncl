let profile = import "../git-source-profile.ncl" in
profile & {
  profile.allow_network = true,
}
