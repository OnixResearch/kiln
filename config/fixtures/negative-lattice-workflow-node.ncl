let profile = import "../lattice-workflow-profile.ncl" in
profile & {
  profile.node_kinds = ["build", "secret-shell"],
}
