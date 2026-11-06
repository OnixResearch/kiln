let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (
  positive
  & {
    lattice_execution = [
      {
        id = "lattice-example",
        peer = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        workflow_policy_id = "web-workflow",
        node_id = "build",
        operation_identity = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        web_service = {
          service_origin = "https://web.example.test",
          tls_ca_path = "/srv/kiln/web-ca.pem",
          token_file = "/srv/kiln/web-worker.token",
          http_timeout_ms = 10000,
          maximum_response_bytes = 1048576,
        },
        organization = "example",
        web_project = "example",
        web_repository = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        repository_path = "/srv/kiln/source",
        source_limits = { max_entries = 1024, max_path_bytes = 1024, max_file_bytes = 1048576, max_total_bytes = 16777216 },
        hosted_project = "example",
        pipeline_id = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        price_id = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        class = "linux",
      }
    ]
  }
)
