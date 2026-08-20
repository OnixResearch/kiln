let profile = import "../lattice-workflow-profile.ncl" in
profile & {
  profile.transport_limits.maximum_frame_bytes = 0,
}
