let profile = import "../lattice-workflow-profile.ncl" in
profile & {
  profile.effect_classes = ["build", "publish"],
}
