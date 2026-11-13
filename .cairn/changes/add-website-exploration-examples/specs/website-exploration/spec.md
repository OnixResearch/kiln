## ADDED Requirements

### Requirement: Disposable interactive website exploration
r[kiln.web.exploration]
Kiln MUST provide a standalone loopback-only example workspace using current console assets and fresh disposable native state. It MUST visibly identify synthetic data, name simulation boundaries and link a browsable example index. It MUST NOT accept production state or provider configuration, execute source commands, fetch remote feeds, deliver notifications or mutate installed services.

#### Scenario: Explore every registered destination and run tab
r[kiln.web.exploration.scenario.coverage]
- GIVEN a newly launched example workspace
- WHEN the reader follows its example index and the console's registered destinations
- THEN every destination and run-detail tab MUST have coherent inspectable examples, exact detail links and suggested exploration actions, including work/planning/review/source/receipt/history/feed/automation feature families

#### Scenario: Modify only disposable local examples
r[kiln.web.exploration.scenario.local]
- GIVEN example CI, work and review state
- WHEN the reader reruns or cancels simulated work, edits an issue or publishes a review discussion
- THEN real local admission/state APIs MUST apply the command only inside the new example root and execution MUST remain explicitly simulated

#### Scenario: Reject cross-origin or filesystem access
r[kiln.web.exploration.scenario.admission]
- GIVEN the example loopback listener
- WHEN a foreign Host or Origin, unsupported request framing, oversized request or unlisted static path is requested
- THEN the shell MUST reject it without reading arbitrary files or requesting a provider effect

#### Scenario: Repeat a clean exploration
r[kiln.web.exploration.scenario.repeat]
- GIVEN a modified example workspace
- WHEN the process exits normally and a new example process starts
- THEN its private temporary stores MUST be recreated from the maintained seeds, and launch/reset instructions MUST name the maintainer and non-claims

#### Scenario: Deploy the standalone examples without changing live CI
r[kiln.web.exploration.scenario.deploy]
- GIVEN an explicit operator request to commit and deploy the example workspace
- WHEN the frozen source is built and installed locally
- THEN a dedicated package and supervised user unit MUST serve the loopback-only examples independently of the authenticated web service, with fresh disposable state on each start and unchanged live profiles, credentials, routing and durable state
