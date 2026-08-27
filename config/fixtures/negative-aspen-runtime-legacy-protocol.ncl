(import "../aspen-runtime-profile.ncl")
& {
  native_host.callback_envelope_schema | force = "molten.system-extension.native-callback-envelope.v1",
  native_host.alpn | force = "molten/system-extension/native/v1",
  native_host.framing | force = "preserves-packed-single-frame-v1",
}
