(import "../hosted-lattice-job.ncl") {
  web = { service_origin = "https://web.example.test", tls_ca_path = "/srv/kiln/web-ca.pem", token_file = "/srv/kiln/worker.token", http_timeout_ms = 1000, maximum_response_bytes = 1048576 },
  network = { key_file = "/srv/kiln/lattice-task.key", bind_address = "127.0.0.1:0", relay_urls = [] },
  remote = { id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", addrs = [{ Ip = "127.0.0.1:4000" }] },
  organization = "example",
  project_id = "example",
  repository_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  node_id = "build",
  policy_id = "lattice-example",
}
