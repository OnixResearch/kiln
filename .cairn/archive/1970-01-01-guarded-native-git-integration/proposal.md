# Why

Issue #63 requires an explicitly selected native Git route for Cairn's non-PR branch publication and guarded target integration. The existing implementation publication contract represents only verified pull requests and cannot serve as a native receipt without fabricating PR metadata.

## What Changes

- Add a separately versioned native-Git publication/integration contract while leaving v1 PR plans and receipts unchanged.
- Persist native branch and target effects before transport; reconcile unknown outcomes by exact remote observation, never blind retry.
- Admit only scoped, supported Git transport and fast-forward integration with current work, fence, candidate, checks, review, policy, provider and remote authority.
- Provide a native Git operator qualification entrypoint and focused instructions; do not claim live qualification absent an operator-authorized exercise.

## Impact

Own route-specific core modules, Git adapter/native publication modules, durable implementer integration and regression fixtures, plus this lifecycle package and the focused operator guide. No PR fallback, force-push, new queue, remote operation, credentials, global configuration change or unrelated rewrite.

## Authority

Issue #63 authorizes implementation and fixture work only. Live provider effects and credential access remain parent/operator-owned. Remote protection failures and unsupported profiles remain explicit blocked outcomes.
