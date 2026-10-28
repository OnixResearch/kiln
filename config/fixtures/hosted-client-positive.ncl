# Authoring-contract fixture only; does not supply a real key, route or membership.
let identity = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" in
(import "../hosted-client.ncl") {
  network = {
    key_file = "/private/client.key",
    bind_address = "127.0.0.1:0",
    relay_urls = [],
  },
  remote = {
    id = identity,
    addrs = [{ Ip = "127.0.0.1:44771" }, { Relay = "https://relay.example" }],
  },
  organization = "acme",
  listen_address = "127.0.0.1:44881",
  webhook = { project = "app", connection = "origin", generation = 1 },
}
