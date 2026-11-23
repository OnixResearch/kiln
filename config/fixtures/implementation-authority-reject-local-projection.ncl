# The local-operator profile never enables projection routes; it has no such
# field.
let input = {
  schema = "kiln.implementation-authority-config.v1",
  profile = {
    local_operator = {
      operator = "onix-local-operator",
      projection_enabled = true,
    },
  },
}
in
(import "../implementation-authority.ncl") input
