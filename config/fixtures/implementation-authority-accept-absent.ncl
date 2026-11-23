# The absent profile: every protected operation denies.
let input = {
  schema = "kiln.implementation-authority-config.v1",
  profile = 'absent,
}
in
(import "../implementation-authority.ncl") input
