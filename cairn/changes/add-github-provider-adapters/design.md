# Design: GitHub provider adapters

## Context

GitHub combines webhook delivery, status publication, review collaboration, merge mutation, accounts, installations, and Git hosting.

Kiln must split these capabilities. A webhook cannot grant API mutation, and a mirror cannot replace Radicle governance.

## Goal and Success Contract

The goal is optional GitHub trigger, status, and collaboration profiles with explicit authority.

Completion requires signature validation, replay protection, exact identity mapping, rate and size bounds, idempotent status, fresh merge preflight, and reconciliation.

False completion includes one universal profile, ambient tokens, webhook-based authority, mirror promotion, stale merge, or API success used as canonical CI success.

## Decisions

### Decision: Use separate capability profiles

Trigger, status, collaboration read, and collaboration mutation use separate profiles. Each profile names its endpoints, credentials, scopes, operations, and limits.

A token or installation for one profile cannot satisfy another profile automatically.

### Decision: Authenticate triggers before parsing into commands

The shell retains exact body bytes and validates GitHub's required HMAC-SHA256 signature. It then parses the pinned event schema under named bounds.

Delivery replay state remains durable. Unknown event types and repository mismatches fail without command admission.

### Decision: Keep status rebuildable

Status and check-run writes bind one exact Kiln attempt, commit, context, profile, and idempotency key.

A status response is a provider observation. It cannot complete the attempt or change the canonical journal.

### Decision: Preserve collaboration facts

Pull-request head, base, commits, reviews, checks, merge queue, and mergeability remain native observations with freshness and API-source facts.

The adapter does not treat GitHub's mergeability result as a Kiln candidate-set identity.

### Decision: Put merge last

The first implementation can ship trigger, status, and collaboration read without merge mutation.

Merge requires an explicit mutation profile. The shell reloads the pull request, target, candidate commits, approvals, checks, and policy before dispatch.

### Decision: Handle rate and transport ambiguity

The shell records rate-limit and retry facts. A disconnect after an accepted write remains unknown until native lookup reconciles it.

Automatic retries require an idempotent operation or an exact reconciliation check.

### Decision: Keep GitHub non-canonical

Profiles label GitHub as a mirror or optional collaboration provider. They cannot silently replace the configured Radicle authority.

## Configuration

Typed Nickel profiles define API cohorts, repositories, installations, secret references, event types, status contexts, capabilities, scopes, rates, retries, bounds, and authority labels.

## Validation

Positive fixtures cover valid webhooks, status writes, pull-request reads, and authorized merge. Negative fixtures cover invalid signatures, replay, wrong repository, stale heads, missing scopes, rate limits, and ambiguous writes.

## Claim Boundary

Passing checks establishes bounded GitHub API and webhook observations. It does not prove account trust, GitHub availability, mirror freshness, merge success, or source correctness.
