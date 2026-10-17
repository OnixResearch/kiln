# Synthetic loopback-only conformance settings, not deployment configuration.
let native = import "../../fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8/producer-fixtures-v1.json" in
let FrameBytes = 65536 in
let LogBytes = 16384 in
let Connections = 4 in
let DeadlineMilliseconds = 5000 in
{
  schema = "kiln.iroh-workflow-profile.v1",
  iroh_version = "0.95.1",
  alpn = "kiln.workflow.v1",
  socket_path = "/tmp/kiln-iroh-fixture/lattice.sock",
  bind_address = "127.0.0.1:0",
  # RFC 8032 test-vector public identity; no private identity is distributed.
  allowed_peers = ["d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"],
  allowed_user_ids = [1000],
  relay_urls = [],
  transport_limits = {
    maximum_frame_bytes = FrameBytes,
    maximum_log_bytes = LogBytes,
    maximum_connections = Connections,
    read_deadline_milliseconds = DeadlineMilliseconds,
    write_deadline_milliseconds = DeadlineMilliseconds,
  },
  contract = native.profile,
  exchange_timeout_ms = DeadlineMilliseconds,
}
