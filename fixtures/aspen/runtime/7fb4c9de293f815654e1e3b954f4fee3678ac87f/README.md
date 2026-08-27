# Aspen runtime callback fixture

`producer-callback-request.preserves` is emitted by the independent `kiln-aspen-extension` producer example.

`kiln-adapter-aspen` consumes the static frame and checks `consumer-expected.json` without invoking the producer.

The fixture binds Aspen revision `7fb4c9de293f815654e1e3b954f4fee3678ac87f`. Regenerate it with:

```console
nix develop path:$PWD -c cargo run -q -p kiln-aspen-extension --example emitcallbackfixture > \
  fixtures/aspen/runtime/7fb4c9de293f815654e1e3b954f4fee3678ac87f/producer-callback-request.preserves
```
