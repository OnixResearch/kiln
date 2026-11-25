let base = import "runbook-registry-inputs.ncl" in
(import "../runbook-registry.ncl") (
  base
  & {
    notifications = [
      {
        id = "summary-hook",
        enabled = true,
        runbooks = ["security-audit"],
        on_success = true,
        on_failure = true,
        on_unknown = true,
        host = "hooks.example.invalid",
        port = 443,
        path = "kiln/summary",
        addresses = ["203.0.113.10"],
        bearer_secret_file = null,
        ca_certificate_file = null,
        allow_loopback = false,
        allow_private = false,
        timeout_ms = 5000,
        max_request_bytes = 65536,
        max_response_bytes = 65536,
        max_resolver_candidates = 4,
      },
    ],
  }
)
