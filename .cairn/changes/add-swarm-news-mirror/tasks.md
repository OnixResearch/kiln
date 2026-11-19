## Phase 1: Claim rules

- [x] [serial] Add subject grammar, body formatting, redaction admission, RFC 2822 parsing and the overlap decision in core, with positive and negative tests. r[kiln.implementation.swarm.claims] r[kiln.implementation.swarm.overlap]

## Phase 2: Transport

- [x] [serial] [depends:kiln.implementation.swarm.claims] Add the bounded `iroh-news` client with cleared environment, exact argv, bounded streams and post outcomes. r[kiln.implementation.swarm.transport] r[kiln.implementation.swarm.outcomes]
- [x] [serial] [depends:kiln.implementation.swarm.transport] Add the `kiln-swarm` operator CLI for read and post. r[kiln.implementation.swarm.transport]

## Phase 3: Evidence

- [x] [serial] [depends:kiln.implementation.swarm.outcomes] Add adapter fixtures for accepted, rejected, unknown, unavailable and oversized responses. r[kiln.implementation.swarm.outcomes.scenario.classify] r[kiln.implementation.swarm.outcomes.scenario.unavailable] r[kiln.implementation.swarm.transport.scenario.bounded]

## Phase 4: Lifecycle wiring

- [ ] [serial] [depends:kiln.implementation.swarm.outcomes] Mirror one `CLAIM` on `Claimed` and one `RELEASE` on the terminal phase or observed lease expiry, recording intent and outcome without duplicate posts. r[kiln.implementation.swarm.outcomes]
- [ ] [serial] [depends:kiln.implementation.swarm.overlap] Read recent claims before admission and defer on an open overlapping claim with a recorded, operator-visible cause. r[kiln.implementation.swarm.overlap.scenario.defer]
- [ ] [serial] [depends:kiln.implementation.swarm.transport] Add the explicit default-off operator profile, documentation, changelog and configuration fixtures. r[kiln.implementation.swarm.transport]
- [ ] [serial] [depends:kiln.implementation.swarm.outcomes] Run focused tests, Clippy, workspace checks, Nickel fixtures and Cairn gates. r[kiln.implementation.swarm.transport]
