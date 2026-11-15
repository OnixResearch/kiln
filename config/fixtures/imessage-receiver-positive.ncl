let ReceiverProfile = import "../imessage-receiver.ncl" in
({
  receiver_id = "openbubbles-imessage-ci-v1",
  payload_schema = "kiln.webhook-notification.v1",
  kiln_identity_algorithm = "BLAKE3",
  destination = {
    id = "openbubbles-imessage-ci-v1",
    endpoint = {
      scheme = "https",
      host = "imessage-receiver.example.invalid",
      port = 443,
      path = "/kiln/imessage",
      method = "POST",
      content_type = "application/json",
      dynamic_destination = false,
      allow_redirects = false,
      allow_private_ranges = false,
    },
    auth = {
      kind = "blake3_signature",
      secret_reference = "secrets/imessage-receiver-signing",
      inline_secret = "",
      signature_algorithm = "BLAKE3-keyed-v1",
    },
    idempotent_receiver = false,
    reconciliation_supported = false,
    bounds = {
      max_request_bytes = 16384,
      max_response_bytes = 65536,
      max_resolver_candidates = 16,
      deadline_ticks = 30,
      retry_limit = 3,
    },
    non_claims = [
      "receiver delivery does not prove human receipt",
      "receiver transport does not change canonical CI outcomes",
    ],
  },
  recipient_handles = ["tel:+15551234567"],
  sender_handle = "mailto:sender@example.invalid",
  message_fields = ["run_id", "outcome", "run_url"],
  run_url_base = "https://ci.example.invalid/runs/",
  bounds = {
    max_message_bytes = 4096,
    max_run_url_bytes = 512,
  },
  non_claims = [
    "iMessage delivery is not a CI gate result",
    "receiver does not create or activate Apple accounts",
  ],
}
) | ReceiverProfile
