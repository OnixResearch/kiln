# Improve Fleet console

## Why

The console shows four totals and dense registers, but operators cannot scan the mix of attempt states or tell whether the figures are current. Heartbeats overwrite the observation timestamp and a reconnect can erase a rejected command message.

## What Changes

- Show a bounded attempt-state breakdown and use clearer labels for queue counts.
- Keep stream status, the last accepted snapshot time and command errors separate.
- Make register rows and forms readable on narrow screens and keep decision drafts across unrelated updates.
- Make state and recorded session-machine bars filter the attempts register without a network request.
- Show automatically refreshed Cairn stages and unit counts in Fleet, linked to exact change details.
- Show recorded worker bindings and Fleet sessions on each Cairn change without inferring placement from heartbeats.
- Show requested model artifacts, model service bindings, provider and harness separately from observed machine and agent data.

## Impact

- Affected spec: fleet-orchestration.
- Affected code: authenticated web console Fleet assets and focused UI tests.
- Maintenance: Kiln web console and its read-only Cairn index. No authorization, standalone coordinator UI, or deployment changes.
