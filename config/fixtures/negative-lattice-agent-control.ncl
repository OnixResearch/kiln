let profile = import "../lattice-agent-profile.ncl" in
profile & {
  profile.controls.pause = true,
}
