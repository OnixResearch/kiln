let Contracts = import "contracts.ncl" in

let NonEmptyString =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let MaxHandleBytes = 256 in
let MaxRecipientHandles = 20 in
let MaxMessageBytes = 65536 in
let MaxRunUrlBytes = 2048 in
let AllowedMessageFields = ["run_id", "outcome", "run_url"] in
let has_prefix = fun prefix value =>
  std.string.length value >= std.string.length prefix
  && std.string.substring 0 (std.string.length prefix) value == prefix
in
let is_apple_handle = fun value =>
  std.is_string value
  && std.string.length value <= MaxHandleBytes
  && (
    (has_prefix "tel:" value && std.string.length value > 4)
    || (has_prefix "mailto:" value && std.string.length value > 7)
  )
in
let KilnWebhookSchema =
  std.contract.from_predicate (fun value =>
    value == "kiln.webhook-notification.v1"
  )
in
let Blake3IdentityAlgorithm =
  std.contract.from_predicate (fun value =>
    value == "BLAKE3"
  )
in
let AppleHandle = std.contract.from_predicate is_apple_handle in
let RecipientHandles =
  std.contract.from_predicate (fun value =>
    std.is_array value
    && std.array.length value > 0
    && std.array.length value <= MaxRecipientHandles
    && std.array.all is_apple_handle value
  )
in
let MessageFields =
  std.contract.from_predicate (fun value =>
    std.is_array value
    && std.array.length value > 0
    && std.array.all (fun item => std.array.elem item AllowedMessageFields) value
    && std.array.all
      (fun item => std.array.length (std.array.filter (fun other => other == item) value) == 1)
      value
  )
in
let RunUrlBase =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= MaxRunUrlBytes
    && has_prefix "https://" value
    && std.string.length value > 8
  )
in
let BoundedBytes = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0 && value <= maximum
  )
in

{
  receiver_id | NonEmptyString,
  payload_schema | KilnWebhookSchema,
  kiln_identity_algorithm | Blake3IdentityAlgorithm,
  destination | Contracts.WebhookProfile,
  recipient_handles | RecipientHandles,
  sender_handle | AppleHandle,
  message_fields | MessageFields,
  run_url_base | RunUrlBase,
  bounds
    | {
      max_message_bytes | BoundedBytes MaxMessageBytes,
      max_run_url_bytes | BoundedBytes MaxRunUrlBytes,
    },
  non_claims | Array NonEmptyString,
}
