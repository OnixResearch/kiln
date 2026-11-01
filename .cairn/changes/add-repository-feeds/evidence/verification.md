# Verification evidence

## Result

Kiln exposes read-only Atom feeds for canonical repositories and operator-managed
external RSS/Atom subscriptions. Feeds are optional and disabled unless the
service profile declares a policy and a dedicated 64-hex feed token.

## Publication

`/feeds.xml?token=...` lists canonical repositories. Each
`/feeds/<repository-key>.xml?token=...` contains retained runs for projects on
that repository, newest first, bounded by the configured entry limit. Text is
XML-escaped; links use the HTTPS origin. The token is accepted only on GET feed
paths, never audited or echoed. Viewers receive no token-bearing URLs.

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

## Evidence

- 10 feed unit tests pass: RSS/Atom, DTD rejection, URL/private-address
  admission, body bound, one-hop redirect, UTC formatting, Atom escaping and
  repository run filtering/order/limit.
- 144 web-service library tests and 69 engine tests pass.
- The workspace passes 1224 tests.
- The runtime round-trip starts the actual service, publishes an Atom feed,
  rejects wrong/missing tokens, adds an external feed, retains entries after a
  failed refresh, and removes the subscription.
- A disposable browser session signed in, rendered 15 repository cards, added
  an external feed, displayed its retained entry with no console errors, and
  was removed with its temporary profile after verification.
- Clippy, rustfmt and Cairn validation pass.

## Non-claims

Feeds are observations, not CI success, acceptance, release evidence, freshness
or hostile-network isolation. External feed text is untrusted. The private
destination exception is not a production claim. No deployment was performed.
