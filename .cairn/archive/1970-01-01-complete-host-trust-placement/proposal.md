## Why

Issue #28 requires observed host security facts, cross-attempt trust separation and honest opt-in timing-channel qualification rather than placement labels or planned co-residency.

## What Changes

Complete bounded host facts and explicit SMT/KSM override admission; reject unknown or vulnerable issue-required kernel status without override. Track actual running job overlap and preserve peer identities across completion. Use physical host identity, not pool labels, for admission. Provide cooperative guest timing probes and an explicit three-placement operator measurement procedure without invented separate-host evidence.

## Impact

Changes affect kiln-microvm host decisions, kiln-hosted fact collection/placement/receipts, host regression tests and hosted operator documentation. Guest kernel pin/check is integrated by the flake owner. No remote mutation or hostile-isolation claim is granted.
