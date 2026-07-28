# Cairn policy provenance

The local lifecycle policy source was initialized from Choregraph revision
`f960b9bd4c75ff599f5a84ca0df01e159430cb94`. That source records Cairn
revision `70db8fe114910feb5d162fea5db8ad4ffdb6f176` as its policy baseline.
Kiln generated its current runtime policy with Cairn revision
`e8218114994dbc4930f798ff009bc5578610f7e1`.

`contracts.ncl` and `default.ncl` are authoritative. The policy permits only
repository-owned source and evidence roots. It rejects generated outputs, build
outputs, and vendored source as traceability evidence.

Refresh the runtime policy with:

```text
nix run path:$CAIRN_SOURCE#cairn -- policy export --output cairn-policy/generated/cairn-policy.json
```

After each refresh, run policy freshness, lifecycle validation, gates, and
traceability coverage.
