# Reviewed example implementation enrollment.
let input = {
  repositories = ["onix/kiln"],
  actors = ["operator"],
}
in
(import "../implementer-enrollment.ncl") input
