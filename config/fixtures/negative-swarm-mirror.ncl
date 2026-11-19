(import "../swarm-mirror.ncl")
& {
  executable = "/nix/store/iroh-news/bin/iroh-news",
  state_dir = "/var/lib/kiln/swarm",
  from = "kiln-aspen1 <kiln-aspen1@swarm.local>",
  timeout_ms = -1,
}
