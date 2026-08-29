# Admit the Radicle Broker 0.31 Wire Envelope

## Problem

The production broker `0.31` sends a Defelo trigger with required `request` and `version` fields. It also includes producer-owned repository, pusher, and revision metadata. Kiln's synthetic Defelo fixture omitted those fields, while its parser rejected every unknown field. The first real post-cutover event therefore failed before Aspen ingress.

## Proposed Change

Admit the exact broker envelope identity and version, then project only the bounded fields Kiln owns: event kind, repository ID, commit list, patch commit list, and branch. Accept producer-owned descriptive metadata without granting it CI authority. Add the broker's published `trigger.json` as a positive fixture and retain malformed, wrong-version, unknown-event, and missing-revision negatives.

## Success Criteria

- Broker request kind `trigger` and protocol version `1` are required.
- The published broker `0.31` push fixture maps to one exact Kiln trigger.
- Producer-owned descriptive metadata cannot replace repository, revision, branch, or event facts.
- Malformed, unsupported, empty, overbound, or revision-less requests fail before effects.
- Tests, strict Clippy, Octet, Cairn, Nix, and a real broker replay pass.
