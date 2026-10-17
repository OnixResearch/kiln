(import "../iroh-workflow.ncl") (
  (import "./iroh-workflow-inputs.ncl")
  & {
    transport_limits.maximum_connections | force = 65,
  }
)
