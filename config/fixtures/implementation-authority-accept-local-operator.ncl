# The explicit local-operator compatibility profile; it never admits projection
# routes.
let input = {
  schema = "kiln.implementation-authority-config.v1",
  profile = { local_operator = { operator = "onix-local-operator" } },
}
in
(import "../implementation-authority.ncl") input
