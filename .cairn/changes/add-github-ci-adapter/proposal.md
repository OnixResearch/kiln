## Why

Kiln has provider-neutral CI decisions and a concrete Radicle-to-Lattice path, but no executable GitHub intake, exact-source job, or result-publication shell. GitHub users cannot currently connect repository push and pull-request events to Kiln CI.

## What Changes

- Add a separately packaged GitHub adapter for authenticated push and pull-request webhooks, bounded durable intake, and restart-safe workflow recovery.
- Bind each run to one admitted repository, exact Git commit, reviewed pipeline, and Lattice request; add a bounded exact-source job command for saved Lattice shell workflows.
- Publish commit statuses through a pinned GitHub REST protocol and SecretSpec-resolved credentials, without giving GitHub identities or API types authority in the core.
- Add typed Nickel configuration, positive/negative conformance fixtures, executable smoke evidence, and operator instructions.

## Impact

- **Files**: `crates/kiln-adapter-github`, workspace manifests/lock, `config`, `flake.nix`, existing operator/architecture documentation, and this native Cairn package.
- **Testing**: authenticated and malformed events; duplicate/conflicting/restarted intake; exact source and command outcomes; stale observations; bounded failures and ambiguous status writes; actual executable smoke; workspace, Clippy, Nickel, traceability and Cairn gates.
- **Owner and consumer**: Kiln maintainers own the adapter; the current consumer is a GitHub repository using the existing Lattice workflow port. Deployment requires an operator-provided webhook secret, status credential, source access, and reviewed Lattice route. This change does not authorize remote repository mutation or deployment.
