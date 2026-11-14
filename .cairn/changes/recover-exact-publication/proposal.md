# Recover exact publication effects

## Why

Issues #47 and #48 require publication, integration and source lifecycle effects to survive crashes without confusing worker success with remote completion. Ordinary GitHub merge head guards do not atomically guard the admitted base.

## What Changes

Reserve each branch, PR, integration observation and source operation durably before transport. Reconcile uncertain attempts before retry. Bind exact work/version/attempt/fence and candidate. Refuse unattended GitHub landing until an atomic strategy is qualified; retain strict external integration readback.

## Impact

Core publication admission, GitHub publication/landing/source clients and implementer effect consumers. No provider mutation or live qualification is authorized by this change.
