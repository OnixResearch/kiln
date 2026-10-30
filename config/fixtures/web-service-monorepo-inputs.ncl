# Isolated conformance inputs only: two scoped projects and one unscoped project share one repository.
let input = import "web-service-inputs.ncl" in
let project = std.array.first input.projects in
input
& {
  projects
    | force
    = [
      project & { id | force = "crate-a", label | force = "Crate A", scope = { root = "crates/a", graphs = ["cargo"] } },
      project & { id | force = "crate-b", label | force = "Crate B", scope = { root = "crates/b", depends_on = ["Cargo.lock"], graphs = ["cargo"] } },
      project & { id | force = "whole", label | force = "Whole repository" },
    ],
}
