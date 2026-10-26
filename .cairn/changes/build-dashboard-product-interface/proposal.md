# Build a SaaS-style dashboard interface

## Why

The operator selected a SaaS-style interface, not a hosted multi-user service. The current catalog wraps the Rats index and full standalone report in separate, visually disconnected documents. Kiln needs one coherent application surface over its existing admitted observations.

## What Changes

- One responsive application shell with Overview, Projects, Runs, Activity, Evidence and Settings navigation.
- Real catalog overview, project/history search and explicit report selection, retaining the actual pinned Rats index.
- Embedded report routes without duplicate sidebars; per-history run selection, filters and missing-identity semantics remain authoritative.
- URL-restorable routes, appearance preferences and keyboard navigation; no account, billing, CI control or hosted backend simulations.
- Retain offline single-file delivery, sandbox isolation, no-JavaScript readability and bounded output.

## Impact

- Consumer and adoption: the Kiln dashboard and existing finite Aspen3 preview use ordinary catalog rendering.
- Owner: Kiln dashboard maintainers own shell, route bridge, styling and documented regeneration/checks; existing Rats source ownership is unchanged.
- Immediate outcome: a usable SaaS-style read-only application. Durable capability: one repeatable app shell for every admitted catalog, not a hardcoded demo.
- Files: dashboard catalog renderer, browser assets and embedded report bridge, existing operator documentation and lifecycle evidence.
- Verification: exercise actual compiled HTML offline at desktop/mobile widths, routes, search, exact selection, restoration, missing identities and unavailable browser capability; run focused and repository checks.

## Out of Scope

Hosted services, authentication, tenants, payments, mutable CI controls, provider changes, production service activation, live index replacement, specification acceptance and source publication.
