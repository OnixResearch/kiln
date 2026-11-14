## Decisions

Keep provider-neutral transitions pure in AttemptLedger and persist work/attempt mutations together under the existing SQLite immediate transaction. An unknown transport observation retains a current reconciliation barrier rather than declaring worker death. Lease expiry revokes result admission but does not establish absence of provider execution. Source supersession freezes old work and applies the admitted abort/finish rule atomically with intake. Publication requires current version/fence authorization independently of whether verification previously succeeded.

Reuse bounded existing CLI stages; no implicit daemon or worker authority expansion. Independent verification and GitHub effects retain separate identities and observations. Provider grants and model accounting never become work completion authority.

## Verification

Parent runs coordinated core and durable shell behavioral tests after integration: competing claims, independent items, stale expiry outputs, source supersession, unknown dispatch/restart, cancellation, replay, budget preservation and terminal publication fencing. Native planning gates precede product edits. Real bug/feature, worker cancellation and cross-host model pilot require explicit operator credentials/capacity approval and retained actual evidence.

## Risks and Non-claims

Persisted older rows require serde defaults for added fields. No recorded observation proves a process stopped, a remote effect occurred, or whole-project completion. Ordinary GitHub head guarding does not guard the target; unsupported landing stays PR-ready.
