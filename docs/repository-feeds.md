# Repository feeds

Kiln can publish read-only Atom feeds for its canonical repositories and let an
operator subscribe to external repository RSS/Atom feeds. Feeds are
observations, not CI evidence, acceptance, or release authority.

## Configure

Add a `feeds` object to the service profile:

```json
"feeds": {
  "token_file": "/home/operator/.config/kiln/web-service/credentials/feed.token",
  "max_subscriptions": 32,
  "max_entries_per_feed": 50,
  "max_feed_bytes": 1048576,
  "max_redirects": 3,
  "timeout_seconds": 10,
  "refresh_interval_ms": 900000,
  "allow_private_destinations": false
}
```

The token file is a 64-character hexadecimal credential, mode `0600` or
`0400`, distinct from the operator, worker, viewer and proxy credentials.
Omitting `feeds` disables both publication and external subscriptions. A feeds
policy is not admitted through `extend-projects`; restart with a reviewed
profile to change it.

## Publication

- `GET /feeds.xml?token=<feed-token>` lists canonical repositories.
- `GET /feeds/<repository-key>.xml?token=<feed-token>` lists retained runs for
  that repository, newest first.

The repository key is the existing BLAKE3 `repository_key` shown by the
projects API. The token is accepted only as the single `token` query
parameter on GET feed paths. It is never written to audit records, and feed
responses do not echo it. Operators can copy token-bearing URLs from the Feeds
page; viewers never receive them.

Feed entries contain the project label, outcome, retained run id, exact source
revision, and a console link. Text is XML-escaped.

## External subscriptions

Operators can add an external feed URL, refresh it, and remove it. When
`refresh_interval_ms` is non-zero, the service refreshes due subscriptions in
the background, oldest attempt first and at most two per tick. `0` keeps refresh
operator-only. Subscription URLs are omitted from viewer responses; entries and
errors remain readable. Add fetches
immediately; refresh replaces entries on success and retains previous entries
with a bounded error on failure. Viewers can read subscriptions but cannot
mutate them. Kiln does not claim continuous freshness; refresh is explicit.

## Network and parsing bounds

Public profiles accept HTTPS only, on the default port, without userinfo or
fragments. DNS is resolved once, every address must be globally routable, and
the connection is pinned to the admitted address. Redirects are manual,
bounded, and re-admitted. Proxy environment variables are ignored.

Refresh requests send stored `ETag` and `Last-Modified` validators. A `304 Not
Modified` retains previous entries, clears the error and updates the validators
without parsing a body. A `304` without a supplied validator is refused.

RSS 2.0 and Atom are parsed with `roxmltree`; DTD and entity declarations are
refused. Titles, summaries, dates, links, entries and response bytes are
bounded. Links are restricted to HTTP(S). The console renders feed text with
text nodes, never HTML.

`allow_private_destinations` is an explicit test/private-network exception. It
is not a hostile-network or production claim.

## Non-claims

Feeds do not prove repository identity, actor authority, build correctness,
remote visibility, freshness, or release readiness. Feed content is untrusted
text. The console page does not create CI state or acceptance evidence.
