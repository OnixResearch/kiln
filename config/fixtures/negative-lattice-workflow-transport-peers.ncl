let profile = import "../lattice-workflow-profile.ncl" in
profile & {
  profile.allowed_user_ids = [],
}
