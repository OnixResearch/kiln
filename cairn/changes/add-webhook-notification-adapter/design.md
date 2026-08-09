# Design: Webhook notification adapter

## Context

Kiln creates notification intents from canonical state. A shell must deliver them without turning notification endpoints into arbitrary HTTP effect authority.

Webhook receivers differ in authentication, idempotency, retry, and reconciliation support. Each endpoint profile must state these facts.

## Goal and Success Contract

The goal is a constrained outbound `NotificationPort` for registered endpoint profiles.

Completion requires deterministic body projection, destination admission, secret isolation, bounded transport, outcome classification, and ambiguity policy.

False completion includes user-supplied URLs, unrestricted redirects, secret logging, retries after ambiguous acceptance, or receiver success used as CI success.

## Decisions

### Decision: Register endpoint profiles

Human-authored Nickel configuration defines each endpoint origin, path policy, method, content type, authentication, signature, redirect, network, and limit policy.

A notification request selects a profile identity. It cannot supply or modify a destination URL.

### Decision: Project bodies in a pure core

A pure function maps one notification intent into a versioned bounded body. It binds the run, outcome, policy, template, endpoint, and idempotency key.

The shell owns DNS, sockets, TLS, credentials, clocks, and response reads.

### Decision: Constrain network authority

Profiles use exact schemes and hosts with explicit port and path rules. Redirects remain disabled unless every destination passes the same policy.

Resolved addresses pass configured network-range policy before connection. A DNS result cannot expand endpoint authority.

### Decision: Keep secrets outside bodies and logs

Profiles refer to shell-owned secrets. The adapter can add approved headers or signatures without exposing secret bytes to the pure core.

Diagnostics and receipts redact headers and body fields according to policy.

### Decision: Make retries depend on ambiguity

Definite pre-acceptance errors can use bounded retry policy. A timeout or disconnect after possible acceptance becomes unknown.

Automatic retry after unknown requires an idempotent receiver contract or exact reconciliation. Otherwise operator policy decides the next action.

### Decision: Keep notifications non-authoritative

Delivery success or failure cannot change run, attempt, gate, or release outcomes. Notifications remain rebuildable effects and provider observations.

## Configuration

Typed Nickel profiles define endpoint identity, URL components, network policy, body schema, authentication, signatures, secrets, limits, deadlines, retries, idempotency, reconciliation, redaction, and non-claims.

## Validation

Positive fixtures cover accepted delivery, signed delivery, definite retry, and idempotent reconciliation. Negative fixtures cover dynamic URLs, blocked ranges, redirects, secret leakage, oversized bodies, rate limits, timeout, and ambiguous acceptance.

## Claim Boundary

Passing checks establishes bounded request construction and observed delivery results. It does not prove human receipt, endpoint trust, TLS ecosystem correctness, or notification effect.
