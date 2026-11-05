# Negative fixture: the maintained Lattice handler cannot execute pause.
let build = import "../web-service.ncl" in
let inputs = import "web-service-inputs.ncl" in
build (
  inputs
  & {
    projects | force =
      std.array.map
        (fun project =>
          project & { workflow = { controls = { pause | force = true } } }
        )
        inputs.projects,
  }
)
