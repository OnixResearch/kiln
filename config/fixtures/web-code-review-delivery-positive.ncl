(import "../web-code-review.ncl") (
  (import "web-code-review-inputs.ncl")
  & {
    delivery | force = {
      schema = "kiln.review-delivery-profile.v1",
      destinations = [
        {
          id = "review",
          project_id = "fixture",
          enabled = true,
          on_success = false,
          on_failure = false,
          host = "localhost",
          port = 9444,
          path = "/review",
          addresses = ["127.0.0.1"],
          bearer_secret_file = null,
          ca_certificate_file = null,
          allow_loopback = true,
          allow_private = false,
          timeout_ms = 1000,
          max_request_bytes = 1024,
          max_response_bytes = 1024,
          max_resolver_candidates = 1,
        }
      ],
      recipients = [
        { principal = "reviewer", destination = "review", address = "reviewer@example.test", channel = "webhook" },
      ],
      max_records = 100,
      max_attempts = 2,
      retry_delay_ms = 1000,
      max_tick_effects = 2,
    },
  }
)
