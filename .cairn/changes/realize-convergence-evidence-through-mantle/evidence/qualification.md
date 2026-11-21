# Live preserve-route qualification

Run 2026-11-21 against the retained phase-1 pilot inputs, Mantle
`4b05f9f677c8236b28aaaeca2af902063610b252`, executable
`/nix/store/ahqnikfq18p6ycp4krx58cqgr5inm2sd-mantle-0.1.0/bin/mantle`
(BLAKE3 `9c2225afe460f7d45854072ca05d8451507385070f36a66602edf542b51cc87f`),
through the adapter's public API only.

## Commands and exit codes

| Command | Exit |
| --- | --- |
| `python3 -m http.server 18471 --bind 127.0.0.1 --directory /datapool/mantle-store/convergence-pilot-20261119/cache` | served for the run, stopped after |
| `mkdir -p /datapool/mantle-store/kiln-phase2b-20261121/{store,state}` | 0 |
| `nix --option min-free 0 --option builders '' develop -c env RUSTC_WRAPPER= CARGO_TARGET_DIR=target cargo test -p kiln-adapter-mantle --test live_realize_pilot -- --ignored --nocapture` (no deadline) | 0 (392.43 s) |
| `… cargo test -p kiln-adapter-mantle --test live_realize_pilot full_pilot_receipt_digest_matches_producer -- --ignored --exact --nocapture` | 0 |

Fresh physical store and state: `/datapool/mantle-store/kiln-phase2b-20261121/`.
The generated receipt is retained compressed as `realization-receipt.json.bz2`
(full-file BLAKE3 in `realization-receipt.b3`). The adapter's domain-separated
receipt identity over those full bytes is
`b64221ba8f3e19280b38e8162f517d1c4770595ba145818f2bd74734049a9f11`.

## Comparison with the pilot receipt

| Fact | Generated | Pilot | Equal |
| --- | --- | --- | --- |
| `selected_root_paths` | `/nix/store/zjq6whhf2c641vkz89kgwwpbajjq5cbq-octet-mantle-aeneas-consumer-job-v1` | same | yes |
| `plan_blake3` | `edd387e529e1d4b277d726edc93901889413dc0dc393c7c7fd715981afb08a78` | same | yes |
| `import_receipt_blake3` | `60a452562c2d845ec379bef934a0d03b97004e8959b2c409bb4804f9a2558c39` | same | yes |
| `source_bundle_manifest_blake3` | `8ed1103b5de3054ee13ea391af805e276e3cc3b6ceaa50b2149193adb1ff1777` | same | yes |
| `cache_closure_policy_blake3` | `0d228941b966d100140a23dbd865f39a4d54579662747b42ec41f718832261a7` | same | yes |
| `status` / `strongest_state` | `complete` / `realized` | same | yes |
| `receipt_blake3` | `4c7b60d7eb928e68f43d185df7c1b9eb589d10bcbab57afc817988030432af1f` | `e96d2864…` | no (new run receipt) |

The adapter admitted one success observation with product `out`, cache kind
`substitution`, receipt identity `b64221ba…`, builder identity equal to the
pinned executable, and `plan_identity` equal to `plan_blake3`.

## Full-receipt digest oracle

`full-receipt-oracle.txt` shows the adapter's producer-digest recomputation on
the full retained pilot receipt:
`e96d2864c5e2b5454fcb0fdf924e9d1e4768b896b4b2e4ad395d32acd3827a8d`, equal to
Mantle's own `receipt_blake3`.
