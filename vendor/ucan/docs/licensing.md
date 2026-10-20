# UCAN licensing boundary

| Package or surface | License |
|---|---|
| `crates/ucan-core/**` | `MPL-2.0` |
| `crates/ucan-nostr/**` | `AGPL-3.0-or-later` |
| Root `ucan` std shell | `AGPL-3.0-or-later` |
| `vendor/verified-logic/**` | `MPL-2.0`, synchronized from the committed Trellis revision in `vendor/verified-logic-revision.txt` |
| Repository documentation and lifecycle material not carrying another notice | `AGPL-3.0-or-later` |
| Other dependencies and generated material containing upstream code | Their stated upstream terms |

The no-std core must not depend on the std shell. The shell may depend on the core and the verified primitive snapshot. The optional `peer-auth` feature uses `blake3`, `hmac`, `sha2`, and `zeroize` under their upstream terms. It copies no Celld code. The snapshot must be refreshed with `scripts/sync-verified-logic.sh`, not manually reformatted or edited.

Complete texts are in `LICENSES/MPL-2.0.txt` and `LICENSES/AGPL-3.0-or-later.txt`. The core, Nostr adapter, and synchronized snapshot also carry their applicable text, so package archives are self-contained.

Earlier grants remain valid. Trellis source/proof alignment does not certify UCAN parsing, cryptography, resolver truth, revocation freshness, replay storage, policy semantics, interoperability, or shell correctness. This map does not establish contributor authority or legal compliance.
