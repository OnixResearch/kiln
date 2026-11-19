# Swarm News Mirror Specification

## Purpose

Defines the `swarm-news-mirror` capability.

## Requirements

### Requirement: Bounded renews-over-iroh client

r[kiln.implementation.swarm.transport] The mirror MUST reach the swarm only through
a pinned `iroh-news` executable run as a bounded child process with a cleared
environment, exact argv, bounded streams and explicit deadlines. Kiln MUST NOT
receive, store or pass the endpoint key, password or server ticket. Only bounded
JSON stdout may be parsed; a failed or oversized response MUST be reported as a
failure, never as an empty group.

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

### Requirement: Bounded, redacted claim articles

r[kiln.implementation.swarm.claims] Claim subjects MUST be exactly
`CLAIM|RELEASE <repository> <scope>` with bounded ASCII fields; every other
subject MUST be ignored. Bodies MUST be rendered from named facts with the
required signature and MUST be refused when they are oversized, control-bearing
or carry secret references or token-shaped values. Bodies MUST NOT be parsed for
decisions.

#### Scenario: Subject grammar

r[kiln.implementation.swarm.claims.scenario.subject]
- GIVEN valid, malformed, lowercase and unknown-verb subjects
- WHEN they are parsed
- THEN only the exact claim and release forms MUST be recognized
- AND every other subject MUST be `Other`

#### Scenario: Body redaction

r[kiln.implementation.swarm.claims.scenario.redaction]
- GIVEN a body with a secret reference, bearer header, token-shaped run or excessive length
- WHEN it is admitted
- THEN it MUST be refused
- AND a bounded body with the signature MUST be admitted

### Requirement: Overlap defers only

r[kiln.implementation.swarm.overlap] A prospective claim MUST be deferred only by
an open claim from another author on the same repository and scope that is
younger than 24 hours. A claim is open until a `RELEASE` article references it.
Our own claims, released claims, stale claims and different scopes MUST NOT defer.
The decision MUST use subjects and `References` only.

#### Scenario: Another author's open claim defers

r[kiln.implementation.swarm.overlap.scenario.defer]
- GIVEN an open claim from another author on the same scope
- WHEN a claim is evaluated
- THEN it MUST defer with that message id, author and age
- AND the same claim from our author or a different scope MUST NOT defer

#### Scenario: Released and stale claims allow

r[kiln.implementation.swarm.overlap.scenario.release]
- GIVEN a claim with a matching release, a claim older than 24 hours, or a non-claim subject
- WHEN a claim is evaluated
- THEN it MUST NOT defer

### Requirement: Recorded post outcomes

r[kiln.implementation.swarm.outcomes] Every post MUST produce one recorded
outcome: `accepted` after server acceptance, `rejected` after an explicit refusal,
`unavailable` when the client cannot start, and `unknown` after a deadline, lost
response or output bound. A post MUST NOT be blindly retried after an unknown
outcome.

#### Scenario: Outcomes classify exactly

r[kiln.implementation.swarm.outcomes.scenario.classify]
- GIVEN an accepted post, an explicit refusal and a deadline
- WHEN each post completes
- THEN the outcomes MUST be `accepted`, `rejected` and `unknown`

#### Scenario: Unavailable read is not empty

r[kiln.implementation.swarm.outcomes.scenario.unavailable]
- GIVEN a client that cannot complete a read
- WHEN claims are read
- THEN the mirror MUST report unavailability
- AND it MUST NOT treat the group as empty
