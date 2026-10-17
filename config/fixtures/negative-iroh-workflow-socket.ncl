(import "../iroh-workflow.ncl") (
  (import "./iroh-workflow-inputs.ncl")
  & {
    socket_path | force = "/tmp/../lattice.sock",
  }
)
