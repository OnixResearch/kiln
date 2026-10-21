# Operator Dashboard Delta Specification

## ADDED Requirements

### Requirement: Bounded dashboard profile
r[kiln.observe.dashboard.profile] Kiln MUST define a typed dashboard profile that declares the observation profile the dashboard projects through, the requested run page limit, the requested run page count, the requested event tail limit, the watch interval, and non-claims, and MUST reject a profile whose bounds are zero or whose observation profile is not admitted.

#### Scenario: Complete dashboard profile is admitted
r[kiln.observe.dashboard.profile.scenario.match]
- GIVEN a complete dashboard profile over an admitted observation profile
- WHEN dashboard profile admission runs
- THEN Kiln MAY render the dashboard under the declared bounds.

#### Scenario: Dashboard profile bound is zero
r[kiln.observe.dashboard.profile.scenario.zero_bound]
- GIVEN a dashboard profile with a zero page limit, page count, event limit, or watch interval
- WHEN dashboard profile admission runs
- THEN Kiln MUST reject the profile before reading a journal.

#### Scenario: Dashboard profile omits a required non-claim
r[kiln.observe.dashboard.profile.scenario.missing_non_claim]
- GIVEN a dashboard profile that omits a required dashboard non-claim
- WHEN dashboard profile admission runs
- THEN Kiln MUST reject the profile before reading a journal.

### Requirement: Verified journal admission
r[kiln.observe.dashboard.journal] The dashboard MUST admit a journal only when every journal record carries exactly the canonical journal record fields, every record's event identity is reproduced by recomputation over the record's canonical event encoding under the encoding convention its identity kind declares, and the whole record set replays canonically from the initial state into admitted state, and MUST reject a journal that fails any of those checks.

#### Scenario: Canonical journal is admitted
r[kiln.observe.dashboard.journal.scenario.match]
- GIVEN a journal document whose records reproduce their event identities and replay canonically
- WHEN the dashboard admits the journal
- THEN Kiln MAY project the replayed state.

#### Scenario: Journal event is substituted
r[kiln.observe.dashboard.journal.scenario.substituted_event]
- GIVEN a journal document whose record carries an event that does not reproduce its recorded event identity
- WHEN the dashboard admits the journal
- THEN Kiln MUST reject the journal before projecting any view.

#### Scenario: Journal event identity kind has no admitted convention
r[kiln.observe.dashboard.journal.scenario.unsupported_identity_kind]
- GIVEN a journal document whose record declares an event identity kind with no admitted encoding convention
- WHEN the dashboard admits the journal
- THEN Kiln MUST reject the journal before projecting any view.

#### Scenario: Journal history does not replay
r[kiln.observe.dashboard.journal.scenario.replay_failure]
- GIVEN a journal document whose records do not replay canonically into admitted state
- WHEN the dashboard admits the journal
- THEN Kiln MUST reject the journal before projecting any view.

#### Scenario: Journal is a canonical prefix
r[kiln.observe.dashboard.journal.scenario.truncated_prefix]
- GIVEN a journal document whose trailing records were removed and whose remaining records still replay canonically
- WHEN the dashboard admits the journal
- THEN Kiln MUST admit the observed prefix, MUST report the state revision it observed, and MUST NOT claim that the journal is complete.

### Requirement: Projected dashboard views
r[kiln.observe.dashboard.projection] The dashboard MUST derive every reported fact from the accepted observation projection over the admitted journal state, and MUST report the same view identity the observation surfaces serve for the same request and window, and MUST NOT introduce a second projection or a new view vocabulary.

#### Scenario: Dashboard view matches the observation view
r[kiln.observe.dashboard.projection.scenario.parity]
- GIVEN an admitted journal state and an admitted observation profile
- WHEN the dashboard projects a run list and the observation surface projects the same request and window
- THEN both views MUST have equal content and equal identity.

#### Scenario: Dashboard redaction is preserved
r[kiln.observe.dashboard.projection.scenario.redaction]
- GIVEN an observation profile that does not expose a field
- WHEN the dashboard renders the report
- THEN the field MUST be absent and the report MUST name it as redacted.

#### Scenario: Selected target is absent
r[kiln.observe.dashboard.projection.scenario.unknown_target]
- GIVEN a selected run or attempt that is absent from the admitted state
- WHEN the dashboard projects the selected detail view
- THEN the dashboard MUST report the absent target rather than a substituted value.

### Requirement: Deterministic bounded report
r[kiln.observe.dashboard.report] The dashboard MUST render a deterministic report whose run, attempt, product, metric, and event counts respect the admitted bounds, and MUST state the observed state revision, the journal's declared content identity, and the observed bounds, and MUST NOT depend on wall-clock time, terminal width, or environment.

#### Scenario: Equal inputs render equal reports
r[kiln.observe.dashboard.report.scenario.deterministic]
- GIVEN equal admitted journal state, profile, and selection
- WHEN the dashboard renders the report twice
- THEN Kiln MUST produce equal reports.

#### Scenario: Report respects the admitted bounds
r[kiln.observe.dashboard.report.scenario.bounded]
- GIVEN an admitted state with more runs, attempts, and events than the admitted bounds allow
- WHEN the dashboard renders the report
- THEN the report MUST show at most the admitted counts and MUST report the observed bounds.

### Requirement: Read-only dashboard shell
r[kiln.observe.dashboard.shell] The dashboard MUST NOT mutate the journal, request a provider effect, dispatch a command, or open a network connection, and MUST read only the journal and profile files the operator names.

#### Scenario: Dashboard performs no effect
r[kiln.observe.dashboard.shell.scenario.read_only]
- GIVEN an admitted journal and profile
- WHEN the dashboard renders the report
- THEN Kiln MUST NOT emit a command, an effect, or a network request.

#### Scenario: Watch mode re-reads the journal
r[kiln.observe.dashboard.shell.scenario.watch]
- GIVEN a dashboard shell in watch mode with a declared interval
- WHEN the watch interval elapses
- THEN Kiln MUST re-read and re-render the named journal without unbounded buffering.

### Requirement: Dashboard non-claims
r[kiln.observe.dashboard.nonclaims] Dashboard output MUST NOT claim CI success, provider correctness, source trust, journal completeness, release readiness, or that the observed revision is the newest one in existence.

#### Scenario: Report evidence stays scoped
r[kiln.observe.dashboard.nonclaims.scenario.scope]
- GIVEN a rendered dashboard report
- WHEN Kiln records evidence about it
- THEN the evidence MUST remain scoped to the checked journal admission, projection, bounds, and observed revision.
