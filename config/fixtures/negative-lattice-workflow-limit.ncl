let profile = import "../lattice-workflow-profile.ncl" in
profile & {
  profile.limits.max_nodes = 0,
}
