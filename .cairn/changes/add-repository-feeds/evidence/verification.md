# Verification evidence

## Result

Kiln exposes read-only Atom feeds for canonical repositories and operator-managed
external RSS/Atom subscriptions. Feeds are optional and disabled unless the
service profile declares a policy and a dedicated 64-hex feed token.

## Publication

`/feeds.xml?token=...` lists canonical repositories. Each
`/feeds/<repository-key>.xml?token=...` contains retained runs for projects on
that repository, newest first, bounded by the configured entry limit. Text is
XML-escaped; links use the HTTPS origin. The token is accepted on GET feed paths as the
query value or an `Authorization: Bearer` header, never audited or echoed. Viewers receive no token-bearing URLs.

## OPML export

`GET /feeds.opml?token=<feed-token>` returns OPML 2.0 with one outline per
canonical repository. Labels and URLs are XML-escaped; URLs carry the feed
token. Operators receive an Export OPML link, while viewer responses contain no
token-bearing OPML URL.

## Subscriptions

Operators can add, refresh and remove subscriptions; viewers are read-only.
Add fetches before success. Refresh failure retains previous entries and records
a bounded error. The engine lock is not held across network I/O. Durable state
is preflighted against the state byte bound before mutation.

## Network and parsing

Public destinations are HTTPS-only on the default port, without userinfo,
fragments or control bytes. DNS is resolved once, every address must be globally
routable, and the connection is pinned to the admitted address. Redirects are
manual, bounded and re-admitted. Proxy environment variables are ignored, and
`allow_private_destinations` is an explicit test/private exception.

RSS 2.0 and Atom are parsed with `roxmltree`; DTD/entity declarations are
refused. Titles, summaries, dates, links, entries, response bytes, redirects and
timeouts are bounded. Links are restricted to HTTP(S), and the console renders
text nodes rather than HTML.

## Conditional refresh

Refresh requests send bounded stored `ETag` and `Last-Modified` validators. A
`304 Not Modified` retains entries and title, clears the error, updates
validators and the attempt time, and requires no body. A `304` without a
supplied validator is refused. Redirects clear validators because they belong
to the issuing origin.

## Feed token creation

`kiln-web-service feed-token --path <absolute-new-path>` generates 32 CSPRNG
bytes as 64 lowercase hex characters, creates the file exclusively with mode
`0600`, and refuses an existing or relative path. The engine test reads back the
token, checks the mode and confirms overwrite refusal.

## Nickel policy

`config/web-service.ncl` accepts the optional feeds policy with matching bounds.
A positive fixture exports the feeds object, and a negative fixture is refused
for a cadence below the 60-second minimum. The repository Nickel check exercises
the export and format gates.

## Evidence

- 18 feed unit tests pass: RSS 2.0/RSS 1.0/Atom parsing with entry deduplication and RSS publication, DTD rejection, URL/private-address
  admission, body bound, one-hop redirect, UTC formatting, Atom escaping and
  repository run filtering/order/limit, conditional 304, OPML escaping, due selection and cadence monotonicity.
- 153 web-service library tests and 71 engine tests pass. Background refresh uses a dedicated scoped thread and a configurable cadence; due selection is oldest-first and bounded to two per tick.
- The workspace passed 1229 tests before the OPML increment. The post-increment
  full workspace command was blocked by another workspace holding the shared
  Cargo target lock; 148 library and 71 engine tests passed and Clippy was clean. The Feeds page reloads its observations every
  60 seconds while open and stops on navigation or logout.
- The runtime round-trip starts the actual service, publishes an Atom feed,
  rejects wrong/missing tokens, adds an external feed, retains entries after a
  failed refresh, and removes the subscription.
- A disposable browser session signed in, rendered 15 compact repository cards
  in an auto-fill grid, added an external feed, displayed its retained entry
  with no console errors, and was removed with its temporary profile after
  verification.
- Clippy, rustfmt and Cairn validation pass.

## Non-claims

Feeds are observations, not CI success, acceptance, release evidence, freshness
or hostile-network isolation. External feed text is untrusted. The private
destination exception is not a production claim. No deployment was performed.
