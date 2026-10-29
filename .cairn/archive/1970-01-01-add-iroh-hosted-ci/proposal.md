## Why

The operator requested all identified hosted-CI competitive gaps and selected Iroh as the customer connection mechanism. Existing remote CI, web inspection and workflow transport must be composed rather than replaced. Source remains external. Implementation does not authorize cloud purchases, live charges, repository publication, customer-data disclosure or production activation.

## What Changes

- Add a hosted-CI composition with authenticated Iroh customer RPC and a private local browser bridge; map authenticated peer identity to organization/project policy rather than trusting caller identity fields.
- Deliver organization/repository onboarding and revocation, durable usage/reservations/invoices, repository pipeline compilation and migration, isolated execution/fleet control, scoped secret/cloud identity exchange, and retention/backup/incident operations.
- Reuse existing GitHub exact-source, replay, Lattice execution and status mechanisms and the existing web application through explicit private service bindings.
- Retain all nine gaps as separately tracked implementation and qualification obligations. Do not describe policy structs, mock ports or passing unit tests as a hosted product or production security evidence.

## Impact

- Current consumer: Kiln operator and self-CI organization; maintenance owner: Kiln maintainers.
- Durable capability: repeatable authenticated hosted-CI administration and execution connected to external source, with explicit financial and deployment authority boundaries.
- Existing standalone/core paths remain independent; no provider or commercial implementation enters kiln-core.
- New human policy uses Nickel; canonical identities use BLAKE3, preserving provider protocol algorithms.
- Required verification: positive/negative consumer behavior, actual Iroh request/response and browser surface, exact-source CI integration, restart/replay and spend admission, isolation and teardown, migration diagnostics, backup/restore, full applicable repository gates.
