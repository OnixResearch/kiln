## Phase 1: Semantic profile

- [ ] [serial] Pin OpenTelemetry data-model, OTLP, CI/CD semantic-convention, license, and stability cohorts. r[kiln.otel.profile] r[kiln.otel.nonclaims]
- [ ] [serial] Select initial span, metric, log, resource, attribute, link, and redaction mappings. r[kiln.otel.profile]
- [ ] [depends:otel-mapping-registry] Add typed Nickel schema, cardinality, sampling, redaction, transport, and bound policy. r[kiln.otel.profile]

## Phase 2: Pure projection

- [ ] [serial] Implement pure span, metric, log, resource, identity-link, and timestamp-fact projection. r[kiln.otel.projection]
- [ ] [depends:otel-projection] Implement pure cardinality, redaction, batching, loss, and authority decisions. r[kiln.otel.safety]
- [ ] [parallel] Add stable mapping and negative drift, secret, bound, loss, and promotion tests. r[kiln.otel.conformance]

## Phase 3: Adapter shell

- [ ] [depends:otel-profile] Add `kiln-adapter-opentelemetry` outside core dependency graphs. r[kiln.otel.profile] r[kiln.otel.nonclaims]
- [ ] [depends:otel-safety-core] Implement optional bounded OTLP encoding, batching, transport, retry, and observation handling. r[kiln.otel.export]
- [ ] [parallel] Add backend-unavailable and runtime independence tests. r[kiln.otel.nonclaims]

## Phase 4: Evidence

- [ ] [serial] Add canonical signal fixtures and an independent consumer. r[kiln.otel.conformance]
- [ ] [serial] Document mappings, cardinality, redaction, loss, transport, and non-claims. r[kiln.otel.profile] r[kiln.otel.nonclaims]
- [ ] [serial] Run formatting, tests, Clippy, `no_std`, Nickel, dependency, Cairn, Tracey, and Nix gates. r[kiln.otel.conformance]
- [ ] [serial] Record evidence, sync specs, archive the change, and rerun validation. r[kiln.otel.conformance] r[kiln.otel.nonclaims]
