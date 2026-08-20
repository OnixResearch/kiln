let Contracts = import "../contracts.ncl" in
({
  id = "bad-webhook",
  endpoint = {
    scheme = "https",
    host = "example.invalid",
    port = 1,
    path = "/",
    method = "POST",
    content_type = "application/json",
    dynamic_destination = false,
    allow_redirects = false,
    allow_private_ranges = false,
  },
  auth = {
    kind = "bearer",
    secret_reference = "secret/ref",
    inline_secret = "plaintext-secret",
    signature_algorithm = "none",
  },
  idempotent_receiver = false,
  reconciliation_supported = false,
  bounds = {
    max_request_bytes = 1,
    max_response_bytes = 1,
    max_resolver_candidates = 1,
    deadline_ticks = 1,
    retry_limit = 1,
  },
  non_claims = ["invalid inline secret"],
}) | Contracts.WebhookProfile
