# Repository Feeds Specification

## Purpose

Defines bounded Atom publication and external RSS/Atom subscriptions for the
single-tenant Kiln console.

## Requirements

### Requirement: Repository Atom publication

r[kiln.web.feeds.publish] Kiln MUST publish `/feeds.xml` and one
`/feeds/<repository-key>.xml` Atom 1.0 document per canonical repository. The
repository feed MUST contain only retained runs whose project belongs to that
repository, newest first and bounded by the configured entry limit. Entry text
MUST be XML-escaped and links MUST use the configured HTTPS origin. A missing or
foreign repository key MUST return no feed. The feed endpoint MUST be GET-only
and MUST NOT mutate state.

#### Scenario: Repository feed is rendered
r[kiln.web.feeds.publish.scenario.rendered]
- GIVEN retained runs for a canonical repository and a valid feed token
- WHEN the repository feed is requested
- THEN it MUST list those runs newest first with escaped text and console links, and no run from another repository.

#### Scenario: Unknown repository is absent
r[kiln.web.feeds.publish.scenario.missing]
- GIVEN an unknown or malformed repository key
- WHEN a feed is requested
- THEN it MUST return no feed.

### Requirement: Dedicated feed token

r[kiln.web.feeds.token] The feed token MUST be a 64-hex credential read from the
feeds policy, distinct from operator, worker, viewer and proxy credentials. It
MUST be accepted only as the single `token` query parameter on GET feed paths.
A missing, malformed or wrong token MUST be denied. Successful and denied feed
requests MUST NOT write the token to audit records, logs or responses.

#### Scenario: Token is required
r[kiln.web.feeds.token.scenario.required]
- GIVEN a feed request without a token or with another credential
- WHEN the feed route runs
- THEN it MUST be denied without state mutation or token disclosure.

#### Scenario: Operator can copy the token URL
r[kiln.web.feeds.token.scenario.operator]
- GIVEN an operator session and a configured feed token
- WHEN the feeds API is read
- THEN it MAY return token-bearing subscription URLs; a viewer response MUST NOT.

### Requirement: External feed subscriptions

r[kiln.web.feeds.subscriptions] An operator MUST be able to add, refresh and
remove external RSS/Atom subscriptions. Add MUST fetch before reporting
success. Refresh failure MUST retain previous entries and record a bounded
error. Subscriptions and entries MUST survive restart, and viewers MUST be
read-only. The engine lock MUST NOT be held across network I/O.

#### Scenario: Subscription is stored
r[kiln.web.feeds.subscriptions.scenario.add]
- GIVEN a valid external feed and operator authority
- WHEN the subscription is added
- THEN its parsed title and bounded entries MUST be retained and visible to readers.

#### Scenario: Refresh failure retains entries
r[kiln.web.feeds.subscriptions.scenario.refresh]
- GIVEN an existing subscription whose next fetch fails
- WHEN refresh runs
- THEN previous entries MUST remain and the error MUST be recorded and reported.

#### Scenario: Viewer cannot mutate
r[kiln.web.feeds.subscriptions.scenario.viewer]
- GIVEN a viewer session
- WHEN add, refresh or remove is attempted
- THEN the service MUST deny it without changing state.

### Requirement: Bounded feed destinations and bytes

r[kiln.web.feeds.security] Public subscriptions MUST use HTTPS without userinfo,
fragments or non-default ports. DNS MUST be resolved once, every address MUST be
globally routable, and the connection MUST be pinned to an admitted address.
Redirects MUST be bounded and re-admitted. Proxy environment variables MUST NOT
be used. Subscriptions, entries, feed bytes, redirects and timeouts MUST use
named bounds.

#### Scenario: Private destination is refused
r[kiln.web.feeds.security.scenario.private]
- GIVEN loopback, private, link-local, documentation, multicast or unspecified destinations
- WHEN a subscription is admitted or fetched
- THEN it MUST be refused before a request is sent.

#### Scenario: Redirect is re-admitted
r[kiln.web.feeds.security.scenario.redirect]
- GIVEN a redirect to another host or non-HTTPS URL
- WHEN the feed is fetched
- THEN the redirect MUST be re-admitted under the same destination policy and redirect bound.

#### Scenario: Malformed or oversized feed is refused
r[kiln.web.feeds.security.scenario.malformed]
- GIVEN DTD/entity XML, malformed XML, an unknown root, too many entries or an oversized body
- WHEN parsing runs
- THEN it MUST fail without changing retained entries.

### Requirement: Bounded feed retention

r[kiln.web.feeds.bounds] Subscription count, entries per feed, response bytes, redirects and timeouts MUST use named policy bounds, and a chain of retained subscriptions MUST remain within the durable state bound before a write is committed.

#### Scenario: Body exceeds its bound
r[kiln.web.feeds.bounds.scenario.body]
- GIVEN an external feed larger than the configured byte bound
- WHEN it is fetched
- THEN it MUST be refused without changing retained entries.

### Requirement: Feeds configuration and console

r[kiln.web.feeds.config] The service profile MAY declare a feeds policy with a
dedicated token file and named bounds; absence keeps publication and
subscriptions disabled. The console MUST expose a Feeds page listing repository
Atom URLs, external subscriptions and their entries, with operator controls,
and MUST render feed text as text, never HTML.

#### Scenario: Feeds are disabled
r[kiln.web.feeds.config.scenario.disabled]
- GIVEN no feeds policy
- WHEN feed publication or subscription routes are used
- THEN they MUST report disabled without creating state.

### Requirement: Feeds console

r[kiln.web.feeds.ui] The console MUST expose a Feeds page listing repository Atom URLs, external subscriptions and their entries, with operator controls, and MUST render feed text as text, never HTML.

#### Scenario: Console shows feeds
r[kiln.web.feeds.config.scenario.console]
- GIVEN configured feeds and retained subscriptions
- WHEN the Feeds page loads
- THEN it MUST show repository subscription URLs and external entries with operator-only controls.
