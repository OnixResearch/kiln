# Design: Backend failure transcripts

## Boundary
The verified primitive consumes finite caller/shell-supplied backend operation facts. Each operation records its kind and whether the backend returned a successful answer. Transcript acceptance is exactly conjunction of all operation success flags. The first failed operation is reported in operation order.

## Integration
The UCAN shell wraps each backend call site (`KeyResolver::resolve_key`, `ProofStore::resolve_proof`, `RevocationChecker::is_revoked`) with a transcript check over that single operation. On success the operation transcript accepts and normal control flow continues. On backend or missing-data errors, the transcript rejects and the existing typed error is returned.

## Trust Boundary
The transcript verifies only fail-closed admission over supplied success/failure facts. It does not verify that returned keys, proof tokens, or revocation answers are correct or fresh.
