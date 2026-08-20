let profile = import "../lattice-workflow-profile.ncl" in
profile & {
  profile.controls.pause = false,
}
