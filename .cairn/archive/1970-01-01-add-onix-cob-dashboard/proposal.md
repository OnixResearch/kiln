## Why

The operator wants to browse Onix collaborative objects alongside Kiln self-CI. Six Onix COB types are already registered, but the live workspace has no repository/type inventory or native object inspection surface.

## What Changes

- Add an authenticated read-only Onix COB page with repository/type/search filters, stable paging and URL-restored object selection.
- Read explicitly configured local Git COB refs and bounded native operation JSON for Cairn changes, RFDs, ADRs, verification obligations, release decisions and policy exceptions.
- Preserve native repository/type/object/ref-tip/operation identities. Label raw operations as observations, not reduced lifecycle state, verified signatures or authority.

## Impact

Kiln dashboard/service maintainers own the view and bounded local reader. Cairn retains domain reduction and lifecycle decisions; Radicle retains native storage and transport. The immediate consumer is the existing operator-owned Kiln self-CI workspace. Typed Nickel configuration remains separate from the canonical CI profile, preserving retained runs. Verification covers real Git objects, error boundaries and desktop/mobile browser interactions. No COB mutation, signing, synchronization, source publication or lifecycle acceptance is authorized.
