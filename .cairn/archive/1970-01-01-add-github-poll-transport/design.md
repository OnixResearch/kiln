## Decisions

The transport uses the same fixed-origin, no-redirect, no-proxy and bounded-DNS
pattern as the existing GitHub status client. The API host is fixed; fixture
tests may use a loopback HTTP origin but production construction never accepts
an endpoint.

Pagination parses only the numeric `page` parameter from a bounded `Link`
header; it never follows the supplied URL. Pull-request records are filtered
before normalization because GitHub's issues endpoint includes them.

HTTP status classes remain distinct: 401/403 authorization, 404 repository,
429 unknown/rate-limited, other statuses infrastructure, and transport failures
infrastructure. Responses are read through a hard byte bound before JSON
decoding. No retry or schedule is introduced.

## Non-claims

A successful page is not admission, source authority or dispatch. The transport
does not authenticate webhook deliveries, deduplicate work, create attempts or
prove repository visibility beyond the configured token.
