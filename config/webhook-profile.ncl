let Contracts = import "contracts.ncl" in
let HttpsPort = 443 in
let MaxRequestBytes = 16384 in
let MaxResponseBytes = 65536 in
let MaxResolverCandidates = 16 in
let DeadlineTicks = 30 in
let RetryLimit = 3 in
({
  id = "incident-webhook-v1",
  endpoint = {
    scheme = "https",
    host = "notifications.example.invalid",
    port = HttpsPort,
    path = "/kiln/events",
    method = "POST",
    content_type = "application/json",
    dynamic_destination = false,
    allow_redirects = false,
    allow_private_ranges = false,
  },
  auth = {
    kind = "blake3_signature",
    secret_reference = "secrets/webhook-signing-key",
    inline_secret = "",
    signature_algorithm = "BLAKE3-keyed-v1",
  },
  idempotent_receiver = false,
  reconciliation_supported = false,
  bounds = {
    max_request_bytes = MaxRequestBytes,
    max_response_bytes = MaxResponseBytes,
    max_resolver_candidates = MaxResolverCandidates,
    deadline_ticks = DeadlineTicks,
    retry_limit = RetryLimit,
  },
  non_claims = [
    "delivery acceptance does not prove human receipt",
    "endpoint transport does not change canonical CI outcomes",
    "TLS and DNS observations do not prove endpoint trust",
  ],
}) | Contracts.WebhookProfile
