# Test-only TLS material

The certificate, chain, and private key in this directory are committed test
fixtures for the HTTP observation shell. They are generated for the test-only
CA and `localhost` SAN and must not be used for a deployed service.

- `ca-cert.pem` — test CA trusted by the Rust integration test client.
- `cert.pem` — test server leaf certificate.
- `chain.pem` — test server leaf followed by the test CA.
- `key.pem` — test server private key.

Regenerate the cohort with OpenSSL if the test validity period or SAN changes.
