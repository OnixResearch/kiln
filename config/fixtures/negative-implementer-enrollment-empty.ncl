# An empty actor list cannot enroll implementation work.
let input = {
  repositories = ["onix/kiln"],
  actors = [],
}
in
(import "../implementer-enrollment.ncl") input
