## MODIFIED Requirements

### Requirement: Bounded renews-over-iroh client

r[kiln.implementation.swarm.transport] The mirror MUST reach the swarm only through
a pinned `iroh-news` executable run as a bounded child process with a cleared
environment, exact argv, bounded streams and explicit deadlines. Kiln MUST NOT
receive, store or pass the endpoint key, password or server ticket. Only bounded
JSON stdout may be parsed; a failed or oversized response MUST be reported as a
failure, never as an empty group. Every read used for overlap or message-id
lookup MUST cover the newest articles, starting after the group's high-water
mark minus the read limit. Posts MUST carry the configured From address through
`IROH_NEWS_FROM`.

#### Scenario: Oversized response fails closed

r[kiln.implementation.swarm.transport.scenario.bounded]
- GIVEN a client response larger than the configured bound
- WHEN the mirror reads claims
- THEN it MUST report a failure
- AND it MUST NOT treat the response as an empty or partial article list

#### Scenario: Article dates parse exactly

r[kiln.implementation.swarm.transport.scenario.date]
- GIVEN an RFC 2822 date with a numeric or symbolic zone
- WHEN the article is reduced for overlap
- THEN the date MUST map to the exact Unix millisecond time

#### Scenario: Newest window and From

r[kiln.implementation.swarm.transport.scenario.window]
- GIVEN a group whose high-water mark exceeds the read limit
- WHEN claims are read for overlap
- THEN the read MUST start at the high-water mark minus the limit
- AND a post MUST run with `IROH_NEWS_FROM` set to the configured From

### Requirement: Overlap defers only

r[kiln.implementation.swarm.overlap] A prospective claim MUST be deferred only by
an open claim from another author whose scope overlaps it under fleet scope
containment and that is younger than 24 hours. A claim is open until a
`RELEASE` article references it. Our own claims, released claims, stale claims
and non-overlapping scopes MUST NOT defer. The decision MUST use subjects and
`References` only.

#### Scenario: Another author's open claim defers

r[kiln.implementation.swarm.overlap.scenario.defer]
- GIVEN an open claim from another author on the same scope, a parent path, the whole repository or the same repository in other letter case
- WHEN a claim is evaluated
- THEN it MUST defer with that message id, author and age
- AND the same claim from our author or a disjoint scope MUST NOT defer

#### Scenario: Released and stale claims allow

r[kiln.implementation.swarm.overlap.scenario.release]
- GIVEN a claim with a matching release, a claim older than 24 hours, or a non-claim subject
- WHEN a claim is evaluated
- THEN it MUST NOT defer
