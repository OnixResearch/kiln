## Why

Kiln #49's live verified-PR pilot boots the onix-core implementer composition in
a local VM, but a live worker cannot pass `kiln-lattice-dispatch`: the bound
route needs fair-admission records (policy, account, queue row, fresh owner
eligibility and an owner-committed reservation) and only the scripted fixture
helper produces them, for `qualification/disposable` alone. `kiln-implementer
claim` also ignores a configured policy, so its claim is never bound to the
queue row the dispatch gate checks. Separately, `poll` admits every open issue
under one repository-wide policy, so a pilot cannot admit exactly issue #1 or
#2, and the native canonical CI graph constructor is only an uninstalled
example.

## What Changes

- Add an explicitly registered host-local resource owner to #61's fair
  admission: a typed Nickel profile declares one host's capacity, the admitted
  repositories and work classes and a finite campaign budget. Pure core
  functions derive the policy, root account, queue rows, usage and eligibility
  observations, the per-claim reservation and the owner's reply; the shell
  persists them through the existing `AdmissionCommand` transitions and the
  `AdmissionOwnerPortV1` contract. Operator commands `host-owner-admit`,
  `host-owner-observe`, `host-owner-reserve` and `host-owner-release`.
- Route `kiln-implementer claim` through the admission `claim` command when a
  policy is configured, binding the queue claim to the attempt/fence.
- Add an optional explicit `selection` (issue numbers and/or exact labels) to
  the GitHub intake policy. Unselected open issues are observed and never
  admitted, through both webhook drain and polling.
- Promote `examples/implementation_verification_profile.rs` to the installed
  `kiln-web-service implementation-verification-graph` command with the same
  behaviour, backed by `Engine::construct_implementation_verification_service`.

## Impact

- **Files**: `crates/kiln-core/src/fair_admission{.rs,/host_owner.rs,/tests*}`,
  `crates/kiln-implementer/src/{host_owner.rs,admission.rs,lib.rs}`,
  `crates/kiln-implementer/tests/admission_host_owner.rs`,
  `crates/kiln-adapter-github/src/{bin/implementer.rs,implementation.rs,poll.rs,poll_http.rs,tests.rs}`,
  `crates/kiln-adapter-github/tests/implementer_cli.rs`,
  `crates/kiln-web-service/src/{main.rs,implementation_verification_graph_cli.rs,engine/verification.rs,engine/verification_tests.rs}`,
  `crates/kiln-web-service/tests/implementation_verification_graph.rs`, the
  removed example, `config/{fair-admission-host-owner,implementer-policy}.ncl`,
  `config/fixtures/*host-owner*`, `config/fixtures/*selection*`,
  `fixtures/fair-admission/host-owner.json`, `scripts/check-nickel-fixtures.sh`,
  `docs/{fair-admission,implementation-runbook,implementation-qualification,cairn-gate}.md`,
  `CHANGELOG.md`.
- **Owner**: Kiln core/implementer maintainers. Consumer: the onix-core
  `kiln-implementer` module stage units (#49).
- **Testing**: fmt, clippy, workspace tests, Nickel fixture check, Cairn
  validate/gates/traceability, Octet, gitleaks.
- **Non-claims**: no live worker, model, provider, serving or remote capacity
  is observed or qualified; the host owner reports only its declared
  registration and the ledger's own reservations. No second scheduler, queue or
  capacity ledger. Defaults are unchanged without an explicit policy or
  selection.
