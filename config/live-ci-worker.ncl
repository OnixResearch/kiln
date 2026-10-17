# Apply beside the existing iroh-workflow.ncl builder (copied by live-ci-worker.py).
# Native contract and transport bounds come unchanged from the reviewed service.
let build = import "iroh-workflow.ncl" in
fun inputs => build {
  schema = "kiln.iroh-workflow-profile.v1",
  iroh_version = "0.95.1",
  alpn = "kiln.workflow.v1",
  socket_path = inputs.root ++ "/lattice.sock",
  bind_address = inputs.bind,
  allowed_peers = [inputs.controller_id],
  allowed_user_ids = [inputs.uid],
  relay_urls = [],
  transport_limits = inputs.native.transport.limits,
  contract = inputs.native.contract,
  exchange_timeout_ms = inputs.exchange_timeout_ms,
}
