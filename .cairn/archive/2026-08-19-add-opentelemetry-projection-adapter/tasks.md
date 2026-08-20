## Phase 1: Semantic profile

- [x] [serial] Pin OpenTelemetry data-model, OTLP, CI/CD semantic-convention, license, and stability cohorts. r[kiln.otel.profile] r[kiln.otel.nonclaims]
- [x] [serial] Select initial span, metric, log, resource, attribute, link, and redaction mappings. r[kiln.otel.profile]
- [x] [depends:otel-mapping-registry] Add typed Nickel schema, cardinality, sampling, redaction, transport, and bound policy. r[kiln.otel.profile]

## Phase 2: Pure projection

- [x] [serial] Implement pure span, metric, log, resource, identity-link, and timestamp-fact projection. r[kiln.otel.projection]
- [x] [depends:otel-projection] Implement pure cardinality, redaction, batching, loss, and authority decisions. r[kiln.otel.safety]
- [x] [parallel] Add stable mapping and negative drift, secret, bound, loss, and promotion tests. r[kiln.otel.conformance]

## Phase 3: Adapter shell

- [x] [depends:otel-profile] Add `kiln-adapter-opentelemetry` outside core dependency graphs. r[kiln.otel.profile] r[kiln.otel.nonclaims]
- [x] [depends:otel-safety-core] Implement optional bounded OTLP encoding, batching, transport, retry, and observation handling. r[kiln.otel.export]
- [x] [parallel] Add backend-unavailable and runtime independence tests. r[kiln.otel.nonclaims]

## Phase 4: Evidence

- [x] [serial] Add canonical signal fixtures and an independent consumer. r[kiln.otel.conformance]
- [x] [serial] Document mappings, cardinality, redaction, loss, transport, and non-claims. r[kiln.otel.profile] r[kiln.otel.nonclaims]
- [x] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.otel.conformance]
- [x] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.otel.conformance] r[kiln.otel.nonclaims]
