## ADDED Requirements

### Requirement: Compressed, ETag-cached static console assets and Node module test coverage

r[kiln.web.static_asset_caching_and_compression]
`crates/kiln-web-service/src/server.rs` MUST serve static console assets with deterministic content-derived `ETag` headers, `304 Not Modified` handling for matching `If-None-Match` requests, bounded `Cache-Control` headers, and negotiated `gzip` or `zstd` compression with `Vary: Accept-Encoding`. The repository MUST provide Node module tests loading `web.js`, `fleet.js`, `work.js`, and `project.js` and record them in `docs/console-port-inventory.json`.

#### Scenario: Repeat console load hits 304 and cold load uses negotiated compression

r[kiln.web.static_asset_caching_and_compression.scenario.http_caching]
- GIVEN a browser requesting static JS/CSS console assets with `Accept-Encoding: gzip`
- WHEN the asset is fetched cold and then re-requested with its returned `ETag` in `If-None-Match`
- THEN the cold response MUST return compressed bytes with `ETag` and `Vary: Accept-Encoding`
- AND the conditional repeat request MUST return `304 Not Modified` with an empty body
