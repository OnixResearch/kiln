# Bounded local-admission profile constructor, not a live-worker qualification.
# coordinator must come from the newly built reviewed Kiln cohort.
fun coordinator =>
  let tools = import "observed-tools.ncl" in
  (import "../../config/implementation-qualification.ncl") {
    schema = "kiln.implementation-qualification.v1",
    programs = { coordinator = coordinator, b3sum = tools.b3sum },
  }
