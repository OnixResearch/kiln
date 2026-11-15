# Context

The web service already exposes a bounded GET-only read surface through
`Engine::api`. The pilot describes that existing boundary rather than replacing
it. Tests call the real engine shell and assert status codes, error bodies,
authentication distinctions, pagination bounds and field presence without
serializing the same Rust type on both sides.

## Decision

- Contract version is `kiln.http-api.v1` and is embedded in the OpenAPI
  `info.version`.
- Family is GET-only: `/api/projects`, `/api/runs`, `/api/runs/{id}`,
  `/api/runs/{id}/execution-binding`, `/api/runs/{id}/artifacts`.
- Authentication failures return 403 with `execution_binding_scope`,
  `worker_scope` or `worker_only`; admission failures stay distinct from
  transport and schema mismatch.
- Unknown outcomes and lost observation remain explicit; 2xx or successful
  deserialization is never execution/CI evidence.
- The in-process lockstep client fixture bounds received bytes before parsing,
  performs no retries and classifies authentication, authorization, not found,
  transport, schema mismatch and unknown statuses separately. Request deadlines
  belong to a networked client and are not exercised by this fixture.
- Lockstep upgrade for this family; tested window is current server plus
  current client only. No promise of every prior version.
- Drift and Progenitor were evaluated in a scratch directory at pinned
  revisions; no dependency was added to the workspace. The no-go decision is
  recorded in `docs/http-api-contract.md`.

## Verification

Cairn proposal/design/tasks gates, focused Rust tests run by parent, doc review.

## Non-claims

This contract is not authorization, not CI evidence, not a server migration,
and not a promise of backward compatibility beyond the tested window.
