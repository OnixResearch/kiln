## ADDED Requirements

### Requirement: Unified Gate Passport and guarded publication/integration recovery surface

r[kiln.cairn.gate_passport_surface]
Runs, Code Reviews, and read-only `kiln-dashboard` reports MUST render a unified **Gate Passport** uniting typed CI checks, `cairn-authoritative-completion` receipts, Patchgate review decisions, SLSA/in-toto subject and dependency digests, Plan Review receipts, and Semantic Merge O/A/B/M quartet observations. Every passport item MUST carry an explicit provenance badge distinguishing `Observed Receipt` (solid border), `Agent Claim` (dotted border), and `Declared` (dashed border), and MUST flag `historical/stale` entries when bound to a superseded candidate. `2.1 Delivery & Cairn` MUST delegate publication and integration recovery actions (`prepare-publication`, `publish`, `recover-integration`, `close-source`) to their owning commands behind Tier 2/3 confirmation rails and fail closed on mismatched candidates or missing operator authority.

#### Scenario: Agent handoff is never rendered as independent canonical verification

r[kiln.cairn.gate_passport_surface.scenario.provenance_separation]
- GIVEN a Cairn unit with an agent self-reported handoff and no independent `cairn-authoritative-completion` receipt
- WHEN the operator opens its Gate Passport in the console or `kiln-dashboard`
- THEN the agent handoff MUST carry the dotted `Agent Claim` badge and the canonical completion gate MUST read as unverified/pending rather than passed
