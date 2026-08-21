# Blocker: GitHub tenant authority and credentials

Recorded: 2026-08-21

## Blocker

Live GitHub conformance requires an operator-selected GitHub App installation and repository allowlist.

The change also requires SecretSpec declarations for the App private key and webhook secret. No such declarations or credentials are available in this workspace.

Merge conformance requires explicit repository merge authority. Planning and local validation do not grant that authority.

Kiln will not invent an installation ID, organization, repository, API endpoint, secret reference, or merge policy.

## Unblock criteria

Provide a reviewed GitHub App profile with these facts:

- exact API and webhook compatibility cohort;
- organization, installation, and repository allowlists;
- SecretSpec references for the App key and webhook secret;
- status and check-run permissions;
- collaboration-read permissions;
- explicit merge authority or an explicit merge denial; and
- a disposable live-conformance repository.

Then run positive and negative live tests for signatures, replay, rate limits, stale heads, status, checks, reads, and authorized merge behavior.

## Current safe state

No GitHub adapter is enabled. No webhook listener, API request, status write, collaboration read, or merge request is dispatched.

## Non-claims

This blocker record does not prove GitHub availability, credential validity, webhook delivery, API compatibility, status publication, collaboration correctness, or merge authority.
