# Aspen callback producer fixture

`producer-callback-request.preserves` is emitted by the independent `kiln-aspen-extension` producer example.

`kiln-adapter-aspen` consumes the static frame and checks `consumer-expected.json` without invoking the producer.

The fixture binds Aspen revision `22f8ded26ca1907c29948e08b53f35df23080733`. Regenerate it with:

```console
nix develop path:$PWD -c cargo run -q -p kiln-aspen-extension --example emitcallbackfixture > \
  fixtures/aspen/runtime/22f8ded26ca1907c29948e08b53f35df23080733/producer-callback-request.preserves
```

This fixture proves independent producer and consumer agreement only. It does not prove Aspen deployment, provider success, or release eligibility.
