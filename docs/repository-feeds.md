# Repository feeds

Kiln can publish read-only Atom feeds for its canonical repositories and let an
operator subscribe to external repository RSS/Atom feeds. Feeds are
observations, not CI evidence, acceptance, or release authority.

## Quick start

1. Create a dedicated feed token outside the repository:

   ```console
   kiln-web-service feed-token --path /etc/kiln-web-service/credentials/feed.token
   ```

2. Add the `feeds` object below to the service profile and restart the service.
   The token file must be mode `0600` or `0400` and distinct from all other
   credentials.

3. Verify the publication:

   ```console
   curl -sk "https://localhost:9443/feeds.xml?token=$(cat /etc/kiln-web-service/credentials/feed.token)"
   curl -sk "https://localhost:9443/feeds.opml?token=$(cat /etc/kiln-web-service/credentials/feed.token)"
   ```

4. Use the token-bearing Atom or RSS URL shown on the Feeds page in a feed
   reader, or import the OPML export to subscribe to every repository.

5. External feeds are added or imported on the same page. With a non-zero
   cadence they refresh automatically; otherwise use Refresh per feed.

## Configure

Add a `feeds` object to the service profile. The optional `feeds` field is also
accepted by `config/web-service.ncl` and its positive/negative fixtures:

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

Create the token with the service binary so it is fresh and private:

```console
kiln-web-service feed-token --path /etc/kiln-web-service/credentials/feed.token
```

The command refuses an existing path and writes one 64-character hexadecimal
credential with mode `0600`. The token is distinct from the operator, worker,
viewer and proxy credentials. Replacing the token file is observed on the next
feed request without restarting the service; an unreadable replacement keeps
the last valid token until the file is fixed.
Omitting `feeds` disables both publication and external subscriptions. A feeds
policy is not admitted through `extend-projects`; restart with a reviewed
profile to change it.

## Publication

- `GET /feeds.xml?token=<feed-token>` lists canonical repositories.
- `GET /feeds/<repository-key>.xml?token=<feed-token>` lists retained runs for
  that repository as Atom, newest first.
- `GET /feeds.rss?token=<feed-token>` and
  `GET /feeds/<repository-key>.rss?token=<feed-token>` return the same data as
  RSS 2.0.
- `GET /feeds.opml?token=<feed-token>` exports all repository feeds as an OPML
  2.0 subscription list.

The repository key is the existing BLAKE3 `repository_key` shown by the
projects API. The token is accepted as the single `token` query
parameter on GET feed paths, or as `Authorization: Bearer <feed-token>` when
the client supports request headers. It is never written to audit records, and feed
responses do not echo it. Operators can copy token-bearing URLs from the Feeds
page; viewers never receive them.

Feed entries contain the project label, outcome, retained run id, exact source
revision, and a console link. Text is XML-escaped. Responses carry an ETag and
`Cache-Control: no-cache`; a matching `If-None-Match` returns 304 without a body.

## External subscriptions

Operators can add an external feed URL, refresh it, rename its display label,
pause or resume background refresh, and remove it. Paused feeds retain their
entries and remain manually refreshable. When
`refresh_interval_ms` is non-zero, the service refreshes due subscriptions in
the background, oldest attempt first and at most two per tick. Consecutive
refresh failures double the retry interval from the base cadence up to one day;
a successful refresh or `304` resets the backoff. `0` keeps refresh
operator-only. Subscription URLs are omitted from viewer responses; entries and
errors remain readable. Add fetches
immediately and returns a bounded failure reason on error; refresh replaces
entries on success and retains previous entries with a bounded error on failure. Viewers can read subscriptions but cannot
mutate them. Kiln does not claim continuous freshness; refresh is explicit.

## Network and parsing bounds

Public profiles accept HTTPS only, on the default port, without userinfo or
fragments. DNS is resolved once, every address must be globally routable, and
the connection is pinned to the admitted address. Redirects are manual,
bounded, and re-admitted. Proxy environment variables are ignored.

Refresh requests send stored `ETag` and `Last-Modified` validators. A `304 Not
Modified` retains previous entries, clears the error and updates the validators
without parsing a body. A `304` without a supplied validator is refused.

RSS 2.0, RSS 1.0/RDF and Atom are parsed with `roxmltree`; DTD and entity
declarations are refused, and element nesting is bounded to 64 levels. Titles, summaries, dates, links, entries and response bytes are
bounded. Links are restricted to HTTP(S). The console renders feed text with
text nodes, never HTML.

`allow_private_destinations` is an explicit test/private-network exception. It
is not a hostile-network or production claim.

## Non-claims

Feeds do not prove repository identity, actor authority, build correctness,
remote visibility, freshness, or release readiness. Feed content is untrusted
text. The console page does not create CI state or acceptance evidence.
