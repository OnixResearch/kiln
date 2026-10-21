## MODIFIED Requirements

### Requirement: Pure task-proposal mapping

r[kiln.nix_eval_jobs.task_mapping] Successful derivation records MUST enter a pure bounded mapping decision before they can become Kiln task proposals. Task identities MUST bind exact source, selector, system, attribute, derivation, output, schema, and mapping-policy facts. Distinct attributes that share one derivation (flake package aliases) MUST be admitted as separate proposals; the selected task disambiguates them.

#### Scenario: Valid derivation job maps to a task

- **GIVEN** one admitted successful job record with supported derivation and output facts
- **WHEN** pure task mapping runs
- **THEN** it MUST return one bounded task proposal with separate Kiln and provider-native identities
- **AND** canonical Kiln command admission MUST remain required

#### Scenario: Job identities conflict

- **GIVEN** duplicate attributes, a missing required output, a dependency cycle, or an exceeded task bound
- **WHEN** task mapping runs
- **THEN** it MUST return stable blockers without events or provider effects

#### Scenario: Alias attributes share a derivation

- **GIVEN** two admitted jobs with distinct attributes and the same derivation identity, such as flake `packages.default` and `packages.kiln`
- **WHEN** pure task mapping runs
- **THEN** it MUST return one proposal per attribute with distinct task identities
- **AND** the selected task MUST resolve to exactly one proposal
