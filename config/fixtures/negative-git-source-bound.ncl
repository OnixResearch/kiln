let profile = import "../git-source-profile.ncl" in
profile & {
  profile.limits.max_entries = 0,
}
