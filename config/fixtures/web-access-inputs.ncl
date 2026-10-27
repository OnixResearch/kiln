{
  viewers = [{ id = "reader", label = "Read-only operator", token_file = "/private/reader.token" }],
  proxy = {
    listen_address = "127.0.0.1:8099",
    credential_file = "/private/proxy.token",
    tailscaled_socket_path = "/run/tailscale/tailscaled.sock",
    principals = [
      {
        user_id = "47256370424925063",
        login_name = "operator@example.com",
        label = "Registered operator",
        role = "operator",
      }
    ],
  },
}
