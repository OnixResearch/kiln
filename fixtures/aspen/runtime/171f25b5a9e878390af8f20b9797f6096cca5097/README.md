# Aspen runtime callback fixture

`producer-callback-request.preserves` is emitted by the independent `kiln-aspen-extension` producer example.

`kiln-adapter-aspen` consumes the static frame and checks `consumer-expected.json` without invoking the producer.

The fixture binds Aspen revision `171f25b5a9e878390af8f20b9797f6096cca5097`. Regenerate it with:

```console
nix develop path:$PWD -c cargo run -q -p kiln-aspen-extension --example emitcallbackfixture > \
  fixtures/aspen/runtime/171f25b5a9e878390af8f20b9797f6096cca5097/producer-callback-request.preserves
```
