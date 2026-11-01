## Context

The service already has a single-tenant HTTPS shell, durable SQLite state, a
read-only console, and bounded HTTP dependencies (`reqwest` blocking and
`roxmltree`).

## Decisions

### Publication

`/feeds.xml?token=...` lists canonical repositories. `/feeds/<repository-key>.xml`
returns retained runs for every project on that repository. The key is the
existing BLAKE3 `repository_key`. Feeds are Atom 1.0, bounded to the configured
entry limit, newest first, XML-escaped, and carry only run id, project label,
revision, outcome and console link. The dedicated feed token is distinct from
operator, worker and viewer credentials and is accepted only on GET feed paths.
Feed namespaces are only in links (`&amp;`), never in entry text.

### Subscriptions

Operators add a URL and optional label. The URL is admitted, resolved and
fetched before the engine lock is reacquired; the engine lock is never held
across network I/O. Fetch success replaces entries and clears the error; fetch
failure on refresh retains previous entries and records the bounded error.
Removal is immediate. Viewers can read subscriptions and entries but cannot
mutate them.

### Destination policy

Public destinations must be HTTPS on the default port, without userinfo,
fragments or control bytes. DNS is resolved once, every address must be
globally routable, and the connection is pinned to the admitted address.
Redirects are manual, bounded and re-admitted. Proxy environment variables are
ignored. `allow_private_destinations` is an explicit test/private-network
exception; public profiles keep it false.

### Parsing and retention

RSS 2.0 and Atom are parsed with `roxmltree`. DTD and entity declarations are
refused. Titles, summaries, dates and links are bounded and truncated at UTF-8
boundaries. Links are restricted to HTTP(S); console rendering uses text nodes
and no external scripts. Entry counts, feed bytes, redirects, timeout and
subscriptions use named profile bounds. The existing state byte bound remains
the final retention backstop.

## Risks

- DNS pinning and redirect re-admission prevent the common SSRF paths but do
  not prove hostile-network safety.
- Feed content is untrusted text; the UI never injects it as HTML.
- Refresh is explicit. Kiln does not claim continuous feed freshness.
